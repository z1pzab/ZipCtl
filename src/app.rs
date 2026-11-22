use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{ListState, TableState};

use zipctl::api_tester::{test_api_connection, ApiTestResult};
use zipctl::config_patcher::{
    load_or_create_config, patch_harness_config, save_config, HarnessSavedConfig, ProviderPreset,
    ZipCtlConfig,
};
use zipctl::git_radar::{quick_commit_and_push, scan_projects, RepoStatus};
use zipctl::harnesses::{
    default_harnesses, default_web_chats, launch_netlify_ai, launch_or_install_harness,
    open_url_xdg, refresh_installed_flags, HandoffNote, HarnessDef, HarnessKind, WebChatDef,
};
use zipctl::services::{refresh_services, restart_service, toggle_service, ServiceStatus};
use zipctl::sys::{trigger_cache_cleanup, SysMonitor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    System,
    VibeCode,
    GitServices,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VibeSection {
    Harnesses,
    WebChats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpsSection {
    Git,
    Services,
}

#[derive(Debug, Clone)]
pub struct ApiConfigModal {
    pub harness_id: String,
    pub harness_name: String,
    pub config_path_display: String,
    pub provider: ProviderPreset,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub show_key: bool,
    pub active_field: usize, // 0: Provider, 1: Base URL, 2: Model, 3: API Key
    pub test_result: Option<ApiTestResult>,
}

#[derive(Debug, Clone)]
pub struct NetlifyConfigModal {
    pub active_field: usize, // 0: Model, 1: Browser Profile, 2: Add new model
    pub new_model_input: String,
}

#[derive(Debug, Clone)]
pub struct HandoffModal {
    pub note: HandoffNote,
    pub active_field: usize, // 0: Done, 1: In Progress, 2: Next Steps, 3: Modified Files
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    ApiConfig(ApiConfigModal),
    NetlifyConfig(NetlifyConfigModal),
    Handoff(HandoffModal),
}

#[derive(Debug, Clone, Default)]
pub struct LayoutAreas {
    pub tabs_area: Rect,
    pub process_table_area: Rect,
    pub harnesses_area: Rect,
    pub webchats_area: Rect,
    pub git_area: Rect,
    pub services_area: Rect,
}

pub struct App {
    pub should_quit: bool,
    pub active_tab: ActiveTab,
    pub vibe_section: VibeSection,
    pub ops_section: OpsSection,
    pub config: ZipCtlConfig,
    pub sys: SysMonitor,
    pub harnesses: Vec<HarnessDef>,
    pub web_chats: Vec<WebChatDef>,
    pub repos: Vec<RepoStatus>,
    pub repos_error: Option<String>,
    pub services: Vec<ServiceStatus>,

    pub process_state: TableState,
    pub harness_state: ListState,
    pub webchat_state: ListState,
    pub repo_state: ListState,
    pub service_state: ListState,

    pub modal: Modal,
    pub status_msg: String,
    pub status_is_error: bool,
    pub areas: LayoutAreas,
}

impl App {
    pub fn new() -> Self {
        let config = load_or_create_config();
        let sys = SysMonitor::new(config.low_space_gb);
        let harnesses = default_harnesses();
        let web_chats = default_web_chats();
        let (repos, repos_error) = match scan_projects(&config.projects_dir) {
            Ok(list) => (list, None),
            Err(err) => (Vec::new(), Some(err)),
        };
        let services = refresh_services(&config.services);

        let mut process_state = TableState::default();
        if !sys.processes.is_empty() {
            process_state.select(Some(0));
        }

        let mut harness_state = ListState::default();
        harness_state.select(Some(0));

        let mut webchat_state = ListState::default();
        webchat_state.select(Some(0));

        let mut repo_state = ListState::default();
        if !repos.is_empty() {
            repo_state.select(Some(0));
        }

        let mut service_state = ListState::default();
        if !services.is_empty() {
            service_state.select(Some(0));
        }

        Self {
            should_quit: false,
            active_tab: ActiveTab::System,
            vibe_section: VibeSection::Harnesses,
            ops_section: OpsSection::Git,
            config,
            sys,
            harnesses,
            web_chats,
            repos,
            repos_error,
            services,
            process_state,
            harness_state,
            webchat_state,
            repo_state,
            service_state,
            modal: Modal::None,
            status_msg: "Готово. [Tab] вкладки • [h] Handoff • [g] Git Push • [c] кэш • [q] выход"
                .to_string(),
            status_is_error: false,
            areas: LayoutAreas::default(),
        }
    }

    pub fn set_status(&mut self, msg: String, is_error: bool) {
        self.status_msg = msg;
        self.status_is_error = is_error;
    }

    pub fn on_tick(&mut self) {
        self.sys.refresh();
        if let Some(sel) = self.process_state.selected() {
            if self.sys.processes.is_empty() {
                self.process_state.select(None);
            } else if sel >= self.sys.processes.len() {
                self.process_state
                    .select(Some(self.sys.processes.len().saturating_sub(1)));
            }
        } else if !self.sys.processes.is_empty() {
            self.process_state.select(Some(0));
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match &self.modal {
            Modal::None => self.handle_main_key(key),
            Modal::ApiConfig(_) => self.handle_api_modal_key(key),
            Modal::NetlifyConfig(_) => self.handle_netlify_modal_key(key),
            Modal::Handoff(_) => self.handle_handoff_modal_key(key),
        }
    }

    fn handle_main_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('й') => {
                self.should_quit = true;
            }
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    ActiveTab::System => ActiveTab::VibeCode,
                    ActiveTab::VibeCode => ActiveTab::GitServices,
                    ActiveTab::GitServices => ActiveTab::System,
                };
            }
            KeyCode::Char('1') => self.active_tab = ActiveTab::System,
            KeyCode::Char('2') => self.active_tab = ActiveTab::VibeCode,
            KeyCode::Char('3') => self.active_tab = ActiveTab::GitServices,
            KeyCode::Char('h') | KeyCode::Char('р') => self.open_handoff_modal(),
            KeyCode::Char('r') | KeyCode::Char('к') => {
                self.sys.refresh();
                refresh_installed_flags(&mut self.harnesses);
                self.refresh_git_and_services();
                self.set_status(
                    "Метрики, харнессы, Git-репозитории и сервисы обновлены.".to_string(),
                    false,
                );
            }
            _ => match self.active_tab {
                ActiveTab::System => self.handle_system_key(key),
                ActiveTab::VibeCode => self.handle_vibecode_key(key),
                ActiveTab::GitServices => self.handle_ops_key(key),
            },
        }
    }

    fn handle_system_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.move_process_selection(-1),
            KeyCode::Down => self.move_process_selection(1),
            KeyCode::PageUp => self.move_process_selection(-10),
            KeyCode::PageDown => self.move_process_selection(10),
            KeyCode::Char('s') | KeyCode::Char('ы') => {
                self.sys.toggle_sort();
                self.set_status(
                    format!("Сортировка процессов: по {}.", self.sys.sort_mode.label()),
                    false,
                );
            }
            KeyCode::Char('k') | KeyCode::Char('л') | KeyCode::Delete => {
                if let Some(idx) = self.process_state.selected() {
                    if let Some(proc_info) = self.sys.processes.get(idx).cloned() {
                        match self.sys.kill_process(proc_info.pid) {
                            Ok(msg) => self.set_status(msg, false),
                            Err(err) => self.set_status(err, true),
                        }
                    }
                }
            }
            KeyCode::Char('c') | KeyCode::Char('с') => match trigger_cache_cleanup() {
                Ok(msg) => self.set_status(msg, false),
                Err(err) => self.set_status(err, true),
            },
            _ => {}
        }
    }

    fn handle_vibecode_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left => self.vibe_section = VibeSection::Harnesses,
            KeyCode::Right => self.vibe_section = VibeSection::WebChats,
            KeyCode::Up => match self.vibe_section {
                VibeSection::Harnesses => self.move_harness_selection(-1),
                VibeSection::WebChats => self.move_webchat_selection(-1),
            },
            KeyCode::Down => match self.vibe_section {
                VibeSection::Harnesses => self.move_harness_selection(1),
                VibeSection::WebChats => self.move_webchat_selection(1),
            },
            KeyCode::Enter => self.activate_selected_vibe_item(),
            KeyCode::Char('e') | KeyCode::Char('у') => {
                if self.vibe_section == VibeSection::Harnesses {
                    self.open_selected_harness_modal();
                } else {
                    self.set_status(
                        "Настройка API доступна только для раздела «Харнессы».".to_string(),
                        false,
                    );
                }
            }
            _ => {}
        }
    }

    fn handle_ops_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left => self.ops_section = OpsSection::Git,
            KeyCode::Right => self.ops_section = OpsSection::Services,
            KeyCode::Up => match self.ops_section {
                OpsSection::Git => self.move_repo_selection(-1),
                OpsSection::Services => self.move_service_selection(-1),
            },
            KeyCode::Down => match self.ops_section {
                OpsSection::Git => self.move_repo_selection(1),
                OpsSection::Services => self.move_service_selection(1),
            },
            KeyCode::Char('g') | KeyCode::Char('п') => {
                self.trigger_selected_git_push();
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.ops_section {
                OpsSection::Git => self.trigger_selected_git_push(),
                OpsSection::Services => self.toggle_selected_service(),
            },
            KeyCode::Char('R') | KeyCode::Char('К') => {
                if self.ops_section == OpsSection::Services {
                    self.restart_selected_service();
                }
            }
            _ => {}
        }
    }

    pub fn refresh_git_and_services(&mut self) {
        match scan_projects(&self.config.projects_dir) {
            Ok(list) => {
                self.repos = list;
                self.repos_error = None;
                if self.repos.is_empty() {
                    self.repo_state.select(None);
                } else if self.repo_state.selected().is_none_or(|i| i >= self.repos.len()) {
                    self.repo_state.select(Some(0));
                }
            }
            Err(err) => {
                self.repos.clear();
                self.repos_error = Some(err);
                self.repo_state.select(None);
            }
        }
        self.services = refresh_services(&self.config.services);
        if self.services.is_empty() {
            self.service_state.select(None);
        } else if self
            .service_state
            .selected()
            .is_none_or(|i| i >= self.services.len())
        {
            self.service_state.select(Some(0));
        }
    }

    fn trigger_selected_git_push(&mut self) {
        let Some(idx) = self.repo_state.selected() else {
            self.set_status("Нет выбранного Git-репозитория.".to_string(), true);
            return;
        };
        let Some(repo) = self.repos.get(idx).cloned() else {
            return;
        };
        match quick_commit_and_push(&repo, None) {
            Ok(msg) => {
                self.refresh_git_and_services();
                self.set_status(msg, false);
            }
            Err(err) => self.set_status(err, true),
        }
    }

    fn toggle_selected_service(&mut self) {
        let Some(idx) = self.service_state.selected() else {
            return;
        };
        let Some(svc) = self.services.get(idx).cloned() else {
            return;
        };
        match toggle_service(&svc.name, svc.state.is_active()) {
            Ok(msg) => {
                self.services = refresh_services(&self.config.services);
                self.set_status(msg, false);
            }
            Err(err) => self.set_status(err, true),
        }
    }

    fn restart_selected_service(&mut self) {
        let Some(idx) = self.service_state.selected() else {
            return;
        };
        let Some(svc) = self.services.get(idx).cloned() else {
            return;
        };
        match restart_service(&svc.name) {
            Ok(msg) => {
                self.services = refresh_services(&self.config.services);
                self.set_status(msg, false);
            }
            Err(err) => self.set_status(err, true),
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if !matches!(self.modal, Modal::None) {
            return;
        }

        let col = mouse.column;
        let row = mouse.row;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if point_in_rect(col, row, self.areas.tabs_area) {
                    let third = (self.areas.tabs_area.width / 3).max(1);
                    let rel = col.saturating_sub(self.areas.tabs_area.x);
                    self.active_tab = if rel < third {
                        ActiveTab::System
                    } else if rel < third * 2 {
                        ActiveTab::VibeCode
                    } else {
                        ActiveTab::GitServices
                    };
                    return;
                }

                match self.active_tab {
                    ActiveTab::System => {
                        if point_in_rect(col, row, self.areas.process_table_area) {
                            // 1 border + 1 header row
                            let header_offset = self.areas.process_table_area.y + 2;
                            if row >= header_offset {
                                let clicked = (row - header_offset) as usize;
                                let offset = self.process_state.offset();
                                let idx = offset + clicked;
                                if idx < self.sys.processes.len() {
                                    self.process_state.select(Some(idx));
                                }
                            }
                        }
                    }
                    ActiveTab::VibeCode => {
                        if point_in_rect(col, row, self.areas.harnesses_area) {
                            self.vibe_section = VibeSection::Harnesses;
                            let start_y = self.areas.harnesses_area.y + 1;
                            if row >= start_y {
                                let idx = (row - start_y) as usize / 2;
                                if idx < self.harnesses.len() {
                                    self.harness_state.select(Some(idx));
                                    self.activate_selected_vibe_item();
                                }
                            }
                        } else if point_in_rect(col, row, self.areas.webchats_area) {
                            self.vibe_section = VibeSection::WebChats;
                            let start_y = self.areas.webchats_area.y + 1;
                            if row >= start_y {
                                let idx = (row - start_y) as usize / 2;
                                if idx < self.web_chats.len() {
                                    self.webchat_state.select(Some(idx));
                                    self.activate_selected_vibe_item();
                                }
                            }
                        }
                    }
                    ActiveTab::GitServices => {
                        if point_in_rect(col, row, self.areas.git_area) {
                            self.ops_section = OpsSection::Git;
                            let start_y = self.areas.git_area.y + 1;
                            if row >= start_y {
                                let idx = (row - start_y) as usize / 2;
                                if idx < self.repos.len() {
                                    self.repo_state.select(Some(idx));
                                }
                            }
                        } else if point_in_rect(col, row, self.areas.services_area) {
                            self.ops_section = OpsSection::Services;
                            let start_y = self.areas.services_area.y + 1;
                            if row >= start_y {
                                let idx = (row - start_y) as usize / 2;
                                if idx < self.services.len() {
                                    self.service_state.select(Some(idx));
                                    self.toggle_selected_service();
                                }
                            }
                        }
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if self.active_tab == ActiveTab::VibeCode
                    && point_in_rect(col, row, self.areas.harnesses_area)
                {
                    self.vibe_section = VibeSection::Harnesses;
                    let start_y = self.areas.harnesses_area.y + 1;
                    if row >= start_y {
                        let idx = (row - start_y) as usize / 2;
                        if idx < self.harnesses.len() {
                            self.harness_state.select(Some(idx));
                            self.open_selected_harness_modal();
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => match self.active_tab {
                ActiveTab::System => self.move_process_selection(-2),
                ActiveTab::VibeCode => match self.vibe_section {
                    VibeSection::Harnesses => self.move_harness_selection(-1),
                    VibeSection::WebChats => self.move_webchat_selection(-1),
                },
                ActiveTab::GitServices => match self.ops_section {
                    OpsSection::Git => self.move_repo_selection(-1),
                    OpsSection::Services => self.move_service_selection(-1),
                },
            },
            MouseEventKind::ScrollDown => match self.active_tab {
                ActiveTab::System => self.move_process_selection(2),
                ActiveTab::VibeCode => match self.vibe_section {
                    VibeSection::Harnesses => self.move_harness_selection(1),
                    VibeSection::WebChats => self.move_webchat_selection(1),
                },
                ActiveTab::GitServices => match self.ops_section {
                    OpsSection::Git => self.move_repo_selection(1),
                    OpsSection::Services => self.move_service_selection(1),
                },
            },
            _ => {}
        }
    }

    fn activate_selected_vibe_item(&mut self) {
        match self.vibe_section {
            VibeSection::Harnesses => {
                let Some(idx) = self.harness_state.selected() else {
                    return;
                };
                let Some(harness) = self.harnesses.get(idx).cloned() else {
                    return;
                };
                let res = match harness.kind {
                    HarnessKind::NetlifyAi => launch_netlify_ai(&self.config.netlify_ai),
                    HarnessKind::Standard => launch_or_install_harness(&harness),
                };
                match res {
                    Ok(msg) => self.set_status(msg, false),
                    Err(err) => self.set_status(err, true),
                }
            }
            VibeSection::WebChats => {
                let Some(idx) = self.webchat_state.selected() else {
                    return;
                };
                let Some(chat) = self.web_chats.get(idx).cloned() else {
                    return;
                };
                match open_url_xdg(chat.url, chat.name) {
                    Ok(msg) => self.set_status(msg, false),
                    Err(err) => self.set_status(err, true),
                }
            }
        }
    }

    fn open_selected_harness_modal(&mut self) {
        let Some(idx) = self.harness_state.selected() else {
            return;
        };
        let Some(harness) = self.harnesses.get(idx).cloned() else {
            return;
        };

        if harness.kind == HarnessKind::NetlifyAi {
            self.modal = Modal::NetlifyConfig(NetlifyConfigModal {
                active_field: 0,
                new_model_input: String::new(),
            });
            return;
        }

        let saved = self.config.harnesses.get(harness.id);
        let provider = saved
            .map(|s| ProviderPreset::from_slug(&s.provider))
            .unwrap_or(ProviderPreset::CoarenaAi);
        let base_url = saved
            .map(|s| s.base_url.clone())
            .unwrap_or_else(|| provider.default_base_url(harness.id).to_string());
        let model = saved
            .map(|s| s.model.clone())
            .unwrap_or_else(|| provider.default_model(harness.id).to_string());

        self.modal = Modal::ApiConfig(ApiConfigModal {
            harness_id: harness.id.to_string(),
            harness_name: harness.name.to_string(),
            config_path_display: harness.config_path_display.to_string(),
            provider,
            base_url,
            model,
            api_key: String::new(),
            show_key: false,
            active_field: 0,
            test_result: None,
        });
    }

    fn open_handoff_modal(&mut self) {
        let note = HandoffNote::load_existing();
        self.modal = Modal::Handoff(HandoffModal {
            note,
            active_field: 0,
        });
    }

    fn handle_api_modal_key(&mut self, key: KeyEvent) {
        let Modal::ApiConfig(mut m) = std::mem::replace(&mut self.modal, Modal::None) else {
            return;
        };

        let is_ctrl_s =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s');

        match key.code {
            KeyCode::Esc => {
                self.set_status("Настройка API отменена.".to_string(), false);
                return;
            }
            KeyCode::Tab | KeyCode::Down => {
                m.active_field = (m.active_field + 1) % 4;
            }
            KeyCode::BackTab | KeyCode::Up => {
                m.active_field = (m.active_field + 3) % 4;
            }
            KeyCode::F(2) => {
                m.show_key = !m.show_key;
            }
            KeyCode::F(5) => {
                m.test_result = Some(test_api_connection(m.provider, &m.base_url, &m.api_key));
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                m.test_result = Some(test_api_connection(m.provider, &m.base_url, &m.api_key));
            }
            KeyCode::Left if m.active_field == 0 => {
                m.provider = m.provider.prev();
                if m.provider != ProviderPreset::Custom {
                    m.base_url = m.provider.default_base_url(&m.harness_id).to_string();
                    m.model = m.provider.default_model(&m.harness_id).to_string();
                }
            }
            KeyCode::Right if m.active_field == 0 => {
                m.provider = m.provider.next();
                if m.provider != ProviderPreset::Custom {
                    m.base_url = m.provider.default_base_url(&m.harness_id).to_string();
                    m.model = m.provider.default_model(&m.harness_id).to_string();
                }
            }
            KeyCode::Enter if m.active_field == 3 || is_ctrl_s => {
                self.save_api_modal(m);
                return;
            }
            KeyCode::Enter => {
                m.active_field = (m.active_field + 1) % 4;
            }
            KeyCode::Char('s') if is_ctrl_s => {
                self.save_api_modal(m);
                return;
            }
            KeyCode::Backspace => match m.active_field {
                1 => {
                    m.base_url.pop();
                }
                2 => {
                    m.model.pop();
                }
                3 => {
                    m.api_key.pop();
                }
                _ => {}
            },
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                match m.active_field {
                    1 => m.base_url.push(c),
                    2 => m.model.push(c),
                    3 => m.api_key.push(c),
                    _ => {}
                }
            }
            _ => {}
        }

        self.modal = Modal::ApiConfig(m);
    }

    fn save_api_modal(&mut self, m: ApiConfigModal) {
        match patch_harness_config(
            &m.harness_id,
            m.provider,
            m.base_url.trim(),
            m.model.trim(),
            m.api_key.trim(),
        ) {
            Ok(path) => {
                self.config.harnesses.insert(
                    m.harness_id.clone(),
                    HarnessSavedConfig {
                        provider: m.provider.slug().to_string(),
                        base_url: m.base_url.trim().to_string(),
                        model: m.model.trim().to_string(),
                    },
                );
                let _ = save_config(&self.config);
                self.set_status(
                    format!(
                        "Конфиг {} обновлён ({}) — провайдер: {}.",
                        m.harness_name,
                        path.display(),
                        m.provider.label()
                    ),
                    false,
                );
            }
            Err(err) => self.set_status(err, true),
        }
    }

    fn handle_netlify_modal_key(&mut self, key: KeyEvent) {
        let Modal::NetlifyConfig(mut m) = std::mem::replace(&mut self.modal, Modal::None) else {
            return;
        };

        let is_ctrl_s =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s');
        let is_ctrl_o =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('o');

        match key.code {
            KeyCode::Esc => {
                let _ = save_config(&self.config);
                self.set_status("Настройки Netlify AI сохранены.".to_string(), false);
                return;
            }
            KeyCode::Tab | KeyCode::Down => {
                m.active_field = (m.active_field + 1) % 3;
            }
            KeyCode::BackTab | KeyCode::Up => {
                m.active_field = (m.active_field + 2) % 3;
            }
            KeyCode::Left => match m.active_field {
                0 => {
                    let len = self.config.netlify_ai.models.len();
                    if len > 0 {
                        self.config.netlify_ai.selected_model =
                            (self.config.netlify_ai.selected_model + len - 1) % len;
                    }
                }
                1 => {
                    let len = self.config.netlify_ai.profiles.len();
                    if len > 0 {
                        self.config.netlify_ai.selected_profile =
                            (self.config.netlify_ai.selected_profile + len - 1) % len;
                    }
                }
                _ => {}
            },
            KeyCode::Right => match m.active_field {
                0 => {
                    let len = self.config.netlify_ai.models.len();
                    if len > 0 {
                        self.config.netlify_ai.selected_model =
                            (self.config.netlify_ai.selected_model + 1) % len;
                    }
                }
                1 => {
                    let len = self.config.netlify_ai.profiles.len();
                    if len > 0 {
                        self.config.netlify_ai.selected_profile =
                            (self.config.netlify_ai.selected_profile + 1) % len;
                    }
                }
                _ => {}
            },
            KeyCode::Char('o') if is_ctrl_o => {
                let _ = save_config(&self.config);
                match launch_netlify_ai(&self.config.netlify_ai) {
                    Ok(msg) => self.set_status(msg, false),
                    Err(err) => self.set_status(err, true),
                }
                return;
            }
            KeyCode::Char('s') if is_ctrl_s => {
                match save_config(&self.config) {
                    Ok(()) => self.set_status(
                        format!(
                            "Netlify AI: модель «{}», профиль «{}» сохранены.",
                            self.config.netlify_ai.current_model(),
                            self.config
                                .netlify_ai
                                .current_profile()
                                .map(|p| p.name.as_str())
                                .unwrap_or("—")
                        ),
                        false,
                    ),
                    Err(err) => self.set_status(err, true),
                }
                return;
            }
            KeyCode::Enter => {
                if m.active_field == 2 && !m.new_model_input.trim().is_empty() {
                    let model_name = m.new_model_input.trim().to_string();
                    self.config.netlify_ai.models.push(model_name);
                    self.config.netlify_ai.selected_model =
                        self.config.netlify_ai.models.len() - 1;
                    m.new_model_input.clear();
                    let _ = save_config(&self.config);
                } else {
                    let _ = save_config(&self.config);
                    match launch_netlify_ai(&self.config.netlify_ai) {
                        Ok(msg) => self.set_status(msg, false),
                        Err(err) => self.set_status(err, true),
                    }
                    return;
                }
            }
            KeyCode::Backspace if m.active_field == 2 => {
                m.new_model_input.pop();
            }
            KeyCode::Char(c)
                if m.active_field == 2 && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                m.new_model_input.push(c);
            }
            _ => {}
        }

        self.modal = Modal::NetlifyConfig(m);
    }

    fn handle_handoff_modal_key(&mut self, key: KeyEvent) {
        let Modal::Handoff(mut m) = std::mem::replace(&mut self.modal, Modal::None) else {
            return;
        };

        let is_ctrl_s =
            key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s');

        match key.code {
            KeyCode::Esc => {
                self.set_status("Редактирование Handoff отменено.".to_string(), false);
                return;
            }
            KeyCode::Tab | KeyCode::Down => {
                m.active_field = (m.active_field + 1) % 4;
            }
            KeyCode::BackTab | KeyCode::Up => {
                m.active_field = (m.active_field + 3) % 4;
            }
            KeyCode::Char('s') if is_ctrl_s => {
                self.save_handoff_modal(m);
                return;
            }
            KeyCode::Enter => {
                if m.active_field == 3 {
                    self.save_handoff_modal(m);
                    return;
                }
                m.active_field += 1;
            }
            KeyCode::Backspace => match m.active_field {
                0 => {
                    m.note.done.pop();
                }
                1 => {
                    m.note.in_progress.pop();
                }
                2 => {
                    m.note.next_steps.pop();
                }
                3 => {
                    m.note.modified_files.pop();
                }
                _ => {}
            },
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                match m.active_field {
                    0 => m.note.done.push(c),
                    1 => m.note.in_progress.push(c),
                    2 => m.note.next_steps.push(c),
                    3 => m.note.modified_files.push(c),
                    _ => {}
                }
            }
            _ => {}
        }

        self.modal = Modal::Handoff(m);
    }

    fn save_handoff_modal(&mut self, m: HandoffModal) {
        let model = self.config.netlify_ai.current_model().to_string();
        let profile = self
            .config
            .netlify_ai
            .current_profile()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "По умолчанию".to_string());

        match m.note.save_and_copy(&model, &profile) {
            Ok(msg) => self.set_status(msg, false),
            Err(err) => self.set_status(err, true),
        }
    }

    fn move_process_selection(&mut self, delta: isize) {
        let len = self.sys.processes.len();
        if len == 0 {
            self.process_state.select(None);
            return;
        }
        let current = self.process_state.selected().unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, (len - 1) as isize) as usize;
        self.process_state.select(Some(next));
    }

    fn move_harness_selection(&mut self, delta: isize) {
        let len = self.harnesses.len();
        if len == 0 {
            return;
        }
        let current = self.harness_state.selected().unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.harness_state.select(Some(next));
    }

    fn move_webchat_selection(&mut self, delta: isize) {
        let len = self.web_chats.len();
        if len == 0 {
            return;
        }
        let current = self.webchat_state.selected().unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.webchat_state.select(Some(next));
    }

    fn move_repo_selection(&mut self, delta: isize) {
        let len = self.repos.len();
        if len == 0 {
            self.repo_state.select(None);
            return;
        }
        let current = self.repo_state.selected().unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.repo_state.select(Some(next));
    }

    fn move_service_selection(&mut self, delta: isize) {
        let len = self.services.len();
        if len == 0 {
            self.service_state.select(None);
            return;
        }
        let current = self.service_state.selected().unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.service_state.select(Some(next));
    }
}

fn point_in_rect(col: u16, row: u16, rect: Rect) -> bool {
    col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

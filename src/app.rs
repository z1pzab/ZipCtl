use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::widgets::{ListState, TableState};
use tokio::sync::mpsc;

use zipctl::api_tester::{test_api_connection_async, ApiTestResult};
use zipctl::config_patcher::{
    detect_existing_api_key_masked, load_or_create_config, patch_harness_config, save_config,
    HarnessSavedConfig, ProviderPreset, ZipCtlConfig,
};
use zipctl::git_radar::{quick_commit_and_push, scan_projects, RepoStatus};
use zipctl::harnesses::{
    default_harnesses, default_web_chats, launch_netlify_ai, refresh_installed_flags, HandoffNote,
    HarnessDef, HarnessKind, WebChatDef,
};
use zipctl::platform;
use zipctl::services::{
    refresh_services, restart_service, toggle_service, ServiceActionResult, ServiceStatus,
};
use zipctl::sys::SysMonitor;

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
pub struct ForegroundCommand {
    pub title: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub pause_after: bool,
    pub refresh_harnesses_after: bool,
    pub refresh_services_after: bool,
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
    pub existing_key_hint: Option<String>,
    pub show_key: bool,
    pub active_field: usize, // 0: Provider, 1: Base URL, 2: Model, 3: API Key
    pub testing: bool,
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
pub struct ConfirmInstallModal {
    pub harness_name: String,
    pub missing_binary: String,
    pub command_display: String,
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Modal {
    None,
    ApiConfig(ApiConfigModal),
    NetlifyConfig(NetlifyConfigModal),
    Handoff(HandoffModal),
    ConfirmInstall(ConfirmInstallModal),
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

#[derive(Debug)]
pub enum BgEvent {
    ReposLoaded(Result<Vec<RepoStatus>, String>),
    ServicesLoaded(Vec<ServiceStatus>),
    HarnessesChecked(Vec<HarnessDef>),
    GitActionFinished(Result<String, String>),
    ServiceActionFinished(ServiceActionResult),
    ApiTestFinished {
        harness_id: String,
        result: ApiTestResult,
    },
    CacheCleaned(Result<String, String>),
    HandoffSaved(Result<String, String>),
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
    pub repos_loading: bool,
    pub services: Vec<ServiceStatus>,
    pub services_loading: bool,
    pub cleanup_running: bool,

    pub process_state: TableState,
    pub harness_state: ListState,
    pub webchat_state: ListState,
    pub repo_state: ListState,
    pub service_state: ListState,

    pub modal: Modal,
    pub status_msg: String,
    pub status_is_error: bool,
    pub areas: LayoutAreas,

    pending_foreground: Option<ForegroundCommand>,
    bg_tx: mpsc::UnboundedSender<BgEvent>,
    bg_rx: mpsc::UnboundedReceiver<BgEvent>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let (config, config_warn) = load_or_create_config();
        let sys = SysMonitor::new(config.low_space_gb);
        let harnesses = default_harnesses();
        let web_chats = default_web_chats();
        let (bg_tx, bg_rx) = mpsc::unbounded_channel();

        let mut process_state = TableState::default();
        if !sys.processes.is_empty() {
            process_state.select(Some(0));
        }

        let mut harness_state = ListState::default();
        harness_state.select(Some(0));

        let mut webchat_state = ListState::default();
        webchat_state.select(Some(0));

        let (status_msg, status_is_error) = if let Some(warn) = config_warn {
            (warn, true)
        } else {
            (
                "Готово. [Tab] вкладки • [h] Handoff • [r] обновить • [c] кэш • [q] выход"
                    .to_string(),
                false,
            )
        };

        let mut app = Self {
            should_quit: false,
            active_tab: ActiveTab::System,
            vibe_section: VibeSection::Harnesses,
            ops_section: OpsSection::Git,
            config,
            sys,
            harnesses,
            web_chats,
            repos: Vec::new(),
            repos_error: None,
            repos_loading: true,
            services: Vec::new(),
            services_loading: true,
            cleanup_running: false,
            process_state,
            harness_state,
            webchat_state,
            repo_state: ListState::default(),
            service_state: ListState::default(),
            modal: Modal::None,
            status_msg,
            status_is_error,
            areas: LayoutAreas::default(),
            pending_foreground: None,
            bg_tx,
            bg_rx,
        };

        app.spawn_refresh_repos();
        app.spawn_refresh_services();
        app
    }

    pub fn set_status(&mut self, msg: String, is_error: bool) {
        self.status_msg = msg;
        self.status_is_error = is_error;
    }

    pub fn take_pending_foreground(&mut self) -> Option<ForegroundCommand> {
        self.pending_foreground.take()
    }

    pub fn on_foreground_finished(
        &mut self,
        fg: &ForegroundCommand,
        result: Result<Option<i32>, String>,
    ) {
        match result {
            Ok(Some(0)) | Ok(None) => {
                self.set_status(format!("«{}» завершён.", fg.title), false);
            }
            Ok(Some(code)) => {
                self.set_status(
                    format!("«{}» завершился с кодом {code}.", fg.title),
                    true,
                );
            }
            Err(err) => self.set_status(format!("Ошибка запуска «{}»: {err}", fg.title), true),
        }

        if fg.refresh_harnesses_after {
            self.spawn_refresh_harnesses();
        }
        if fg.refresh_services_after {
            self.spawn_refresh_services();
        }
    }

    pub fn drain_bg_events(&mut self) {
        while let Ok(event) = self.bg_rx.try_recv() {
            match event {
                BgEvent::ReposLoaded(res) => {
                    self.repos_loading = false;
                    match res {
                        Ok(list) => {
                            self.repos = list;
                            self.repos_error = None;
                            if self.repos.is_empty() {
                                self.repo_state.select(None);
                            } else if self.repo_state.selected().is_none_or(|i| i >= self.repos.len())
                            {
                                self.repo_state.select(Some(0));
                            }
                        }
                        Err(err) => {
                            self.repos.clear();
                            self.repos_error = Some(err);
                            self.repo_state.select(None);
                        }
                    }
                }
                BgEvent::ServicesLoaded(list) => {
                    self.services_loading = false;
                    self.services = list;
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
                BgEvent::HarnessesChecked(list) => {
                    self.harnesses = list;
                    if self.harnesses.is_empty() {
                        self.harness_state.select(None);
                    } else if self
                        .harness_state
                        .selected()
                        .is_none_or(|i| i >= self.harnesses.len())
                    {
                        self.harness_state.select(Some(0));
                    }
                }
                BgEvent::GitActionFinished(res) => match res {
                    Ok(msg) => {
                        self.set_status(msg, false);
                        self.spawn_refresh_repos();
                    }
                    Err(err) => self.set_status(err, true),
                },
                BgEvent::ServiceActionFinished(action_res) => match action_res {
                    ServiceActionResult::Completed(Ok(msg)) => {
                        self.set_status(msg, false);
                        self.spawn_refresh_services();
                    }
                    ServiceActionResult::Completed(Err(err)) => {
                        self.set_status(err, true);
                        self.spawn_refresh_services();
                    }
                    ServiceActionResult::NeedsInteractive {
                        program,
                        args,
                        title,
                    } => {
                        if let Some(exe) = platform::find_in_path(&program) {
                            self.pending_foreground = Some(ForegroundCommand {
                                title,
                                program: exe,
                                args,
                                pause_after: true,
                                refresh_harnesses_after: false,
                                refresh_services_after: true,
                            });
                        } else {
                            self.set_status(
                                format!("Утилита «{program}» не найдена в PATH."),
                                true,
                            );
                        }
                    }
                },
                BgEvent::ApiTestFinished { harness_id, result } => {
                    if let Modal::ApiConfig(ref mut m) = self.modal {
                        if m.harness_id == harness_id {
                            m.testing = false;
                            m.test_result = Some(result);
                        }
                    }
                }
                BgEvent::CacheCleaned(res) => {
                    self.cleanup_running = false;
                    match res {
                        Ok(msg) => {
                            self.sys.refresh();
                            self.set_status(msg, false);
                        }
                        Err(err) => self.set_status(err, true),
                    }
                }
                BgEvent::HandoffSaved(res) => match res {
                    Ok(msg) => self.set_status(msg, false),
                    Err(err) => self.set_status(err, true),
                },
            }
        }
    }

    pub fn spawn_refresh_repos(&mut self) {
        self.repos_loading = true;
        let tx = self.bg_tx.clone();
        let dir = self.config.projects_dir.clone();
        tokio::spawn(async move {
            let res = scan_projects(dir).await;
            let _ = tx.send(BgEvent::ReposLoaded(res));
        });
    }

    pub fn spawn_refresh_services(&mut self) {
        self.services_loading = true;
        let tx = self.bg_tx.clone();
        let names = self.config.services.clone();
        tokio::spawn(async move {
            let list = refresh_services(names).await;
            let _ = tx.send(BgEvent::ServicesLoaded(list));
        });
    }

    pub fn spawn_refresh_harnesses(&self) {
        let tx = self.bg_tx.clone();
        let mut current = self.harnesses.clone();
        tokio::spawn(async move {
            let updated = tokio::task::spawn_blocking(move || {
                refresh_installed_flags(&mut current);
                current
            })
            .await;
            if let Ok(list) = updated {
                let _ = tx.send(BgEvent::HarnessesChecked(list));
            }
        });
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
            Modal::ConfirmInstall(_) => self.handle_confirm_install_key(key),
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
                self.spawn_refresh_harnesses();
                self.spawn_refresh_repos();
                self.spawn_refresh_services();
                self.set_status(
                    "Запущено фоновое обновление метрик, харнессов, Git и сервисов.".to_string(),
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
            KeyCode::Char('c') | KeyCode::Char('с') => {
                if self.cleanup_running {
                    self.set_status("Очистка кэша уже выполняется в фоне...".to_string(), false);
                    return;
                }
                self.cleanup_running = true;
                self.set_status("Очистка кэша выполняется в фоне...".to_string(), false);
                let tx = self.bg_tx.clone();
                tokio::spawn(async move {
                    let res = platform::cleanup_cache().await;
                    let _ = tx.send(BgEvent::CacheCleaned(res));
                });
            }
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
            KeyCode::Char('o') | KeyCode::Char('щ') if self.ops_section == OpsSection::Git => {
                self.open_selected_repo_dir();
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.ops_section {
                OpsSection::Git => self.trigger_selected_git_push(),
                OpsSection::Services => self.toggle_selected_service(),
            },
            KeyCode::Char('R') | KeyCode::Char('К')
                if self.ops_section == OpsSection::Services =>
            {
                self.restart_selected_service();
            }
            _ => {}
        }
    }

    fn open_selected_repo_dir(&mut self) {
        let Some(idx) = self.repo_state.selected() else {
            self.set_status("Нет выбранного Git-репозитория.".to_string(), true);
            return;
        };
        let Some(repo) = self.repos.get(idx) else {
            return;
        };
        match platform::open_path(&repo.path) {
            Ok(msg) => self.set_status(msg, false),
            Err(err) => self.set_status(err, true),
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
        self.set_status(
            format!("Выполняется git add/commit/push для «{}»...", repo.name),
            false,
        );
        let tx = self.bg_tx.clone();
        tokio::spawn(async move {
            let res = quick_commit_and_push(repo, None).await;
            let _ = tx.send(BgEvent::GitActionFinished(res));
        });
    }

    fn toggle_selected_service(&mut self) {
        let Some(idx) = self.service_state.selected() else {
            return;
        };
        let Some(svc) = self.services.get(idx).cloned() else {
            return;
        };
        let action_word = if svc.state.is_active() {
            "Остановка"
        } else {
            "Запуск"
        };
        self.set_status(format!("{action_word} службы «{}»...", svc.name), false);
        let tx = self.bg_tx.clone();
        tokio::spawn(async move {
            let res = toggle_service(svc.name, svc.state.is_active()).await;
            let _ = tx.send(BgEvent::ServiceActionFinished(res));
        });
    }

    fn restart_selected_service(&mut self) {
        let Some(idx) = self.service_state.selected() else {
            return;
        };
        let Some(svc) = self.services.get(idx).cloned() else {
            return;
        };
        self.set_status(format!("Перезапуск службы «{}»...", svc.name), false);
        let tx = self.bg_tx.clone();
        tokio::spawn(async move {
            let res = restart_service(svc.name).await;
            let _ = tx.send(BgEvent::ServiceActionFinished(res));
        });
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
                            let header_offset = self.areas.process_table_area.y + 2;
                            if row >= header_offset {
                                let clicked = (row - header_offset) as usize;
                                let idx = self.process_state.offset() + clicked;
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
                                let idx = self.harness_state.offset() + (row - start_y) as usize / 2;
                                if idx < self.harnesses.len() {
                                    self.harness_state.select(Some(idx));
                                    self.activate_selected_vibe_item();
                                }
                            }
                        } else if point_in_rect(col, row, self.areas.webchats_area) {
                            self.vibe_section = VibeSection::WebChats;
                            let start_y = self.areas.webchats_area.y + 1;
                            if row >= start_y {
                                let idx = self.webchat_state.offset() + (row - start_y) as usize / 2;
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
                                let idx = self.repo_state.offset() + (row - start_y) as usize / 2;
                                if idx < self.repos.len() {
                                    self.repo_state.select(Some(idx));
                                }
                            }
                        } else if point_in_rect(col, row, self.areas.services_area) {
                            self.ops_section = OpsSection::Services;
                            let start_y = self.areas.services_area.y + 1;
                            if row >= start_y {
                                let idx = self.service_state.offset() + (row - start_y) as usize / 2;
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
                        let idx = self.harness_state.offset() + (row - start_y) as usize / 2;
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
                match harness.kind {
                    HarnessKind::NetlifyAi => match launch_netlify_ai(&self.config.netlify_ai) {
                        Ok(msg) => self.set_status(msg, false),
                        Err(err) => self.set_status(err, true),
                    },
                    HarnessKind::Standard => {
                        if let Some(exe) = harness.active_binary() {
                            self.pending_foreground = Some(ForegroundCommand {
                                title: harness.name.to_string(),
                                program: exe,
                                args: Vec::new(),
                                pause_after: false,
                                refresh_harnesses_after: false,
                                refresh_services_after: false,
                            });
                        } else if let Some(spec) = harness.install_spec() {
                            self.modal = Modal::ConfirmInstall(ConfirmInstallModal {
                                harness_name: harness.name.to_string(),
                                missing_binary: harness.binary.to_string(),
                                command_display: spec.display_string(),
                                program: spec.program.to_string(),
                                args: spec.args.iter().map(|s| (*s).to_string()).collect(),
                            });
                        } else {
                            self.set_status(
                                format!(
                                    "Харнесс «{}» ({}) не найден в PATH, автоустановка для этой ОС не задана.",
                                    harness.name, harness.binary
                                ),
                                true,
                            );
                        }
                    }
                }
            }
            VibeSection::WebChats => {
                let Some(idx) = self.webchat_state.selected() else {
                    return;
                };
                let Some(chat) = self.web_chats.get(idx).cloned() else {
                    return;
                };
                match platform::open_url(chat.url, chat.name) {
                    Ok(msg) => self.set_status(msg, false),
                    Err(err) => self.set_status(err, true),
                }
            }
        }
    }

    fn handle_confirm_install_key(&mut self, key: KeyEvent) {
        let Modal::ConfirmInstall(m) = std::mem::replace(&mut self.modal, Modal::None) else {
            return;
        };

        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Char('д') | KeyCode::Char('Д') => {
                if let Some(exe) = platform::find_in_path(&m.program) {
                    self.pending_foreground = Some(ForegroundCommand {
                        title: format!("Установка {}", m.harness_name),
                        program: exe,
                        args: m.args,
                        pause_after: true,
                        refresh_harnesses_after: true,
                        refresh_services_after: false,
                    });
                } else {
                    self.set_status(
                        format!(
                            "Не найден «{}» в PATH — не удалось выполнить «{}».",
                            m.program, m.command_display
                        ),
                        true,
                    );
                }
            }
            KeyCode::Char('n')
            | KeyCode::Char('N')
            | KeyCode::Char('н')
            | KeyCode::Char('Н')
            | KeyCode::Esc
            | KeyCode::Enter => {
                self.set_status(
                    format!("Установка «{}» отменена пользователем.", m.harness_name),
                    false,
                );
            }
            _ => {
                self.modal = Modal::ConfirmInstall(m);
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
        let existing_key_hint = detect_existing_api_key_masked(harness.id);

        self.modal = Modal::ApiConfig(ApiConfigModal {
            harness_id: harness.id.to_string(),
            harness_name: harness.name.to_string(),
            config_path_display: harness.config_path_display(),
            provider,
            base_url,
            model,
            api_key: String::new(),
            existing_key_hint,
            show_key: false,
            active_field: 0,
            testing: false,
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

    fn trigger_api_test(&self, m: &mut ApiConfigModal) {
        m.testing = true;
        m.test_result = None;
        let tx = self.bg_tx.clone();
        let harness_id = m.harness_id.clone();
        let provider = m.provider;
        let base_url = m.base_url.clone();
        let api_key = m.api_key.clone();
        tokio::spawn(async move {
            let result = test_api_connection_async(provider, base_url, api_key).await;
            let _ = tx.send(BgEvent::ApiTestFinished { harness_id, result });
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
                self.trigger_api_test(&mut m);
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.trigger_api_test(&mut m);
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
            Ok(res) => {
                self.config.harnesses.insert(
                    m.harness_id.clone(),
                    HarnessSavedConfig {
                        provider: m.provider.slug().to_string(),
                        base_url: m.base_url.trim().to_string(),
                        model: m.model.trim().to_string(),
                    },
                );
                let _ = save_config(&self.config);
                let bak_info = res
                    .backup_path
                    .as_ref()
                    .map(|b| format!(" • бэкап: {}", b.display()))
                    .unwrap_or_default();
                self.set_status(
                    format!(
                        "Конфиг {} обновлён ({}) — провайдер: {}{bak_info}.",
                        m.harness_name,
                        res.path.display(),
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

        self.set_status(
            "Сохранение Handoff и копирование в буфер обмена...".to_string(),
            false,
        );
        let tx = self.bg_tx.clone();
        tokio::spawn(async move {
            let res = m.note.save_and_copy(&model, &profile).await;
            let _ = tx.send(BgEvent::HandoffSaved(res));
        });
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

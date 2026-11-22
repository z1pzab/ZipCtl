use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::config_patcher::NetlifyAiConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessKind {
    Standard,
    NetlifyAi,
}

#[derive(Debug, Clone)]
pub struct HarnessDef {
    pub id: &'static str,
    pub name: &'static str,
    pub binary: &'static str,
    pub install_cmd: &'static str,
    pub launch_cmd: &'static str,
    pub config_path_display: &'static str,
    pub description: &'static str,
    pub kind: HarnessKind,
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct WebChatDef {
    pub name: &'static str,
    pub url: &'static str,
    pub description: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct HandoffNote {
    pub done: String,
    pub in_progress: String,
    pub next_steps: String,
    pub modified_files: String,
}

pub fn default_harnesses() -> Vec<HarnessDef> {
    let mut items = vec![
        HarnessDef {
            id: "antigravity",
            name: "Antigravity",
            binary: "antigravity",
            install_cmd: "yay -S --noconfirm antigravity-bin",
            launch_cmd: "antigravity",
            config_path_display: "~/.config/antigravity/config.json",
            description: "Агентная среда разработки Google DeepMind",
            kind: HarnessKind::Standard,
            installed: false,
        },
        HarnessDef {
            id: "codex",
            name: "Codex",
            binary: "codex",
            install_cmd: "npm i -g @openai/codex",
            launch_cmd: "codex",
            config_path_display: "~/.codex/config.toml",
            description: "CLI-агент OpenAI Codex",
            kind: HarnessKind::Standard,
            installed: false,
        },
        HarnessDef {
            id: "claude_code",
            name: "Claude Code",
            binary: "claude",
            install_cmd: "npm i -g @anthropic-ai/claude-code",
            launch_cmd: "claude",
            config_path_display: "~/.claude.json",
            description: "CLI-агент Anthropic Claude Code",
            kind: HarnessKind::Standard,
            installed: false,
        },
        HarnessDef {
            id: "deepseek_harness",
            name: "DeepSeek Harness",
            binary: "deepseek-cli",
            install_cmd: "npm i -g deepseek-cli",
            launch_cmd: "deepseek-cli",
            config_path_display: "~/.config/deepseek-harness/config.toml",
            description: "Терминальный агент для моделей DeepSeek",
            kind: HarnessKind::Standard,
            installed: false,
        },
        HarnessDef {
            id: "superagent",
            name: "SuperAgent",
            binary: "superagent",
            install_cmd: "npm i -g superagent-cli",
            launch_cmd: "superagent",
            config_path_display: "~/.config/superagent/config.json",
            description: "Автономный мультимодельный CLI-харнесс",
            kind: HarnessKind::Standard,
            installed: false,
        },
        HarnessDef {
            id: "opencode",
            name: "OpenCode",
            binary: "opencode",
            install_cmd: "yay -S --noconfirm opencode-bin",
            launch_cmd: "opencode",
            config_path_display: "~/.config/opencode/config.json",
            description: "Открытый TUI-агент для разработки",
            kind: HarnessKind::Standard,
            installed: false,
        },
        HarnessDef {
            id: "netlify_ai",
            name: "Netlify AI",
            binary: "xdg-open",
            install_cmd: "yay -S --noconfirm xdg-utils",
            launch_cmd: "xdg-open https://app.netlify.com",
            config_path_display: "~/.config/zipctl/config.toml",
            description: "Смена Google-профиля, выбор модели и Handoff без API-ключа",
            kind: HarnessKind::NetlifyAi,
            installed: true,
        },
    ];

    refresh_installed_flags(&mut items);
    items
}

pub fn default_web_chats() -> Vec<WebChatDef> {
    vec![
        WebChatDef {
            name: "Gemini",
            url: "https://gemini.google.com",
            description: "Веб-чат Google Gemini",
        },
        WebChatDef {
            name: "ChatGPT",
            url: "https://chatgpt.com",
            description: "Веб-чат OpenAI ChatGPT",
        },
        WebChatDef {
            name: "Claude",
            url: "https://claude.ai",
            description: "Веб-чат Anthropic Claude",
        },
        WebChatDef {
            name: "DeepSeek",
            url: "https://chat.deepseek.com",
            description: "Веб-чат DeepSeek",
        },
    ]
}

pub fn refresh_installed_flags(harnesses: &mut [HarnessDef]) {
    for h in harnesses {
        h.installed = match h.kind {
            HarnessKind::NetlifyAi => true,
            HarnessKind::Standard => which_exists(h.binary),
        };
    }
}

pub fn which_exists(bin: &str) -> bool {
    if bin.contains('/') {
        return Path::new(bin).is_file();
    }
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join(bin);
            if candidate.is_file() {
                return true;
            }
        }
    }
    false
}

pub fn launch_or_install_harness(harness: &HarnessDef) -> Result<String, String> {
    if which_exists(harness.binary) {
        spawn_in_kitty(
            &format!("zipctl: {}", harness.name),
            &format!("{}; exec ${{SHELL:-/bin/bash}}", harness.launch_cmd),
        )?;
        Ok(format!("Запущен «{}» в новом окне Kitty.", harness.name))
    } else {
        let install_script = format!(
            "echo '=== Установка {} ==='; echo 'Команда: {}'; echo ''; {}; echo ''; read -rp 'Установка завершена. Нажмите Enter...'",
            harness.name, harness.install_cmd, harness.install_cmd
        );
        spawn_in_kitty(&format!("Установка {}", harness.name), &install_script)?;
        Ok(format!(
            "Бинарник «{}» не найден — запущена установка ({}) в Kitty.",
            harness.binary, harness.install_cmd
        ))
    }
}

pub fn spawn_in_kitty(title: &str, shell_cmd: &str) -> Result<(), String> {
    if which_exists("kitty") {
        Command::new("kitty")
            .args(["--title", title, "sh", "-c", shell_cmd])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Ошибка запуска Kitty: {e}"))?;
        Ok(())
    } else {
        Err("Терминал Kitty не найден в PATH. Установите пакет kitty.".to_string())
    }
}

pub fn open_url_xdg(url: &str, label: &str) -> Result<String, String> {
    Command::new("xdg-open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Не удалось открыть {label} через xdg-open: {e}"))?;
    Ok(format!("Открыт {label} ({url}) через xdg-open."))
}

pub fn launch_netlify_ai(cfg: &NetlifyAiConfig) -> Result<String, String> {
    let model = cfg.current_model();
    let url = if cfg.url.trim().is_empty() {
        "https://app.netlify.com"
    } else {
        cfg.url.trim()
    };

    if let Some(profile) = cfg.current_profile() {
        let browser = profile.browser.trim();
        let dir = profile.profile_dir.trim();

        if browser != "xdg-open" && !browser.is_empty() && which_exists(browser) {
            let mut cmd = Command::new(browser);
            if browser.contains("firefox") || browser.contains("zen") || browser.contains("librewolf")
            {
                if !dir.is_empty() {
                    cmd.args(["-P", dir]);
                }
            } else if !dir.is_empty() {
                cmd.arg(format!("--profile-directory={dir}"));
            }
            cmd.arg(url)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Ошибка запуска браузера {browser}: {e}"))?;

            return Ok(format!(
                "Netlify AI открыт в профиле «{}» (модель: {model}).",
                profile.name
            ));
        }
    }

    open_url_xdg(url, &format!("Netlify AI (модель: {model})"))
}

impl HandoffNote {
    pub const FILE_NAME: &'static str = ".zipctl_handoff.md";

    pub fn load_existing() -> Self {
        let Ok(content) = fs::read_to_string(Self::FILE_NAME) else {
            return Self::default();
        };

        let mut note = Self::default();
        let mut section = "";
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("## Сделано") || trimmed.starts_with("## Done") {
                section = "done";
                continue;
            } else if trimmed.starts_with("## В процессе") || trimmed.starts_with("## In Progress")
            {
                section = "in_progress";
                continue;
            } else if trimmed.starts_with("## Следующие шаги")
                || trimmed.starts_with("## Next Steps")
            {
                section = "next_steps";
                continue;
            } else if trimmed.starts_with("## Изменённые файлы")
                || trimmed.starts_with("## Modified Files")
            {
                section = "modified_files";
                continue;
            } else if trimmed.starts_with('#') || trimmed.starts_with('>') {
                continue;
            }

            if trimmed.is_empty() {
                continue;
            }
            let target = match section {
                "done" => &mut note.done,
                "in_progress" => &mut note.in_progress,
                "next_steps" => &mut note.next_steps,
                "modified_files" => &mut note.modified_files,
                _ => continue,
            };
            if !target.is_empty() {
                target.push_str("; ");
            }
            target.push_str(trimmed.trim_start_matches("- "));
        }
        note
    }

    pub fn render_markdown(&self, active_model: &str, active_profile: &str) -> String {
        let done = if self.done.trim().is_empty() {
            "—"
        } else {
            self.done.trim()
        };
        let in_progress = if self.in_progress.trim().is_empty() {
            "—"
        } else {
            self.in_progress.trim()
        };
        let next_steps = if self.next_steps.trim().is_empty() {
            "—"
        } else {
            self.next_steps.trim()
        };
        let modified = if self.modified_files.trim().is_empty() {
            "—"
        } else {
            self.modified_files.trim()
        };

        format!(
            "# HANDOFF — Передача контекста (zipctl)\n\
             > Модель: {active_model} | Профиль: {active_profile}\n\n\
             ## Сделано (Done)\n\
             - {done}\n\n\
             ## В процессе (In Progress)\n\
             - {in_progress}\n\n\
             ## Следующие шаги (Next Steps)\n\
             - {next_steps}\n\n\
             ## Изменённые файлы (Modified Files)\n\
             - {modified}\n"
        )
    }

    pub fn save_and_copy(&self, active_model: &str, active_profile: &str) -> Result<String, String> {
        let md = self.render_markdown(active_model, active_profile);
        fs::write(Self::FILE_NAME, &md)
            .map_err(|e| format!("Не удалось записать {}: {e}", Self::FILE_NAME))?;

        let clip_status = copy_to_wayland_clipboard(&md);
        match clip_status {
            Ok(tool) => Ok(format!(
                "Заметка сохранена в ./{} и скопирована в буфер ({tool}).",
                Self::FILE_NAME
            )),
            Err(warn) => Ok(format!(
                "Сохранено в ./{}, но буфер обмена недоступен ({warn}).",
                Self::FILE_NAME
            )),
        }
    }
}

fn copy_to_wayland_clipboard(text: &str) -> Result<&'static str, String> {
    if which_exists("wl-copy") {
        let mut child = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("wl-copy: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("wl-copy write: {e}"))?;
        }
        let _ = child.wait();
        return Ok("wl-copy");
    }

    if which_exists("xclip") {
        let mut child = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("xclip: {e}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("xclip write: {e}"))?;
        }
        let _ = child.wait();
        return Ok("xclip");
    }

    Err("установите wl-clipboard".to_string())
}

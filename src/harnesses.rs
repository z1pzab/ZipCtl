use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::config_patcher::{harness_config_path, NetlifyAiConfig};
use crate::platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessKind {
    Standard,
    NetlifyAi,
}

#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    pub program: &'static str,
    pub args: &'static [&'static str],
}

impl CommandSpec {
    pub fn display_string(&self) -> String {
        if self.args.is_empty() {
            self.program.to_string()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

#[derive(Debug, Clone)]
pub struct HarnessDef {
    pub id: &'static str,
    pub name: &'static str,
    pub binary: &'static str,
    pub alt_binaries: &'static [&'static str],
    pub install_linux: Option<CommandSpec>,
    pub install_windows: Option<CommandSpec>,
    pub description: &'static str,
    pub kind: HarnessKind,
    pub installed: bool,
    pub resolved_binary: Option<PathBuf>,
}

impl HarnessDef {
    pub fn install_spec(&self) -> Option<CommandSpec> {
        #[cfg(target_os = "linux")]
        {
            self.install_linux
        }
        #[cfg(target_os = "windows")]
        {
            self.install_windows
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            self.install_linux.or(self.install_windows)
        }
    }

    pub fn install_command_display(&self) -> Option<String> {
        self.install_spec().map(|s| s.display_string())
    }

    pub fn active_binary(&self) -> Option<PathBuf> {
        if let Some(p) = platform::find_in_path(self.binary) {
            return Some(p);
        }
        for alt in self.alt_binaries {
            if let Some(p) = platform::find_in_path(alt) {
                return Some(p);
            }
        }
        None
    }

    pub fn config_path_display(&self) -> String {
        harness_config_path(self.id)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "—".to_string())
    }
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

const NPM_CODEX: CommandSpec = CommandSpec {
    program: "npm",
    args: &["install", "-g", "@openai/codex"],
};
const NPM_CLAUDE: CommandSpec = CommandSpec {
    program: "npm",
    args: &["install", "-g", "@anthropic-ai/claude-code"],
};
const NPM_OPENCODE: CommandSpec = CommandSpec {
    program: "npm",
    args: &["install", "-g", "opencode-ai"],
};
const NPM_DEEPSEEK: CommandSpec = CommandSpec {
    program: "npm",
    args: &["install", "-g", "deepseek-cli"],
};
const NPM_ZCODE: CommandSpec = CommandSpec {
    program: "npm",
    args: &["install", "-g", "zcode-cli"],
};
const NPM_SUPERAGENT: CommandSpec = CommandSpec {
    program: "npm",
    args: &["install", "-g", "superagent-cli"],
};
const YAY_OPENCODE: CommandSpec = CommandSpec {
    program: "yay",
    args: &["-S", "opencode-bin"],
};
const YAY_ANTIGRAVITY: CommandSpec = CommandSpec {
    program: "yay",
    args: &["-S", "antigravity-bin"],
};

pub fn default_harnesses() -> Vec<HarnessDef> {
    let mut items = vec![
        HarnessDef {
            id: "claude_code",
            name: "Claude Code",
            binary: "claude",
            alt_binaries: &[],
            install_linux: Some(NPM_CLAUDE),
            install_windows: Some(NPM_CLAUDE),
            description: "CLI-агент Anthropic Claude Code",
            kind: HarnessKind::Standard,
            installed: false,
            resolved_binary: None,
        },
        HarnessDef {
            id: "codex",
            name: "Codex",
            binary: "codex",
            alt_binaries: &[],
            install_linux: Some(NPM_CODEX),
            install_windows: Some(NPM_CODEX),
            description: "CLI-агент OpenAI Codex",
            kind: HarnessKind::Standard,
            installed: false,
            resolved_binary: None,
        },
        HarnessDef {
            id: "opencode",
            name: "OpenCode",
            binary: "opencode",
            alt_binaries: &[],
            install_linux: Some(YAY_OPENCODE),
            install_windows: Some(NPM_OPENCODE),
            description: "Открытый TUI-агент для разработки",
            kind: HarnessKind::Standard,
            installed: false,
            resolved_binary: None,
        },
        HarnessDef {
            id: "deepseek_harness",
            name: "DeepAI Code (dsh)",
            binary: "dsh",
            alt_binaries: &["deepseek-cli"],
            install_linux: Some(NPM_DEEPSEEK),
            install_windows: Some(NPM_DEEPSEEK),
            description: "Терминальный агент DeepAI / DeepSeek",
            kind: HarnessKind::Standard,
            installed: false,
            resolved_binary: None,
        },
        HarnessDef {
            id: "zcode",
            name: "ZCode",
            binary: "zcode",
            alt_binaries: &[],
            install_linux: Some(NPM_ZCODE),
            install_windows: Some(NPM_ZCODE),
            description: "Терминальный AI-ассистент zcode",
            kind: HarnessKind::Standard,
            installed: false,
            resolved_binary: None,
        },
        HarnessDef {
            id: "antigravity",
            name: "Antigravity",
            binary: "antigravity",
            alt_binaries: &[],
            install_linux: Some(YAY_ANTIGRAVITY),
            install_windows: None,
            description: "Агентная среда разработки Google DeepMind",
            kind: HarnessKind::Standard,
            installed: false,
            resolved_binary: None,
        },
        HarnessDef {
            id: "superagent",
            name: "SuperAgent",
            binary: "superagent",
            alt_binaries: &[],
            install_linux: Some(NPM_SUPERAGENT),
            install_windows: Some(NPM_SUPERAGENT),
            description: "Автономный мультимодельный CLI-харнесс",
            kind: HarnessKind::Standard,
            installed: false,
            resolved_binary: None,
        },
        HarnessDef {
            id: "netlify_ai",
            name: "Netlify AI",
            binary: "browser",
            alt_binaries: &[],
            install_linux: None,
            install_windows: None,
            description: "Смена Google-профиля, выбор модели и Handoff без API-ключа",
            kind: HarnessKind::NetlifyAi,
            installed: true,
            resolved_binary: None,
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
        match h.kind {
            HarnessKind::NetlifyAi => {
                h.installed = true;
                h.resolved_binary = None;
            }
            HarnessKind::Standard => {
                let found = h.active_binary();
                h.installed = found.is_some();
                h.resolved_binary = found;
            }
        }
    }
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

        if browser != "xdg-open" && browser != "default" && !browser.is_empty() {
            if let Some(browser_exe) = platform::find_in_path(browser) {
                let mut cmd = Command::new(browser_exe);
                let lower = browser.to_ascii_lowercase();
                if lower.contains("firefox") || lower.contains("zen") || lower.contains("librewolf")
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
    }

    platform::open_url(url, &format!("Netlify AI (модель: {model})"))
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

    pub async fn save_and_copy(
        &self,
        active_model: &str,
        active_profile: &str,
    ) -> Result<String, String> {
        let md = self.render_markdown(active_model, active_profile);
        let md_for_write = md.clone();
        tokio::task::spawn_blocking(move || fs::write(Self::FILE_NAME, md_for_write))
            .await
            .map_err(|e| format!("Ошибка записи {}: {e}", Self::FILE_NAME))?
            .map_err(|e| format!("Не удалось записать {}: {e}", Self::FILE_NAME))?;

        match platform::copy_to_clipboard(md).await {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_harness_install_command_uses_noconfirm_or_sh_c() {
        for h in default_harnesses() {
            for spec in [h.install_linux, h.install_windows].into_iter().flatten() {
                assert!(!spec.args.contains(&"--noconfirm"));
                assert_ne!(spec.program, "sh");
            }
        }
    }
}

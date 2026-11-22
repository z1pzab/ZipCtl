use std::process::{Command, Stdio};

use crate::harnesses::which_exists;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Active,
    Inactive,
    Failed,
    Activating,
    NotFound,
    Unknown,
}

impl ServiceState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "Активен",
            Self::Inactive => "Остановлен",
            Self::Failed => "Ошибка",
            Self::Activating => "Запуск...",
            Self::NotFound => "Не найден",
            Self::Unknown => "Неизвестно",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Activating)
    }
}

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub description: String,
    pub state: ServiceState,
    pub enabled: bool,
}

pub fn default_services() -> Vec<String> {
    vec![
        "zapret".to_string(),
        "docker".to_string(),
        "bluetooth".to_string(),
        "NetworkManager".to_string(),
    ]
}

fn friendly_description(name: &str) -> String {
    match name {
        "zapret" => "Обход DPI (zapret)".to_string(),
        "docker" => "Контейнеры Docker".to_string(),
        "bluetooth" => "Стек Bluetooth".to_string(),
        "NetworkManager" => "Сетевые подключения".to_string(),
        "sshd" => "SSH-сервер".to_string(),
        "tailscaled" => "Tailscale VPN".to_string(),
        "libvirtd" => "Виртуализация KVM/QEMU".to_string(),
        "postgresql" => "СУБД PostgreSQL".to_string(),
        other => format!("Юнит systemd ({other})"),
    }
}

pub fn refresh_services(names: &[String]) -> Vec<ServiceStatus> {
    let has_systemctl = which_exists("systemctl");
    names
        .iter()
        .map(|name| {
            if !has_systemctl {
                return ServiceStatus {
                    name: name.clone(),
                    description: friendly_description(name),
                    state: ServiceState::Unknown,
                    enabled: false,
                };
            }

            let state = query_active_state(name);
            let enabled = query_enabled(name);

            ServiceStatus {
                name: name.clone(),
                description: friendly_description(name),
                state,
                enabled,
            }
        })
        .collect()
}

fn query_active_state(name: &str) -> ServiceState {
    let Ok(out) = Command::new("systemctl")
        .args(["is-active", name])
        .output()
    else {
        return ServiceState::Unknown;
    };

    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    match text.as_str() {
        "active" => ServiceState::Active,
        "inactive" | "deactivating" => ServiceState::Inactive,
        "failed" => ServiceState::Failed,
        "activating" | "reloading" => ServiceState::Activating,
        _ => {
            // Проверяем, существует ли юнит вообще
            let load_out = Command::new("systemctl")
                .args(["show", "-p", "LoadState", "--value", name])
                .output();
            if let Ok(lo) = load_out {
                if String::from_utf8_lossy(&lo.stdout).trim() == "not-found" {
                    return ServiceState::NotFound;
                }
            }
            ServiceState::Inactive
        }
    }
}

fn query_enabled(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-enabled", name])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            matches!(s.trim(), "enabled" | "static" | "indirect")
        })
        .unwrap_or(false)
}

pub fn toggle_service(name: &str, currently_active: bool) -> Result<String, String> {
    let action = if currently_active { "stop" } else { "start" };
    run_systemctl_action(name, action)
}

pub fn restart_service(name: &str) -> Result<String, String> {
    run_systemctl_action(name, "restart")
}

fn run_systemctl_action(name: &str, action: &str) -> Result<String, String> {
    if !which_exists("systemctl") {
        return Err("Утилита systemctl не найдена в системе.".to_string());
    }

    let action_ru = match action {
        "start" => "запущен",
        "stop" => "остановлен",
        "restart" => "перезапущен",
        _ => "обновлён",
    };

    // 1. Сначала пробуем pkexec (нативный polkit-диалог в Hyprland)
    if which_exists("pkexec") {
        let out = Command::new("pkexec")
            .args(["systemctl", action, name])
            .output()
            .map_err(|e| format!("Ошибка pkexec для {name}: {e}"))?;

        if out.status.success() {
            return Ok(format!("Сервис «{name}» успешно {action_ru}."));
        }
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !stderr.is_empty() {
            return Err(format!("systemctl {action} {name}: {stderr}"));
        }
    }

    // 2. Пробуем беспарольный sudo -n
    if which_exists("sudo") {
        if let Ok(out) = Command::new("sudo")
            .args(["-n", "systemctl", action, name])
            .output()
        {
            if out.status.success() {
                return Ok(format!("Сервис «{name}» успешно {action_ru}."));
            }
        }
    }

    // 3. Резерв: запускаем в окне Kitty с запросом пароля sudo
    if which_exists("kitty") {
        let cmd = format!(
            "echo '=== systemctl {action} {name} ==='; sudo systemctl {action} {name}; read -rp 'Нажмите Enter для закрытия...'"
        );
        Command::new("kitty")
            .args(["--title", &format!("zipctl: {name}"), "sh", "-c", &cmd])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Не удалось открыть Kitty для systemctl: {e}"))?;

        return Ok(format!(
            "Открыто окно Kitty для команды: sudo systemctl {action} {name}."
        ));
    }

    Err(format!(
        "Недостаточно прав для systemctl {action} {name}. Установите polkit/pkexec или kitty."
    ))
}

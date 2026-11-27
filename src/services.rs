#[cfg(any(target_os = "linux", target_os = "windows"))]
use tokio::process::Command;

#[cfg(any(target_os = "linux", target_os = "windows"))]
use crate::platform;

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

#[derive(Debug, Clone)]
pub enum ServiceActionResult {
    Completed(Result<String, String>),
    NeedsInteractive {
        program: String,
        args: Vec<String>,
        title: String,
    },
}

pub fn default_services() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        vec![
            "wuauserv".to_string(),
            "Spooler".to_string(),
            "W32Time".to_string(),
            "Dhcp".to_string(),
            "Dnscache".to_string(),
            "WinDefend".to_string(),
        ]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![
            "zapret".to_string(),
            "docker".to_string(),
            "bluetooth".to_string(),
            "NetworkManager".to_string(),
        ]
    }
}

fn friendly_description(name: &str) -> String {
    match name {
        // Linux / systemd
        "zapret" => "Обход DPI (zapret)".to_string(),
        "docker" => "Контейнеры Docker".to_string(),
        "bluetooth" => "Стек Bluetooth".to_string(),
        "NetworkManager" => "Сетевые подключения".to_string(),
        "sshd" => "SSH-сервер".to_string(),
        "tailscaled" => "Tailscale VPN".to_string(),
        "libvirtd" => "Виртуализация KVM/QEMU".to_string(),
        "postgresql" => "СУБД PostgreSQL".to_string(),
        // Windows Services
        "wuauserv" => "Центр обновления Windows".to_string(),
        "Spooler" => "Диспетчер печати".to_string(),
        "W32Time" => "Служба времени Windows".to_string(),
        "Dhcp" => "DHCP-клиент".to_string(),
        "Dnscache" => "DNS-клиент".to_string(),
        "WinDefend" => "Антивирусная служба Defender".to_string(),
        other => {
            #[cfg(target_os = "windows")]
            {
                format!("Служба Windows ({other})")
            }
            #[cfg(not(target_os = "windows"))]
            {
                format!("Юнит systemd ({other})")
            }
        }
    }
}

pub async fn refresh_services(names: Vec<String>) -> Vec<ServiceStatus> {
    #[cfg(target_os = "linux")]
    {
        refresh_services_linux(names).await
    }
    #[cfg(target_os = "windows")]
    {
        refresh_services_windows(names).await
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        names
            .into_iter()
            .map(|name| ServiceStatus {
                description: friendly_description(&name),
                name,
                state: ServiceState::Unknown,
                enabled: false,
            })
            .collect()
    }
}

pub async fn toggle_service(name: String, currently_active: bool) -> ServiceActionResult {
    let action = if currently_active { "stop" } else { "start" };
    #[cfg(target_os = "linux")]
    {
        run_systemctl_action(&name, action).await
    }
    #[cfg(target_os = "windows")]
    {
        run_windows_sc_action(&name, action).await
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (name, action);
        ServiceActionResult::Completed(Err(
            "Управление службами не поддерживается на этой ОС.".to_string(),
        ))
    }
}

pub async fn restart_service(name: String) -> ServiceActionResult {
    #[cfg(target_os = "linux")]
    {
        run_systemctl_action(&name, "restart").await
    }
    #[cfg(target_os = "windows")]
    {
        run_windows_sc_restart(&name).await
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = name;
        ServiceActionResult::Completed(Err(
            "Управление службами не поддерживается на этой ОС.".to_string(),
        ))
    }
}

#[cfg(target_os = "linux")]
async fn refresh_services_linux(names: Vec<String>) -> Vec<ServiceStatus> {
    let systemctl_bin = platform::find_in_path("systemctl");
    let mut out = Vec::with_capacity(names.len());

    for name in names {
        let Some(ref bin) = systemctl_bin else {
            out.push(ServiceStatus {
                description: friendly_description(&name),
                name,
                state: ServiceState::Unknown,
                enabled: false,
            });
            continue;
        };

        let state = query_systemd_active_state(bin, &name).await;
        let enabled = query_systemd_enabled(bin, &name).await;

        out.push(ServiceStatus {
            description: friendly_description(&name),
            name,
            state,
            enabled,
        });
    }
    out
}

#[cfg(target_os = "linux")]
async fn query_systemd_active_state(systemctl_bin: &std::path::Path, name: &str) -> ServiceState {
    let Ok(out) = Command::new(systemctl_bin)
        .args(["is-active", name])
        .output()
        .await
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
            let load_out = Command::new(systemctl_bin)
                .args(["show", "-p", "LoadState", "--value", name])
                .output()
                .await;
            if let Ok(lo) = load_out {
                if String::from_utf8_lossy(&lo.stdout).trim() == "not-found" {
                    return ServiceState::NotFound;
                }
            }
            ServiceState::Inactive
        }
    }
}

#[cfg(target_os = "linux")]
async fn query_systemd_enabled(systemctl_bin: &std::path::Path, name: &str) -> bool {
    Command::new(systemctl_bin)
        .args(["is-enabled", name])
        .output()
        .await
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout);
            matches!(s.trim(), "enabled" | "static" | "indirect")
        })
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
async fn run_systemctl_action(name: &str, action: &str) -> ServiceActionResult {
    if platform::find_in_path("systemctl").is_none() {
        return ServiceActionResult::Completed(Err(
            "Утилита systemctl не найдена в PATH.".to_string()
        ));
    }

    let action_ru = match action {
        "start" => "запущен",
        "stop" => "остановлен",
        "restart" => "перезапущен",
        _ => "обновлён",
    };

    if let Some(pkexec) = platform::find_in_path("pkexec") {
        match Command::new(pkexec)
            .args(["systemctl", action, name])
            .output()
            .await
        {
            Ok(out) if out.status.success() => {
                return ServiceActionResult::Completed(Ok(format!(
                    "Сервис «{name}» успешно {action_ru}."
                )));
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                if !stderr.is_empty() && out.status.code() != Some(127) {
                    return ServiceActionResult::Completed(Err(format!(
                        "systemctl {action} {name}: {stderr}"
                    )));
                }
            }
            Err(_) => {}
        }
    }

    if let Some(sudo) = platform::find_in_path("sudo") {
        if let Ok(out) = Command::new(sudo)
            .args(["-n", "systemctl", action, name])
            .output()
            .await
        {
            if out.status.success() {
                return ServiceActionResult::Completed(Ok(format!(
                    "Сервис «{name}» успешно {action_ru}."
                )));
            }
        }

        return ServiceActionResult::NeedsInteractive {
            program: "sudo".to_string(),
            args: vec![
                "systemctl".to_string(),
                action.to_string(),
                name.to_string(),
            ],
            title: format!("sudo systemctl {action} {name}"),
        };
    }

    ServiceActionResult::Completed(Err(format!(
        "Недостаточно прав для systemctl {action} {name}. Установите pkexec или sudo."
    )))
}

#[cfg(target_os = "windows")]
async fn refresh_services_windows(names: Vec<String>) -> Vec<ServiceStatus> {
    let sc_bin = platform::find_in_path("sc.exe");
    let mut out = Vec::with_capacity(names.len());

    for name in names {
        let Some(ref bin) = sc_bin else {
            out.push(ServiceStatus {
                description: friendly_description(&name),
                name,
                state: ServiceState::Unknown,
                enabled: false,
            });
            continue;
        };

        let state = match Command::new(bin).args(["query", &name]).output().await {
            Ok(o) => {
                let text = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                );
                parse_sc_query_state(&text, o.status.code())
            }
            Err(_) => ServiceState::Unknown,
        };

        let enabled = if state == ServiceState::NotFound {
            false
        } else {
            Command::new(bin)
                .args(["qc", &name])
                .output()
                .await
                .map(|o| parse_sc_qc_enabled(&String::from_utf8_lossy(&o.stdout)))
                .unwrap_or(false)
        };

        out.push(ServiceStatus {
            description: friendly_description(&name),
            name,
            state,
            enabled,
        });
    }

    out
}

#[cfg(target_os = "windows")]
async fn run_windows_sc_action(name: &str, action: &str) -> ServiceActionResult {
    let Some(sc_bin) = platform::find_in_path("sc.exe") else {
        return ServiceActionResult::Completed(Err(
            "Утилита sc.exe не найдена в PATH.".to_string()
        ));
    };

    let out = match Command::new(&sc_bin).args([action, name]).output().await {
        Ok(o) => o,
        Err(e) => {
            return ServiceActionResult::Completed(Err(format!(
                "Ошибка запуска sc.exe {action} {name}: {e}"
            )));
        }
    };

    if out.status.success() {
        let action_ru = match action {
            "start" => "запущена",
            "stop" => "остановлена",
            _ => "обновлена",
        };
        return ServiceActionResult::Completed(Ok(format!(
            "Служба Windows «{name}» успешно {action_ru}."
        )));
    }

    let combined = format!(
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let code = out.status.code().unwrap_or(-1);
    if code == 5 || combined.contains(" 5") || combined.to_lowercase().contains("access is denied") {
        return ServiceActionResult::Completed(Err(format!(
            "Отказано в доступе для службы «{name}»: запустите zipctl от имени администратора."
        )));
    }

    ServiceActionResult::Completed(Err(format!(
        "sc.exe {action} {name} (код {code}): {}",
        combined.trim()
    )))
}

#[cfg(target_os = "windows")]
async fn run_windows_sc_restart(name: &str) -> ServiceActionResult {
    let Some(sc_bin) = platform::find_in_path("sc.exe") else {
        return ServiceActionResult::Completed(Err(
            "Утилита sc.exe не найдена в PATH.".to_string()
        ));
    };

    let stop_res = run_windows_sc_action(name, "stop").await;
    if let ServiceActionResult::Completed(Err(err)) = stop_res {
        // Ignore "service not started" (1062) when restarting.
        if !err.contains("1062") {
            return ServiceActionResult::Completed(Err(err));
        }
    }

    // Brief wait for Windows SCM to transition to STOPPED before issuing start.
    for _ in 0..8 {
        if let Ok(q) = Command::new(&sc_bin).args(["query", name]).output().await {
            let st = parse_sc_query_state(&String::from_utf8_lossy(&q.stdout), q.status.code());
            if st == ServiceState::Inactive {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    run_windows_sc_action(name, "start").await
}

/// Locale-independent parser for `sc.exe query <service>` output.
pub fn parse_sc_query_state(output: &str, exit_code: Option<i32>) -> ServiceState {
    if exit_code == Some(1060) || output.contains("1060") {
        return ServiceState::NotFound;
    }

    for line in output.lines() {
        let upper = line.to_ascii_uppercase();
        if upper.contains("RUNNING") {
            return ServiceState::Active;
        }
        if upper.contains("START_PENDING") || upper.contains("STOP_PENDING") {
            return ServiceState::Activating;
        }
        if upper.contains("STOPPED") || upper.contains("PAUSED") {
            return ServiceState::Inactive;
        }
        // Localized Windows builds always keep numeric state after ':' (e.g. "СОСТОЯНИЕ : 4  RUNNING").
        if let Some((_, rhs)) = upper.split_once(':') {
            let token = rhs.split_whitespace().next().unwrap_or("");
            match token {
                "4" => return ServiceState::Active,
                "2" | "3" => return ServiceState::Activating,
                "1" | "7" => return ServiceState::Inactive,
                _ => {}
            }
        }
    }

    ServiceState::Unknown
}

/// Locale-independent parser for `sc.exe qc <service>` output.
pub fn parse_sc_qc_enabled(output: &str) -> bool {
    for line in output.lines() {
        let upper = line.to_ascii_uppercase();
        if upper.contains("AUTO_START") {
            return true;
        }
        if upper.contains("DEMAND_START") || upper.contains("DISABLED") {
            return false;
        }
        if upper.contains("START_TYPE") {
            if let Some((_, rhs)) = upper.split_once(':') {
                let code = rhs.split_whitespace().next().unwrap_or("");
                if code == "2" {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sc_query_in_english_and_localized_output() {
        let running = "SERVICE_NAME: wuauserv\n        STATE              : 4  RUNNING\n";
        assert_eq!(parse_sc_query_state(running, Some(0)), ServiceState::Active);

        let ru_stopped = "ИМЯ_СЛУЖБЫ: Spooler\n        СОСТОЯНИЕ          : 1  STOPPED\n";
        assert_eq!(
            parse_sc_query_state(ru_stopped, Some(0)),
            ServiceState::Inactive
        );

        let ru_numeric_only = "        СОСТОЯНИЕ          : 4  ВЫПОЛНЯЕТСЯ\n";
        assert_eq!(
            parse_sc_query_state(ru_numeric_only, Some(0)),
            ServiceState::Active
        );

        assert_eq!(
            parse_sc_query_state("[SC] EnumQueryServicesStatus:OpenService FAILED 1060:", Some(1060)),
            ServiceState::NotFound
        );
    }

    #[test]
    fn parses_sc_qc_start_type() {
        let auto = "        START_TYPE         : 2   AUTO_START\n";
        let manual = "        START_TYPE         : 3   DEMAND_START\n";
        assert!(parse_sc_qc_enabled(auto));
        assert!(!parse_sc_qc_enabled(manual));
    }
}

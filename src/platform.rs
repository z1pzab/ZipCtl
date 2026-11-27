use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config_patcher::BrowserProfile;

/// Returns a short OS/environment badge for the header bar.
pub fn os_badge() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some_and(|v| !v.is_empty()) {
            "Linux / Hyprland"
        } else {
            "Linux"
        }
    }
    #[cfg(target_os = "windows")]
    {
        "Windows"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        "Unix"
    }
}

pub fn url_opener_label() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "xdg-open"
    }
    #[cfg(target_os = "windows")]
    {
        "explorer"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        "open"
    }
}

pub fn clipboard_label() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "wl-copy / xclip"
    }
    #[cfg(target_os = "windows")]
    {
        "буфер Windows"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        "буфер обмена"
    }
}

pub fn cache_cleanup_hint() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "[c] очистка ~/.cache"
    }
    #[cfg(target_os = "windows")]
    {
        "[c] очистка %TEMP%"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        "[c] очистка кэша"
    }
}

pub fn services_manager_label() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "systemd"
    }
    #[cfg(target_os = "windows")]
    {
        "Windows Services (sc)"
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        "services"
    }
}

pub fn default_browser_profiles() -> Vec<BrowserProfile> {
    #[cfg(target_os = "windows")]
    {
        vec![
            BrowserProfile {
                name: "Google Chrome (Default)".to_string(),
                browser: "chrome".to_string(),
                profile_dir: "Default".to_string(),
            },
            BrowserProfile {
                name: "Google Chrome (Profile 1)".to_string(),
                browser: "chrome".to_string(),
                profile_dir: "Profile 1".to_string(),
            },
            BrowserProfile {
                name: "Microsoft Edge (Default)".to_string(),
                browser: "msedge".to_string(),
                profile_dir: "Default".to_string(),
            },
            BrowserProfile {
                name: "Системный браузер (по умолчанию)".to_string(),
                browser: "default".to_string(),
                profile_dir: String::new(),
            },
        ]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![
            BrowserProfile {
                name: "Основной Google (Default)".to_string(),
                browser: "chromium".to_string(),
                profile_dir: "Default".to_string(),
            },
            BrowserProfile {
                name: "Запасной аккаунт #1 (Profile 1)".to_string(),
                browser: "chromium".to_string(),
                profile_dir: "Profile 1".to_string(),
            },
            BrowserProfile {
                name: "Запасной аккаунт #2 (Profile 2)".to_string(),
                browser: "chromium".to_string(),
                profile_dir: "Profile 2".to_string(),
            },
            BrowserProfile {
                name: "Системный браузер (xdg-open)".to_string(),
                browser: "default".to_string(),
                profile_dir: String::new(),
            },
        ]
    }
}

/// Checks if a file exists and is executable (on Unix checks +x permission).
fn is_executable_file(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.is_file()
            && fs::metadata(path)
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

#[cfg(windows)]
fn windows_pathext_candidates(base: &Path) -> Vec<PathBuf> {
    let mut out = vec![base.to_path_buf()];
    if base.extension().is_none() {
        let pathext =
            env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        for ext in pathext.split(';').filter(|e| !e.trim().is_empty()) {
            let ext_trimmed = ext.trim().trim_start_matches('.');
            out.push(base.with_extension(ext_trimmed));
        }
    }
    out
}

/// Finds an executable by name or explicit path, respecting PATH and Windows PATHEXT.
pub fn find_in_path(bin: &str) -> Option<PathBuf> {
    let trimmed = bin.trim();
    if trimmed.is_empty() {
        return None;
    }

    let as_path = Path::new(trimmed);
    if trimmed.contains('/') || trimmed.contains('\\') || as_path.is_absolute() {
        #[cfg(windows)]
        {
            for cand in windows_pathext_candidates(as_path) {
                if is_executable_file(&cand) {
                    return Some(cand);
                }
            }
            return None;
        }
        #[cfg(not(windows))]
        {
            return is_executable_file(as_path).then(|| as_path.to_path_buf());
        }
    }

    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(trimmed);
        #[cfg(windows)]
        {
            for cand in windows_pathext_candidates(&candidate) {
                if is_executable_file(&cand) {
                    return Some(cand);
                }
            }
        }
        #[cfg(not(windows))]
        {
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

pub fn which_exists(bin: &str) -> bool {
    find_in_path(bin).is_some()
}

/// Opens an http/https URL in the system default browser without spawning a shell.
pub fn open_url(url: &str, label: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(format!(
            "Небезопасный URL для {label}: разрешены только http:// и https://."
        ));
    }

    #[cfg(target_os = "linux")]
    {
        let opener = find_in_path("xdg-open")
            .ok_or_else(|| "Утилита xdg-open не найдена в PATH (установите xdg-utils).".to_string())?;
        Command::new(opener)
            .arg(trimmed)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Не удалось открыть {label} через xdg-open: {e}"))?;
        Ok(format!("Открыт {label} ({trimmed}) через xdg-open."))
    }
    #[cfg(target_os = "windows")]
    {
        let explorer = find_in_path("explorer.exe")
            .ok_or_else(|| "explorer.exe не найден в PATH.".to_string())?;
        Command::new(explorer)
            .arg(trimmed)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Не удалось открыть {label} через explorer.exe: {e}"))?;
        Ok(format!("Открыт {label} ({trimmed}) через explorer.exe."))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let opener = find_in_path("open")
            .ok_or_else(|| format!("Не найден системный обработчик URL для {label}."))?;
        Command::new(opener)
            .arg(trimmed)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Не удалось открыть {label}: {e}"))?;
        Ok(format!("Открыт {label} ({trimmed})."))
    }
}

/// Opens a local directory or file in the system file manager.
pub fn open_path(path: &Path) -> Result<String, String> {
    if !path.exists() {
        return Err(format!("Путь не существует: {}", path.display()));
    }

    #[cfg(target_os = "linux")]
    {
        let opener = find_in_path("xdg-open")
            .ok_or_else(|| "Утилита xdg-open не найдена в PATH.".to_string())?;
        Command::new(opener)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Ошибка xdg-open: {e}"))?;
        Ok(format!("Открыт каталог: {}", path.display()))
    }
    #[cfg(target_os = "windows")]
    {
        let explorer = find_in_path("explorer.exe")
            .ok_or_else(|| "explorer.exe не найден в PATH.".to_string())?;
        Command::new(explorer)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Ошибка explorer.exe: {e}"))?;
        Ok(format!("Открыт каталог в Проводнике: {}", path.display()))
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let opener = find_in_path("open")
            .ok_or_else(|| "Не найден системный файловый менеджер.".to_string())?;
        Command::new(opener)
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Ошибка открытия пути: {e}"))?;
        Ok(format!("Открыт каталог: {}", path.display()))
    }
}

/// Copies UTF-8 text to the OS clipboard asynchronously.
pub async fn copy_to_clipboard(text: String) -> Result<&'static str, String> {
    #[cfg(target_os = "linux")]
    {
        use tokio::io::AsyncWriteExt;

        for (bin, args, label) in [
            ("wl-copy", &[][..], "wl-copy"),
            ("xclip", &["-selection", "clipboard"][..], "xclip"),
            ("xsel", &["--clipboard", "--input"][..], "xsel"),
        ] {
            if let Some(exe) = find_in_path(bin) {
                let mut child = tokio::process::Command::new(exe)
                    .args(args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| format!("{bin}: {e}"))?;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin
                        .write_all(text.as_bytes())
                        .await
                        .map_err(|e| format!("{bin} write: {e}"))?;
                    let _ = stdin.shutdown().await;
                }
                let status = child
                    .wait()
                    .await
                    .map_err(|e| format!("{bin} wait: {e}"))?;
                if status.success() {
                    return Ok(label);
                }
            }
        }
        Err("установите wl-clipboard, xclip или xsel".to_string())
    }
    #[cfg(target_os = "windows")]
    {
        let fallback_text = text.clone();
        let arboard_res = tokio::task::spawn_blocking(move || {
            let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
            clipboard.set_text(text).map_err(|e| e.to_string())?;
            Ok::<&'static str, String>("Windows Clipboard")
        })
        .await;

        if let Ok(Ok(label)) = arboard_res {
            return Ok(label);
        }

        if let Some(clip_exe) = find_in_path("clip.exe") {
            use tokio::io::AsyncWriteExt;
            let mut child = tokio::process::Command::new(clip_exe)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("clip.exe: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(fallback_text.as_bytes())
                    .await
                    .map_err(|e| format!("clip.exe write: {e}"))?;
                let _ = stdin.shutdown().await;
            }
            let status = child
                .wait()
                .await
                .map_err(|e| format!("clip.exe wait: {e}"))?;
            if status.success() {
                return Ok("clip.exe");
            }
        }

        Err("не удалось записать в буфер обмена Windows".to_string())
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = text;
        Err("буфер обмена не поддерживается на этой ОС".to_string())
    }
}

/// Asynchronously cleans user cache (~/.cache on Linux, %TEMP% on Windows) safely.
pub async fn cleanup_cache() -> Result<String, String> {
    tokio::task::spawn_blocking(cleanup_cache_blocking)
        .await
        .map_err(|e| format!("Ошибка фоновой очистки кэша: {e}"))?
}

fn cleanup_cache_blocking() -> Result<String, String> {
    #[cfg(target_os = "linux")]
    let target_dir = dirs::cache_dir().unwrap_or_else(|| crate::config_patcher::home_dir().join(".cache"));
    #[cfg(target_os = "windows")]
    let target_dir = env::temp_dir();
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let target_dir = dirs::cache_dir().unwrap_or_else(env::temp_dir);

    if !target_dir.exists() {
        return Ok(format!(
            "Каталог кэша не найден ({}).",
            target_dir.display()
        ));
    }

    let (removed, skipped, freed_bytes) = remove_dir_contents(&target_dir);
    let freed_mb = freed_bytes as f64 / (1024.0 * 1024.0);

    #[cfg(target_os = "linux")]
    let extra = if which_exists("paccache") {
        " • для пакетного кэша Arch: sudo paccache -rk1"
    } else {
        ""
    };
    #[cfg(not(target_os = "linux"))]
    let extra = "";

    Ok(format!(
        "Очищено объектов в {}: {removed} (~{freed_mb:.1} МБ), пропущено занятых: {skipped}{extra}",
        target_dir.display()
    ))
}

fn entry_size(path: &Path) -> u64 {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.is_file() {
            return meta.len();
        }
        if meta.is_dir() && !meta.file_type().is_symlink() {
            let mut sum = 0u64;
            if let Ok(entries) = fs::read_dir(path) {
                for e in entries.flatten() {
                    sum = sum.saturating_add(entry_size(&e.path()));
                }
            }
            return sum;
        }
    }
    0
}

fn remove_dir_contents(dir: &Path) -> (usize, usize, u64) {
    let mut removed = 0usize;
    let mut skipped = 0usize;
    let mut freed = 0u64;

    let Ok(entries) = fs::read_dir(dir) else {
        return (0, 0, 0);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let size = entry_size(&path);
        let res = if path.is_dir() && !path.is_symlink() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if res.is_ok() {
            removed += 1;
            freed = freed.saturating_add(size);
        } else {
            skipped += 1;
        }
    }

    (removed, skipped, freed)
}

/// Restricts config file permissions to owner read/write (0600) on Unix.
pub fn set_private_file_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Fallback CPU temperature reader from Linux sysfs thermal zones.
pub fn read_cpu_temp_fallback() -> Option<f32> {
    #[cfg(target_os = "linux")]
    {
        let entries = fs::read_dir("/sys/class/thermal").ok()?;
        for entry in entries.flatten() {
            let name = entry.file_name();
            if !name.to_string_lossy().starts_with("thermal_zone") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(entry.path().join("temp")) {
                if let Ok(milli) = raw.trim().parse::<f32>() {
                    let temp = if milli > 1000.0 { milli / 1000.0 } else { milli };
                    if temp > 0.0 && temp <= 130.0 {
                        return Some(temp);
                    }
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Fallback GPU temperature reader from Linux DRM hwmon.
pub fn read_gpu_temp_fallback() -> Option<f32> {
    #[cfg(target_os = "linux")]
    {
        for card in 0..4 {
            let hwmon_dir = format!("/sys/class/drm/card{card}/device/hwmon");
            if let Ok(entries) = fs::read_dir(&hwmon_dir) {
                for entry in entries.flatten() {
                    let temp_path = entry.path().join("temp1_input");
                    if let Ok(raw) = fs::read_to_string(temp_path) {
                        if let Ok(milli) = raw.trim().parse::<f32>() {
                            let temp = if milli > 1000.0 { milli / 1000.0 } else { milli };
                            if temp > 0.0 && temp <= 130.0 {
                                return Some(temp);
                            }
                        }
                    }
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_url_rejects_non_http_schemes() {
        let err = open_url("file:///etc/passwd", "test").unwrap_err();
        assert!(err.contains("http://"));
    }

    #[test]
    fn find_in_path_handles_empty_input() {
        assert!(find_in_path("").is_none());
        assert!(find_in_path("   ").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn find_in_path_discovers_windows_system_binaries() {
        assert!(find_in_path("cmd").is_some());
        assert!(find_in_path("explorer.exe").is_some());
    }
}

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::platform;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderPreset {
    CoarenaAi,
    CodexRelay,
    OpenRouter,
    Official,
    Custom,
}

impl ProviderPreset {
    pub const ALL: [ProviderPreset; 5] = [
        ProviderPreset::CoarenaAi,
        ProviderPreset::CodexRelay,
        ProviderPreset::OpenRouter,
        ProviderPreset::Official,
        ProviderPreset::Custom,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::CoarenaAi => "Coarena AI",
            Self::CodexRelay => "Codex Relay",
            Self::OpenRouter => "OpenRouter",
            Self::Official => "Official",
            Self::Custom => "Custom (Свой)",
        }
    }

    pub fn slug(&self) -> &'static str {
        match self {
            Self::CoarenaAi => "coarena",
            Self::CodexRelay => "codex_relay",
            Self::OpenRouter => "openrouter",
            Self::Official => "official",
            Self::Custom => "custom",
        }
    }

    pub fn default_base_url(&self, harness_id: &str) -> &'static str {
        match self {
            Self::CoarenaAi => "https://api.coarena.ai/v1",
            Self::CodexRelay => "https://relay.codex.sh/v1",
            Self::OpenRouter => "https://openrouter.ai/api/v1",
            Self::Official => match harness_id {
                "claude_code" => "https://api.anthropic.com",
                "deepseek_harness" => "https://api.deepseek.com/v1",
                "antigravity" => "https://generativelanguage.googleapis.com/v1beta",
                _ => "https://api.openai.com/v1",
            },
            Self::Custom => "",
        }
    }

    pub fn default_model(&self, harness_id: &str) -> &'static str {
        match self {
            Self::OpenRouter => "anthropic/claude-sonnet-4",
            Self::CoarenaAi => "claude-sonnet-4-5",
            Self::CodexRelay => "gpt-5-codex",
            Self::Official => match harness_id {
                "claude_code" => "claude-sonnet-4-5",
                "deepseek_harness" => "deepseek-reasoner",
                "antigravity" => "gemini-2.5-pro",
                "codex" => "o4-mini",
                _ => "gpt-4.1",
            },
            Self::Custom => "",
        }
    }

    pub fn next(&self) -> Self {
        let idx = Self::ALL.iter().position(|p| p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(&self) -> Self {
        let idx = Self::ALL.iter().position(|p| p == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn from_slug(slug: &str) -> Self {
        match slug {
            "coarena" => Self::CoarenaAi,
            "codex_relay" => Self::CodexRelay,
            "openrouter" => Self::OpenRouter,
            "official" => Self::Official,
            _ => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserProfile {
    pub name: String,
    pub browser: String,
    pub profile_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetlifyAiConfig {
    pub url: String,
    pub selected_model: usize,
    pub models: Vec<String>,
    pub selected_profile: usize,
    pub profiles: Vec<BrowserProfile>,
}

impl Default for NetlifyAiConfig {
    fn default() -> Self {
        Self {
            url: "https://app.netlify.com".to_string(),
            selected_model: 0,
            models: vec![
                "claude-sonnet-4-5".to_string(),
                "gemini-2.5-pro".to_string(),
                "gpt-5".to_string(),
                "deepseek-r1".to_string(),
            ],
            selected_profile: 0,
            profiles: platform::default_browser_profiles(),
        }
    }
}

impl NetlifyAiConfig {
    pub fn current_model(&self) -> &str {
        self.models
            .get(self.selected_model)
            .map(String::as_str)
            .unwrap_or("claude-sonnet-4-5")
    }

    pub fn current_profile(&self) -> Option<&BrowserProfile> {
        self.profiles.get(self.selected_profile)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessSavedConfig {
    pub provider: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZipCtlConfig {
    #[serde(default = "default_low_space_gb")]
    pub low_space_gb: u64,
    #[serde(default = "default_projects_dir")]
    pub projects_dir: String,
    #[serde(default = "default_services_list")]
    pub services: Vec<String>,
    #[serde(default)]
    pub netlify_ai: NetlifyAiConfig,
    #[serde(default)]
    pub harnesses: BTreeMap<String, HarnessSavedConfig>,
}

fn default_low_space_gb() -> u64 {
    5
}

pub fn default_projects_dir() -> String {
    home_dir().join("Projects").to_string_lossy().into_owned()
}

fn default_services_list() -> Vec<String> {
    crate::services::default_services()
}

impl Default for ZipCtlConfig {
    fn default() -> Self {
        Self {
            low_space_gb: default_low_space_gb(),
            projects_dir: default_projects_dir(),
            services: default_services_list(),
            netlify_ai: NetlifyAiConfig::default(),
            harnesses: BTreeMap::new(),
        }
    }
}

/// Returns the user's home directory cross-platform.
pub fn home_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Returns the OS-standard config directory (`~/.config` on Linux, `%APPDATA%` on Windows).
pub fn config_base_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| home_dir().join(".config"))
}

/// Returns the zipctl config file path (`~/.config/zipctl/config.toml` or `%APPDATA%\zipctl\config.toml`).
pub fn zipctl_config_path() -> PathBuf {
    config_base_dir().join("zipctl").join("config.toml")
}

/// Loads `config.toml` or generates a default one on first launch.
/// Never silently overwrites a corrupted existing config file.
pub fn load_or_create_config() -> (ZipCtlConfig, Option<String>) {
    let path = zipctl_config_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(raw) => match toml::from_str::<ZipCtlConfig>(&raw) {
                Ok(mut cfg) => {
                    if cfg.netlify_ai.models.is_empty() {
                        cfg.netlify_ai.models = NetlifyAiConfig::default().models;
                    }
                    if cfg.netlify_ai.profiles.is_empty() {
                        cfg.netlify_ai.profiles = NetlifyAiConfig::default().profiles;
                    }
                    if cfg.netlify_ai.selected_model >= cfg.netlify_ai.models.len() {
                        cfg.netlify_ai.selected_model = 0;
                    }
                    if cfg.netlify_ai.selected_profile >= cfg.netlify_ai.profiles.len() {
                        cfg.netlify_ai.selected_profile = 0;
                    }
                    if cfg.projects_dir.trim().is_empty() {
                        cfg.projects_dir = default_projects_dir();
                    }
                    return (cfg, None);
                }
                Err(err) => {
                    return (
                        ZipCtlConfig::default(),
                        Some(format!(
                            "Некорректный конфиг {}: {err}. Используются настройки по умолчанию (файл не перезаписан).",
                            path.display()
                        )),
                    );
                }
            },
            Err(err) => {
                return (
                    ZipCtlConfig::default(),
                    Some(format!(
                        "Не удалось прочитать {}: {err}. Используются настройки по умолчанию.",
                        path.display()
                    )),
                );
            }
        }
    }

    let cfg = ZipCtlConfig::default();
    let warn = save_config(&cfg).err();
    (cfg, warn)
}

pub fn save_config(cfg: &ZipCtlConfig) -> Result<(), String> {
    let path = zipctl_config_path();
    ensure_parent(&path)?;
    let _ = backup_file_if_exists(&path)?;
    let body = toml::to_string_pretty(cfg)
        .map_err(|e| format!("Ошибка сериализации config.toml: {e}"))?;
    fs::write(&path, body).map_err(|e| format!("Ошибка записи {}: {e}", path.display()))?;
    platform::set_private_file_permissions(&path);
    Ok(())
}

/// Resolves the target config file path for a harness on the current OS.
pub fn harness_config_path(harness_id: &str) -> Option<PathBuf> {
    let home = home_dir();
    let cfg = config_base_dir();
    match harness_id {
        "codex" => Some(home.join(".codex").join("config.toml")),
        "claude_code" => Some(home.join(".claude.json")),
        "opencode" => Some(cfg.join("opencode").join("config.json")),
        "deepseek_harness" => Some(cfg.join("deepseek-harness").join("config.toml")),
        "zcode" => Some(cfg.join("zcode").join("config.json")),
        "antigravity" => Some(cfg.join("antigravity").join("config.json")),
        "superagent" => Some(cfg.join("superagent").join("config.json")),
        "netlify_ai" => Some(zipctl_config_path()),
        _ => None,
    }
}

#[derive(Debug, Clone)]
pub struct PatchResult {
    pub path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

/// Creates a `<filename>.bak` copy if `path` already exists.
pub fn backup_file_if_exists(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let mut bak_os = OsString::from(path.as_os_str());
    bak_os.push(".bak");
    let bak_path = PathBuf::from(bak_os);
    fs::copy(path, &bak_path).map_err(|e| {
        format!(
            "Не удалось создать резервную копию {}: {e}",
            bak_path.display()
        )
    })?;
    platform::set_private_file_permissions(&bak_path);
    Ok(Some(bak_path))
}

/// Masks an API key so raw secrets are never shown in plain text by default.
pub fn mask_api_key(key: &str) -> String {
    let trimmed = key.trim();
    let count = trimmed.chars().count();
    if count == 0 {
        return String::new();
    }
    if count <= 8 {
        return "•".repeat(count);
    }
    let prefix: String = trimmed.chars().take(3).collect();
    let suffix: String = trimmed.chars().skip(count - 3).collect();
    let hidden = "•".repeat((count - 6).min(12));
    format!("{prefix}{hidden}{suffix}")
}

/// Inspects an existing harness config and returns a masked representation of any stored API key.
pub fn detect_existing_api_key_masked(harness_id: &str) -> Option<String> {
    let path = harness_config_path(harness_id)?;
    let raw = fs::read_to_string(&path).ok()?;

    let found: Option<String> = match harness_id {
        "codex" => {
            let tbl = toml::from_str::<toml::Value>(&raw)
                .ok()
                .and_then(|v| v.as_table().cloned())?;
            let active = tbl
                .get("model_provider")
                .and_then(|v| v.as_str())
                .unwrap_or("coarena");
            tbl.get("model_providers")
                .and_then(|v| v.as_table())
                .and_then(|mp| mp.get(active))
                .and_then(|v| v.as_table())
                .and_then(|entry| entry.get("api_key"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        }
        "claude_code" => {
            let obj = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v.as_object().cloned())?;
            obj.get("env")
                .and_then(|v| v.as_object())
                .and_then(|e| e.get("ANTHROPIC_API_KEY"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        }
        "opencode" => {
            let obj = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v.as_object().cloned())?;
            let providers = obj.get("provider").and_then(|v| v.as_object())?;
            providers.values().find_map(|entry| {
                entry
                    .get("options")
                    .and_then(|o| o.get("apiKey"))
                    .and_then(|k| k.as_str())
                    .map(str::to_string)
            })
        }
        "deepseek_harness" => toml::from_str::<toml::Value>(&raw)
            .ok()
            .and_then(|v| v.as_table().cloned())
            .and_then(|t| t.get("api_key").and_then(|k| k.as_str()).map(str::to_string)),
        "antigravity" | "superagent" | "zcode" => serde_json::from_str::<serde_json::Value>(&raw)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .and_then(|o| o.get("api_key").and_then(|k| k.as_str()).map(str::to_string)),
        _ => None,
    };

    found
        .map(|s| mask_api_key(&s))
        .filter(|masked| !masked.is_empty())
}

pub fn patch_harness_config(
    harness_id: &str,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<PatchResult, String> {
    let path = harness_config_path(harness_id)
        .ok_or_else(|| format!("Неизвестный харнесс: {harness_id}"))?;

    let backup_path = match harness_id {
        "codex" => patch_codex_toml(&path, provider, base_url, model, api_key)?,
        "claude_code" => patch_claude_json(&path, provider, base_url, model, api_key)?,
        "opencode" => patch_opencode_json(&path, provider, base_url, model, api_key)?,
        "deepseek_harness" => patch_generic_toml(&path, provider, base_url, model, api_key)?,
        "antigravity" | "superagent" | "zcode" => {
            patch_generic_json(&path, provider, base_url, model, api_key)?
        }
        _ => return Err(format!("Харнесс {harness_id} не использует внешний API-конфиг.")),
    };

    Ok(PatchResult { path, backup_path })
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Не удалось создать каталог {}: {e}", parent.display()))?;
    }
    Ok(())
}

fn read_toml_table(path: &Path) -> toml::map::Map<String, toml::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
        .and_then(|v| v.as_table().cloned())
        .unwrap_or_default()
}

fn write_toml_table(
    path: &Path,
    table: toml::map::Map<String, toml::Value>,
) -> Result<Option<PathBuf>, String> {
    ensure_parent(path)?;
    let backup = backup_file_if_exists(path)?;
    let content = toml::to_string_pretty(&toml::Value::Table(table))
        .map_err(|e| format!("Ошибка сериализации TOML: {e}"))?;
    fs::write(path, content).map_err(|e| format!("Ошибка записи {}: {e}", path.display()))?;
    platform::set_private_file_permissions(path);
    Ok(backup)
}

fn read_json_object(path: &Path) -> serde_json::Map<String, serde_json::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn write_json_object(
    path: &Path,
    obj: serde_json::Map<String, serde_json::Value>,
) -> Result<Option<PathBuf>, String> {
    ensure_parent(path)?;
    let backup = backup_file_if_exists(path)?;
    let content = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| format!("Ошибка сериализации JSON: {e}"))?;
    fs::write(path, content).map_err(|e| format!("Ошибка записи {}: {e}", path.display()))?;
    platform::set_private_file_permissions(path);
    Ok(backup)
}

fn patch_codex_toml(
    path: &Path,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<Option<PathBuf>, String> {
    let mut root = read_toml_table(path);
    let slug = provider.slug().to_string();

    if !model.is_empty() {
        root.insert("model".to_string(), toml::Value::String(model.to_string()));
    }
    root.insert("model_provider".to_string(), toml::Value::String(slug.clone()));

    let mut providers = root
        .get("model_providers")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();

    let mut entry = providers
        .get(&slug)
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();

    entry.insert(
        "name".to_string(),
        toml::Value::String(provider.label().to_string()),
    );
    if !base_url.is_empty() {
        entry.insert(
            "base_url".to_string(),
            toml::Value::String(base_url.to_string()),
        );
    }
    if !api_key.is_empty() {
        entry.insert(
            "api_key".to_string(),
            toml::Value::String(api_key.to_string()),
        );
    }
    entry.insert(
        "wire_api".to_string(),
        toml::Value::String("responses".to_string()),
    );

    providers.insert(slug, toml::Value::Table(entry));
    root.insert("model_providers".to_string(), toml::Value::Table(providers));

    write_toml_table(path, root)
}

fn patch_claude_json(
    path: &Path,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<Option<PathBuf>, String> {
    let mut root = read_json_object(path);
    let mut env_obj = root
        .get("env")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    if !base_url.is_empty() {
        env_obj.insert(
            "ANTHROPIC_BASE_URL".to_string(),
            serde_json::Value::String(base_url.to_string()),
        );
    }
    if !api_key.is_empty() {
        env_obj.insert(
            "ANTHROPIC_API_KEY".to_string(),
            serde_json::Value::String(api_key.to_string()),
        );
    }
    if !model.is_empty() {
        env_obj.insert(
            "ANTHROPIC_MODEL".to_string(),
            serde_json::Value::String(model.to_string()),
        );
    }

    root.insert("env".to_string(), serde_json::Value::Object(env_obj));
    root.insert(
        "zipctl_provider".to_string(),
        serde_json::Value::String(provider.label().to_string()),
    );

    write_json_object(path, root)
}

fn patch_opencode_json(
    path: &Path,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<Option<PathBuf>, String> {
    let mut root = read_json_object(path);
    let slug = provider.slug().to_string();

    if !model.is_empty() {
        root.insert(
            "model".to_string(),
            serde_json::Value::String(format!("{slug}/{model}")),
        );
    }

    let mut providers = root
        .get("provider")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut entry = providers
        .get(&slug)
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut options = entry
        .get("options")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    if !base_url.is_empty() {
        options.insert(
            "baseURL".to_string(),
            serde_json::Value::String(base_url.to_string()),
        );
    }
    if !api_key.is_empty() {
        options.insert(
            "apiKey".to_string(),
            serde_json::Value::String(api_key.to_string()),
        );
    }

    entry.insert(
        "name".to_string(),
        serde_json::Value::String(provider.label().to_string()),
    );
    entry.insert("options".to_string(), serde_json::Value::Object(options));

    providers.insert(slug, serde_json::Value::Object(entry));
    root.insert("provider".to_string(), serde_json::Value::Object(providers));

    write_json_object(path, root)
}

fn patch_generic_json(
    path: &Path,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<Option<PathBuf>, String> {
    let mut root = read_json_object(path);
    root.insert(
        "provider".to_string(),
        serde_json::Value::String(provider.label().to_string()),
    );
    if !base_url.is_empty() {
        root.insert(
            "base_url".to_string(),
            serde_json::Value::String(base_url.to_string()),
        );
    }
    if !model.is_empty() {
        root.insert(
            "model".to_string(),
            serde_json::Value::String(model.to_string()),
        );
    }
    if !api_key.is_empty() {
        root.insert(
            "api_key".to_string(),
            serde_json::Value::String(api_key.to_string()),
        );
    }
    write_json_object(path, root)
}

fn patch_generic_toml(
    path: &Path,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<Option<PathBuf>, String> {
    let mut root = read_toml_table(path);
    root.insert(
        "provider".to_string(),
        toml::Value::String(provider.label().to_string()),
    );
    if !base_url.is_empty() {
        root.insert(
            "base_url".to_string(),
            toml::Value::String(base_url.to_string()),
        );
    }
    if !model.is_empty() {
        root.insert("model".to_string(), toml::Value::String(model.to_string()));
    }
    if !api_key.is_empty() {
        root.insert(
            "api_key".to_string(),
            toml::Value::String(api_key.to_string()),
        );
    }
    write_toml_table(path, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_api_key_hides_secret() {
        assert_eq!(mask_api_key(""), "");
        assert_eq!(mask_api_key("12345"), "•••••");
        let masked = mask_api_key("sk-ant-api03-very-secret-key-9876");
        assert!(masked.starts_with("sk-"));
        assert!(masked.ends_with("876"));
        assert!(!masked.contains("very-secret"));
    }

    #[test]
    fn zipctl_config_path_uses_dirs_without_hardcoded_linux_mnt() {
        let cfg = ZipCtlConfig::default();
        assert!(!cfg.projects_dir.contains("/mnt/hdd"));
        let path = zipctl_config_path();
        assert!(path.ends_with(Path::new("zipctl").join("config.toml")));
    }

    #[test]
    fn patching_creates_bak_and_preserves_existing_fields() {
        let tmp = std::env::temp_dir().join(format!("zipctl_patch_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let file = tmp.join("claude.json");

        fs::write(&file, r#"{"custom_flag": true, "env": {"EXISTING": "1"}}"#).unwrap();

        let bak = patch_claude_json(
            &file,
            ProviderPreset::OpenRouter,
            "https://openrouter.ai/api/v1",
            "claude-sonnet-4",
            "sk-or-test-123456",
        )
        .unwrap();

        assert!(bak.is_some());
        let bak_path = bak.unwrap();
        assert!(bak_path.exists());
        assert!(fs::read_to_string(&bak_path).unwrap().contains("custom_flag"));

        let updated = fs::read_to_string(&file).unwrap();
        assert!(updated.contains("custom_flag"));
        assert!(updated.contains("EXISTING"));
        assert!(updated.contains("ANTHROPIC_API_KEY"));

        let _ = fs::remove_dir_all(&tmp);
    }
}

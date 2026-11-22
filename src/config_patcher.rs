use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

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
            profiles: vec![
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
                    browser: "xdg-open".to_string(),
                    profile_dir: String::new(),
                },
            ],
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
    #[serde(default)]
    pub netlify_ai: NetlifyAiConfig,
    #[serde(default)]
    pub harnesses: BTreeMap<String, HarnessSavedConfig>,
}

fn default_low_space_gb() -> u64 {
    5
}

impl Default for ZipCtlConfig {
    fn default() -> Self {
        Self {
            low_space_gb: default_low_space_gb(),
            netlify_ai: NetlifyAiConfig::default(),
            harnesses: BTreeMap::new(),
        }
    }
}

pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

pub fn zipctl_config_path() -> PathBuf {
    home_dir().join(".config").join("zipctl").join("config.toml")
}

pub fn load_or_create_config() -> ZipCtlConfig {
    let path = zipctl_config_path();
    if let Ok(raw) = fs::read_to_string(&path) {
        if let Ok(mut cfg) = toml::from_str::<ZipCtlConfig>(&raw) {
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
            return cfg;
        }
    }

    let cfg = ZipCtlConfig::default();
    let _ = save_config(&cfg);
    cfg
}

pub fn save_config(cfg: &ZipCtlConfig) -> Result<(), String> {
    let path = zipctl_config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Не удалось создать каталог: {e}"))?;
    }
    let body = toml::to_string_pretty(cfg)
        .map_err(|e| format!("Ошибка сериализации config.toml: {e}"))?;
    fs::write(&path, body).map_err(|e| format!("Ошибка записи {}: {e}", path.display()))
}

pub fn patch_harness_config(
    harness_id: &str,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<PathBuf, String> {
    let home = home_dir();
    match harness_id {
        "codex" => {
            let path = home.join(".codex").join("config.toml");
            patch_codex_toml(&path, provider, base_url, model, api_key)?;
            Ok(path)
        }
        "claude_code" => {
            let path = home.join(".claude.json");
            patch_claude_json(&path, provider, base_url, model, api_key)?;
            Ok(path)
        }
        "opencode" => {
            let path = home.join(".config").join("opencode").join("config.json");
            patch_opencode_json(&path, provider, base_url, model, api_key)?;
            Ok(path)
        }
        "antigravity" => {
            let path = home.join(".config").join("antigravity").join("config.json");
            patch_generic_json(&path, provider, base_url, model, api_key)?;
            Ok(path)
        }
        "deepseek_harness" => {
            let path = home
                .join(".config")
                .join("deepseek-harness")
                .join("config.toml");
            patch_generic_toml(&path, provider, base_url, model, api_key)?;
            Ok(path)
        }
        "superagent" => {
            let path = home.join(".config").join("superagent").join("config.json");
            patch_generic_json(&path, provider, base_url, model, api_key)?;
            Ok(path)
        }
        _ => Err(format!("Неизвестный харнесс: {harness_id}")),
    }
}

fn ensure_parent(path: &PathBuf) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Не удалось создать каталог {}: {e}", parent.display()))?;
    }
    Ok(())
}

fn read_toml_table(path: &PathBuf) -> toml::map::Map<String, toml::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str::<toml::Value>(&s).ok())
        .and_then(|v| v.as_table().cloned())
        .unwrap_or_default()
}

fn write_toml_table(
    path: &PathBuf,
    table: toml::map::Map<String, toml::Value>,
) -> Result<(), String> {
    ensure_parent(path)?;
    let content = toml::to_string_pretty(&toml::Value::Table(table))
        .map_err(|e| format!("Ошибка сериализации TOML: {e}"))?;
    fs::write(path, content).map_err(|e| format!("Ошибка записи {}: {e}", path.display()))
}

fn read_json_object(path: &PathBuf) -> serde_json::Map<String, serde_json::Value> {
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn write_json_object(
    path: &PathBuf,
    obj: serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    ensure_parent(path)?;
    let content = serde_json::to_string_pretty(&serde_json::Value::Object(obj))
        .map_err(|e| format!("Ошибка сериализации JSON: {e}"))?;
    fs::write(path, content).map_err(|e| format!("Ошибка записи {}: {e}", path.display()))
}

fn patch_codex_toml(
    path: &PathBuf,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<(), String> {
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
    path: &PathBuf,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<(), String> {
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
    path: &PathBuf,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<(), String> {
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

    let mut options = serde_json::Map::new();
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

    let mut entry = serde_json::Map::new();
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
    path: &PathBuf,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<(), String> {
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
    path: &PathBuf,
    provider: ProviderPreset,
    base_url: &str,
    model: &str,
    api_key: &str,
) -> Result<(), String> {
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

use std::time::{Duration, Instant};

use crate::config_patcher::ProviderPreset;

#[derive(Debug, Clone)]
pub struct ApiTestResult {
    pub ok: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u128,
    pub message: String,
}

pub fn test_api_connection(
    _provider: ProviderPreset,
    base_url: &str,
    api_key: &str,
) -> ApiTestResult {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return ApiTestResult {
            ok: false,
            status_code: None,
            latency_ms: 0,
            message: "Укажите Base URL перед проверкой.".to_string(),
        };
    }

    let key = api_key.trim();
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(6))
        .build();

    let (target_url, req) = if trimmed.contains("anthropic.com") {
        let url = if trimmed.ends_with("/v1") {
            format!("{trimmed}/models")
        } else {
            format!("{trimmed}/v1/models")
        };
        let mut r = agent
            .get(&url)
            .set("anthropic-version", "2023-06-01")
            .set("User-Agent", "zipctl/0.2");
        if !key.is_empty() {
            r = r.set("x-api-key", key);
        }
        (url, r)
    } else if trimmed.contains("generativelanguage.googleapis.com") {
        let url = if key.is_empty() {
            format!("{trimmed}/models")
        } else {
            format!("{trimmed}/models?key={key}")
        };
        let r = agent.get(&url).set("User-Agent", "zipctl/0.2");
        (url, r)
    } else {
        let url = if trimmed.ends_with("/models") {
            trimmed.to_string()
        } else {
            format!("{trimmed}/models")
        };
        let mut r = agent.get(&url).set("User-Agent", "zipctl/0.2");
        if !key.is_empty() {
            r = r.set("Authorization", &format!("Bearer {key}"));
        }
        (url, r)
    };

    let started = Instant::now();
    match req.call() {
        Ok(resp) => {
            let code = resp.status();
            let ms = started.elapsed().as_millis();
            ApiTestResult {
                ok: (200..300).contains(&code),
                status_code: Some(code),
                latency_ms: ms,
                message: format!("{code} OK • {ms} мс — подключение успешно"),
            }
        }
        Err(ureq::Error::Status(code, _)) => {
            let ms = started.elapsed().as_millis();
            let detail = match code {
                401 | 403 => "неверный или отсутствующий API-ключ",
                404 => "эндпоинт /models не найден (проверьте Base URL)",
                429 => "превышена квота / лимит запросов (Rate Limit)",
                500..=599 => "ошибка на стороне провайдера",
                _ => "отклонён сервером",
            };
            ApiTestResult {
                ok: false,
                status_code: Some(code),
                latency_ms: ms,
                message: format!("HTTP {code} • {ms} мс — {detail}"),
            }
        }
        Err(ureq::Error::Transport(err)) => {
            let ms = started.elapsed().as_millis();
            ApiTestResult {
                ok: false,
                status_code: None,
                latency_ms: ms,
                message: format!("Ошибка сети ({ms} мс) [{target_url}]: {err}"),
            }
        }
    }
}

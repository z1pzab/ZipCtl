use std::time::{Duration, Instant};

use crate::config_patcher::ProviderPreset;

#[derive(Debug, Clone)]
pub struct ApiTestResult {
    pub ok: bool,
    pub status_code: Option<u16>,
    pub latency_ms: u128,
    pub message: String,
}

pub async fn test_api_connection_async(
    provider: ProviderPreset,
    base_url: String,
    api_key: String,
) -> ApiTestResult {
    tokio::task::spawn_blocking(move || test_api_connection(provider, &base_url, &api_key))
        .await
        .unwrap_or_else(|e| ApiTestResult {
            ok: false,
            status_code: None,
            latency_ms: 0,
            message: format!("Внутренняя ошибка задачи проверки API: {e}"),
        })
}

pub fn build_target_endpoint(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.contains("anthropic.com") {
        if trimmed.ends_with("/v1") {
            format!("{trimmed}/models")
        } else {
            format!("{trimmed}/v1/models")
        }
    } else if trimmed.ends_with("/models") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/models")
    }
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

    let target_url = build_target_endpoint(trimmed);
    let req = if trimmed.contains("anthropic.com") {
        let mut r = agent
            .get(&target_url)
            .set("anthropic-version", "2023-06-01")
            .set("User-Agent", "zipctl/0.2");
        if !key.is_empty() {
            r = r.set("x-api-key", key);
        }
        r
    } else if trimmed.contains("generativelanguage.googleapis.com") {
        // Always pass Gemini API keys via header instead of URL query params to avoid leaking secrets.
        let mut r = agent.get(&target_url).set("User-Agent", "zipctl/0.2");
        if !key.is_empty() {
            r = r.set("x-goog-api-key", key);
        }
        r
    } else {
        let mut r = agent.get(&target_url).set("User-Agent", "zipctl/0.2");
        if !key.is_empty() {
            r = r.set("Authorization", &format!("Bearer {key}"));
        }
        r
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
            let err_text = if key.is_empty() {
                err.to_string()
            } else {
                err.to_string().replace(key, "***")
            };
            ApiTestResult {
                ok: false,
                status_code: None,
                latency_ms: ms,
                message: format!("Ошибка сети ({ms} мс) [{target_url}]: {err_text}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_base_url() {
        let res = test_api_connection(ProviderPreset::Custom, "   ", "secret");
        assert!(!res.ok);
        assert!(res.message.contains("Base URL"));
    }

    #[test]
    fn gemini_endpoint_never_includes_key_query_param() {
        let ep = build_target_endpoint("https://generativelanguage.googleapis.com/v1beta/");
        assert_eq!(ep, "https://generativelanguage.googleapis.com/v1beta/models");
        assert!(!ep.contains("?key="));
    }
}

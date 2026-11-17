use reqwest::header::HeaderMap;
use serde::Deserialize;

/// A failed GitHub REST call, classified so the HTTP layer can map it to a precise status.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum GitHubError {
    #[error("GitHub rejected the access token")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Gone(String),
    #[error("{0}")]
    Validation(String),
    #[error("GitHub API rate limit exceeded; retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },
    #[error("GitHub returned HTTP {status}: {message}")]
    Upstream { status: u16, message: String },
    #[error("GitHub request timed out")]
    Timeout,
    #[error("could not reach GitHub: {0}")]
    Network(String),
    #[error("unexpected response from GitHub: {0}")]
    Decode(String),
}

impl From<reqwest::Error> for GitHubError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            GitHubError::Timeout
        } else if e.is_decode() {
            GitHubError::Decode(e.without_url().to_string())
        } else {
            GitHubError::Network(e.without_url().to_string())
        }
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
    errors: Option<Vec<serde_json::Value>>,
}

/// Turns a non-success GitHub response into a [`GitHubError`].
pub(crate) fn classify(status: u16, headers: &HeaderMap, body: &[u8], now_unix: u64) -> GitHubError {
    let parsed: Option<ErrorBody> = serde_json::from_slice(body).ok();
    let message = parsed
        .as_ref()
        .and_then(|b| b.message.clone())
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| {
            reqwest::StatusCode::from_u16(status)
                .ok()
                .and_then(|s| s.canonical_reason())
                .unwrap_or("Unknown error")
                .to_owned()
        });
    let details = parsed
        .and_then(|b| b.errors)
        .unwrap_or_default()
        .iter()
        .map(describe_validation_error)
        .filter(|d| !d.is_empty())
        .collect::<Vec<_>>()
        .join("; ");

    match status {
        401 => GitHubError::Unauthorized,
        403 | 429 => match rate_limit_wait(headers, &message, now_unix) {
            Some(retry_after_secs) => GitHubError::RateLimited { retry_after_secs },
            None if status == 429 => GitHubError::RateLimited { retry_after_secs: 60 },
            None => GitHubError::Forbidden(message),
        },
        404 => GitHubError::NotFound(message),
        410 => GitHubError::Gone(message),
        422 if details.is_empty() => GitHubError::Validation(message),
        422 => GitHubError::Validation(format!("{message}: {details}")),
        _ => GitHubError::Upstream { status, message },
    }
}

fn describe_validation_error(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => {
            if let Some(msg) = map.get("message").and_then(|m| m.as_str()) {
                return msg.to_owned();
            }
            let field = map.get("field").and_then(|v| v.as_str()).unwrap_or("");
            let code = map.get("code").and_then(|v| v.as_str()).unwrap_or("");
            let value = map.get("value").map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            });
            match value {
                Some(v) if !field.is_empty() => format!("{field} '{v}' is {code}"),
                _ => format!("{field} {code}").trim().to_owned(),
            }
        }
        _ => String::new(),
    }
}

/// Seconds to wait when the response signals a primary or secondary rate limit.
fn rate_limit_wait(headers: &HeaderMap, message: &str, now_unix: u64) -> Option<u64> {
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim);

    if let Some(secs) = header("retry-after").and_then(|v| v.parse::<u64>().ok()) {
        return Some(secs.max(1));
    }
    if header("x-ratelimit-remaining") == Some("0") {
        let reset = header("x-ratelimit-reset").and_then(|v| v.parse::<u64>().ok());
        return Some(reset.map(|r| r.saturating_sub(now_unix)).unwrap_or(60).max(1));
    }
    if message.to_ascii_lowercase().contains("rate limit") {
        return Some(60);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(*k, HeaderValue::from_static(v));
        }
        h
    }

    #[test]
    fn unauthorized() {
        assert_eq!(
            classify(401, &HeaderMap::new(), br#"{"message":"Bad credentials"}"#, 0),
            GitHubError::Unauthorized
        );
    }

    #[test]
    fn primary_rate_limit_uses_reset_header() {
        let h = headers(&[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1100")]);
        assert_eq!(
            classify(403, &h, br#"{"message":"API rate limit exceeded"}"#, 1000),
            GitHubError::RateLimited { retry_after_secs: 100 }
        );
    }

    #[test]
    fn secondary_rate_limit_uses_retry_after() {
        let h = headers(&[("retry-after", "30")]);
        assert_eq!(
            classify(403, &h, b"{}", 0),
            GitHubError::RateLimited { retry_after_secs: 30 }
        );
    }

    #[test]
    fn plain_forbidden_is_not_rate_limited() {
        let h = headers(&[("x-ratelimit-remaining", "4999")]);
        assert_eq!(
            classify(403, &h, br#"{"message":"Must have admin rights to Repository."}"#, 0),
            GitHubError::Forbidden("Must have admin rights to Repository.".into())
        );
    }

    #[test]
    fn validation_errors_are_detailed() {
        let body = br#"{"message":"Validation Failed","errors":[{"resource":"Issue","code":"invalid","field":"assignees","value":"ghost"}]}"#;
        assert_eq!(
            classify(422, &HeaderMap::new(), body, 0),
            GitHubError::Validation("Validation Failed: assignees 'ghost' is invalid".into())
        );
    }

    #[test]
    fn non_json_body_falls_back_to_reason_phrase() {
        assert_eq!(
            classify(502, &HeaderMap::new(), b"<html>", 0),
            GitHubError::Upstream {
                status: 502,
                message: "Bad Gateway".into()
            }
        );
    }

    #[test]
    fn gone_and_not_found() {
        assert_eq!(
            classify(
                410,
                &HeaderMap::new(),
                br#"{"message":"Issues are disabled for this repo"}"#,
                0
            ),
            GitHubError::Gone("Issues are disabled for this repo".into())
        );
        assert_eq!(
            classify(404, &HeaderMap::new(), b"", 0),
            GitHubError::NotFound("Not Found".into())
        );
    }
}

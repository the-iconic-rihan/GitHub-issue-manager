//! Centralised API error type. Every error response has the shape
//! `{"error": {"code": "...", "message": "...", "details": [...]?}}`.

use actix_web::{
    error::{JsonPayloadError, PathError, QueryPayloadError},
    http::{header, StatusCode},
    HttpRequest, HttpResponse, ResponseError,
};
use middleware::github::GitHubError;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldError {
    pub field: String,
    pub message: String,
}

impl FieldError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("request validation failed")]
    Validation(Vec<FieldError>),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Gone(String),
    #[error("{0}")]
    UpstreamValidation(String),
    #[error("GitHub API rate limit exceeded. Try again in {retry_after_secs} seconds.")]
    RateLimited { retry_after_secs: u64 },
    #[error("{0}")]
    BadGateway(String),
    #[error("GitHub did not respond in time")]
    GatewayTimeout,
    #[error("{0}")]
    Unavailable(String),
    /// The message is logged, never returned to the client.
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorPayload<'a>,
}

#[derive(Serialize)]
struct ErrorPayload<'a> {
    code: &'a str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a [FieldError]>,
}

impl ApiError {
    pub fn validation(field: &str, message: &str) -> Self {
        ApiError::Validation(vec![FieldError::new(field, message)])
    }

    /// Message safe to show clients: internal details are logged, never returned.
    pub fn public_message(&self) -> String {
        match self {
            ApiError::Internal(detail) => {
                tracing::error!(error = %detail, "internal error");
                "An unexpected error occurred.".to_owned()
            }
            other => other.to_string(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Validation(_) => "validation_failed",
            ApiError::Unauthorized(_) => "unauthorized",
            ApiError::Forbidden(_) => "forbidden",
            ApiError::NotFound(_) => "not_found",
            ApiError::Conflict(_) => "conflict",
            ApiError::Gone(_) => "gone",
            ApiError::UpstreamValidation(_) => "github_validation_failed",
            ApiError::RateLimited { .. } => "rate_limited",
            ApiError::BadGateway(_) => "github_unavailable",
            ApiError::GatewayTimeout => "github_timeout",
            ApiError::Unavailable(_) => "service_unavailable",
            ApiError::Internal(_) => "internal_error",
        }
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Validation(_) | ApiError::UpstreamValidation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Gone(_) => StatusCode::GONE,
            ApiError::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::BadGateway(_) => StatusCode::BAD_GATEWAY,
            ApiError::GatewayTimeout => StatusCode::GATEWAY_TIMEOUT,
            ApiError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();
        if status.is_server_error() && !matches!(self, ApiError::Internal(_)) {
            tracing::warn!(error = %self, status = status.as_u16(), "upstream error");
        }
        let message = self.public_message();
        let details = match self {
            ApiError::Validation(fields) => Some(fields.as_slice()),
            _ => None,
        };

        let mut response = HttpResponse::build(status);
        if let ApiError::RateLimited { retry_after_secs } = self {
            response.insert_header((header::RETRY_AFTER, retry_after_secs.to_string()));
        }
        if let ApiError::Unauthorized(_) = self {
            response.insert_header((header::WWW_AUTHENTICATE, "Bearer"));
        }
        response.json(ErrorBody {
            error: ErrorPayload {
                code: self.code(),
                message,
                details,
            },
        })
    }
}

impl From<GitHubError> for ApiError {
    fn from(e: GitHubError) -> Self {
        match e {
            GitHubError::Unauthorized => {
                ApiError::Unauthorized("Your GitHub session is invalid or has expired. Please sign in again.".into())
            }
            GitHubError::Forbidden(msg) => ApiError::Forbidden(format!("GitHub denied the request: {msg}")),
            GitHubError::NotFound(msg) => ApiError::NotFound(format!(
                "Not found on GitHub ({msg}). It may not exist, or you may not have access to it."
            )),
            GitHubError::Gone(msg) => ApiError::Gone(msg),
            GitHubError::Validation(msg) => ApiError::UpstreamValidation(format!("GitHub rejected the change: {msg}")),
            GitHubError::RateLimited { retry_after_secs } => ApiError::RateLimited { retry_after_secs },
            GitHubError::Timeout => ApiError::GatewayTimeout,
            GitHubError::Upstream { status, message } => {
                ApiError::BadGateway(format!("GitHub returned an error (HTTP {status}): {message}"))
            }
            GitHubError::Network(msg) => ApiError::BadGateway(format!("Could not reach GitHub: {msg}")),
            GitHubError::Decode(msg) => ApiError::BadGateway(format!("Unexpected response from GitHub: {msg}")),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed | sqlx::Error::Io(_) => {
                tracing::error!(error = %e, "database unavailable");
                ApiError::Unavailable("The database is temporarily unavailable.".into())
            }
            other => ApiError::Internal(format!("database error: {other}")),
        }
    }
}

impl From<middleware::error::Error> for ApiError {
    fn from(e: middleware::error::Error) -> Self {
        use middleware::error::Error as E;
        match e {
            E::Reqwest(err) if err.is_timeout() => ApiError::GatewayTimeout,
            E::Reqwest(err) => ApiError::BadGateway(format!("GitHub GraphQL request failed: {}", err.without_url())),
            E::GraphQL(msg) => ApiError::BadGateway(format!("GitHub GraphQL API returned errors: {msg}")),
            E::Sqlx(err) => err.into(),
            other => ApiError::Internal(other.to_string()),
        }
    }
}

/// Converts actix extractor failures (malformed JSON, bad query strings or path params)
/// into the standard error format.
pub fn json_error_handler(err: JsonPayloadError, _req: &HttpRequest) -> actix_web::Error {
    let message = match &err {
        JsonPayloadError::ContentType => "Content-Type must be application/json".to_owned(),
        JsonPayloadError::OverflowKnownLength { .. } | JsonPayloadError::Overflow { .. } => {
            "Request body is too large".to_owned()
        }
        JsonPayloadError::Deserialize(e) => format!("Invalid JSON body: {e}"),
        other => format!("Invalid request body: {other}"),
    };
    ApiError::BadRequest(message).into()
}

pub fn query_error_handler(err: QueryPayloadError, _req: &HttpRequest) -> actix_web::Error {
    ApiError::BadRequest(format!("Invalid query parameters: {err}")).into()
}

pub fn path_error_handler(err: PathError, _req: &HttpRequest) -> actix_web::Error {
    ApiError::BadRequest(format!("Invalid path parameter: {err}")).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::body::to_bytes;

    async fn body_json(err: ApiError) -> (StatusCode, serde_json::Value, Option<String>) {
        let resp = err.error_response();
        let status = resp.status();
        let retry = resp
            .headers()
            .get(header::RETRY_AFTER)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = to_bytes(resp.into_body()).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap(), retry)
    }

    #[actix_web::test]
    async fn rate_limit_sets_retry_after() {
        let (status, body, retry) = body_json(GitHubError::RateLimited { retry_after_secs: 42 }.into()).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(retry.as_deref(), Some("42"));
        assert_eq!(body["error"]["code"], "rate_limited");
    }

    #[actix_web::test]
    async fn internal_errors_are_not_leaked() {
        let (status, body, _) = body_json(ApiError::Internal("password=hunter2 in DSN".into())).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"]["message"], "An unexpected error occurred.");
    }

    #[actix_web::test]
    async fn validation_includes_details() {
        let (status, body, _) = body_json(ApiError::validation("title", "is required")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["error"]["details"][0]["field"], "title");
    }

    #[test]
    fn github_errors_map_to_statuses() {
        let cases = [
            (GitHubError::Unauthorized, StatusCode::UNAUTHORIZED),
            (GitHubError::Forbidden("x".into()), StatusCode::FORBIDDEN),
            (GitHubError::NotFound("x".into()), StatusCode::NOT_FOUND),
            (GitHubError::Gone("x".into()), StatusCode::GONE),
            (GitHubError::Validation("x".into()), StatusCode::UNPROCESSABLE_ENTITY),
            (GitHubError::Timeout, StatusCode::GATEWAY_TIMEOUT),
            (
                GitHubError::Upstream {
                    status: 500,
                    message: "x".into(),
                },
                StatusCode::BAD_GATEWAY,
            ),
            (GitHubError::Network("x".into()), StatusCode::BAD_GATEWAY),
        ];
        for (gh, expected) in cases {
            assert_eq!(ApiError::from(gh).status_code(), expected);
        }
    }
}

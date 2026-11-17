//! Authentication.
//!
//! * [`AuthUser`]: a user request carrying `Authorization: Bearer <GitHub OAuth token>`.
//!   The token is validated against GitHub (`GET /user`) and the result cached briefly.
//!   Authorisation for repository data is enforced by GitHub itself, because every GitHub
//!   call is made with the user's own token; local data is always scoped to the user's id.
//! * [`CommandAuth`]: operator requests using `COMMANDER_ACCESS_TOKEN` / `ADMIN_ACCESS_TOKEN`.

use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::Mutex,
    time::{Duration, Instant},
};

use actix_web::{dev::Payload, http::header, web, FromRequest, HttpRequest};
use middleware::github::models::User;
use sha2::{Digest, Sha256};

use crate::{error::ApiError, state::AppState};

const MAX_CACHE_ENTRIES: usize = 10_000;
const MAX_TOKEN_LEN: usize = 1024;

type TokenKey = [u8; 32];

fn token_key(token: &str) -> TokenKey {
    Sha256::digest(token.as_bytes()).into()
}

/// In-memory cache of validated tokens, keyed by the token's SHA-256 digest.
pub struct AuthCache {
    ttl: Duration,
    entries: Mutex<HashMap<TokenKey, (User, Instant)>>,
}

impl AuthCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, token: &str) -> Option<User> {
        if self.ttl.is_zero() {
            return None;
        }
        let entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        entries
            .get(&token_key(token))
            .filter(|(_, at)| at.elapsed() < self.ttl)
            .map(|(user, _)| user.clone())
    }

    pub fn insert(&self, token: &str, user: User) {
        if self.ttl.is_zero() {
            return;
        }
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        if entries.len() >= MAX_CACHE_ENTRIES {
            let ttl = self.ttl;
            entries.retain(|_, (_, at)| at.elapsed() < ttl);
            if entries.len() >= MAX_CACHE_ENTRIES {
                entries.clear();
            }
        }
        entries.insert(token_key(token), (user, Instant::now()));
    }

    pub fn invalidate(&self, token: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        entries.remove(&token_key(token));
    }
}

/// Extracts the bearer token from the `Authorization` header.
fn bearer_token(req: &HttpRequest) -> Result<String, ApiError> {
    let value = req
        .headers()
        .get(header::AUTHORIZATION)
        .ok_or_else(|| ApiError::Unauthorized("Missing Authorization header. Sign in with GitHub.".into()))?
        .to_str()
        .map_err(|_| ApiError::Unauthorized("Malformed Authorization header.".into()))?;
    let (scheme, token) = value
        .split_once(' ')
        .ok_or_else(|| ApiError::Unauthorized("Authorization header must use the Bearer scheme.".into()))?;
    let token = token.trim();
    if !scheme.eq_ignore_ascii_case("bearer")
        || token.is_empty()
        || token.len() > MAX_TOKEN_LEN
        || !token.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(ApiError::Unauthorized(
            "Authorization header must be 'Bearer <token>'.".into(),
        ));
    }
    Ok(token.to_owned())
}

fn app_state(req: &HttpRequest) -> Result<web::Data<AppState>, ApiError> {
    req.app_data::<web::Data<AppState>>()
        .cloned()
        .ok_or_else(|| ApiError::Internal("AppState not registered".into()))
}

/// An authenticated GitHub user.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user: User,
    token: String,
}

impl AuthUser {
    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn id(&self) -> i64 {
        self.user.id
    }

    pub fn login(&self) -> &str {
        &self.user.login
    }
}

impl FromRequest for AuthUser {
    type Error = ApiError;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let token = bearer_token(req);
        let state = app_state(req);
        Box::pin(async move {
            let token = token?;
            let state = state?;
            if let Some(user) = state.auth_cache.get(&token) {
                return Ok(AuthUser { user, token });
            }
            let user = state.github.authenticated_user(&token).await?;
            state.auth_cache.insert(&token, user.clone());
            Ok(AuthUser { user, token })
        })
    }
}

/// Constant-time comparison (both inputs are hashed first so lengths match).
fn tokens_match(provided: &str, expected: &str) -> bool {
    let a = token_key(provided);
    let b = token_key(expected);
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Operator access for maintenance commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAuth {
    Commander,
    Admin,
}

impl FromRequest for CommandAuth {
    type Error = ApiError;
    type Future = std::future::Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let result = (|| {
            let state = app_state(req)?;
            if state.commander_token.is_none() && state.admin_token.is_none() {
                return Err(ApiError::Unavailable(
                    "Commands are disabled: set COMMANDER_ACCESS_TOKEN or ADMIN_ACCESS_TOKEN.".into(),
                ));
            }
            let token = bearer_token(req)?;
            if state.admin_token.as_deref().is_some_and(|t| tokens_match(&token, t)) {
                return Ok(CommandAuth::Admin);
            }
            if state
                .commander_token
                .as_deref()
                .is_some_and(|t| tokens_match(&token, t))
            {
                return Ok(CommandAuth::Commander);
            }
            Err(ApiError::Unauthorized("Invalid command token.".into()))
        })();
        std::future::ready(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    fn user(id: i64) -> User {
        User {
            id,
            login: format!("u{id}"),
            name: None,
            avatar_url: String::new(),
            html_url: String::new(),
        }
    }

    #[test]
    fn cache_respects_ttl_and_invalidation() {
        let cache = AuthCache::new(Duration::from_secs(60));
        cache.insert("tok", user(1));
        assert_eq!(cache.get("tok").unwrap().id, 1);
        assert!(cache.get("other").is_none());
        cache.invalidate("tok");
        assert!(cache.get("tok").is_none());

        let disabled = AuthCache::new(Duration::ZERO);
        disabled.insert("tok", user(1));
        assert!(disabled.get("tok").is_none());
    }

    #[test]
    fn bearer_parsing() {
        let ok = TestRequest::default()
            .insert_header(("Authorization", "Bearer abc123"))
            .to_http_request();
        assert_eq!(bearer_token(&ok).unwrap(), "abc123");
        let lower = TestRequest::default()
            .insert_header(("Authorization", "bearer abc"))
            .to_http_request();
        assert_eq!(bearer_token(&lower).unwrap(), "abc");
        for bad in ["Basic abc", "Bearer", "Bearer ", "Token abc"] {
            let req = TestRequest::default()
                .insert_header(("Authorization", bad))
                .to_http_request();
            assert!(bearer_token(&req).is_err(), "{bad} should be rejected");
        }
        assert!(bearer_token(&TestRequest::default().to_http_request()).is_err());
    }

    #[test]
    fn token_comparison() {
        assert!(tokens_match("same-secret-value", "same-secret-value"));
        assert!(!tokens_match("same-secret-value", "same-secret-valuf"));
        assert!(!tokens_match("short", "same-secret-value"));
    }
}

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
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use actix_web::{dev::Payload, http::header, web, FromRequest, HttpRequest};
use middleware::github::{models::User, GitHubError};
use sha2::{Digest, Sha256};
use tokio::sync::OnceCell;

use crate::{error::ApiError, state::AppState};

const MAX_CACHE_ENTRIES: usize = 10_000;
const MAX_TOKEN_LEN: usize = 1024;

type TokenKey = [u8; 32];

fn token_key(token: &str) -> TokenKey {
    Sha256::digest(token.as_bytes()).into()
}

type Validation = Arc<OnceCell<Result<User, GitHubError>>>;

/// In-memory cache of validated tokens, keyed by the token's SHA-256 digest.
///
/// Concurrent validations of the same token are coalesced into one GitHub request
/// (a page load fires several API calls at once; without this each would hit `GET /user`).
pub struct AuthCache {
    ttl: Duration,
    entries: Mutex<HashMap<TokenKey, (User, Instant)>>,
    in_flight: Mutex<HashMap<TokenKey, Validation>>,
}

impl AuthCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the cached user, or validates the token with `validate`, sharing one
    /// in-flight validation between concurrent callers. Only successes are cached.
    pub async fn get_or_validate<F, Fut>(&self, token: &str, validate: F) -> Result<User, GitHubError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<User, GitHubError>>,
    {
        if let Some(user) = self.get(token) {
            return Ok(user);
        }
        let key = token_key(token);
        let cell = {
            let mut in_flight = self.in_flight.lock().unwrap_or_else(|p| p.into_inner());
            in_flight.entry(key).or_default().clone()
        };
        let result = cell.get_or_init(validate).await.clone();
        if let Ok(user) = &result {
            // Cache before releasing the in-flight slot so late arrivals hit the cache.
            self.insert(token, user.clone());
        }
        {
            let mut in_flight = self.in_flight.lock().unwrap_or_else(|p| p.into_inner());
            if in_flight.get(&key).is_some_and(|c| Arc::ptr_eq(c, &cell)) {
                in_flight.remove(&key);
            }
        }
        result
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
pub(crate) fn bearer_token(req: &HttpRequest) -> Result<String, ApiError> {
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
            let user = state
                .auth_cache
                .get_or_validate(&token, || state.github.authenticated_user(&token))
                .await?;
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

    #[tokio::test]
    async fn concurrent_validations_share_one_request_and_failures_are_not_cached() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let cache = AuthCache::new(Duration::from_secs(60));
        let calls = AtomicUsize::new(0);
        let calls = &calls;
        let ok = move || async move {
            calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(user(7))
        };
        let (a, b, c) = tokio::join!(
            cache.get_or_validate("tok", ok),
            cache.get_or_validate("tok", ok),
            cache.get_or_validate("tok", ok)
        );
        assert_eq!((a.unwrap().id, b.unwrap().id, c.unwrap().id), (7, 7, 7));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "one GitHub call for three concurrent requests"
        );
        assert_eq!(cache.get_or_validate("tok", ok).await.unwrap().id, 7);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "served from cache afterwards");

        let failing = move || async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(GitHubError::Unauthorized)
        };
        assert!(cache.get_or_validate("bad", failing).await.is_err());
        assert!(cache.get_or_validate("bad", failing).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3, "failures are retried, not cached");
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

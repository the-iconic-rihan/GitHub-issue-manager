use actix_web::{web, HttpResponse};
use middleware::issue::{DiscoverFilter, Issue};

use crate::{
    auth::AuthUser,
    error::ApiError,
    response::{self, PageMeta},
    state::AppState,
    validate::{DiscoverQuery, PageQuery},
};

fn clean(value: Option<String>, field: &str) -> Result<Option<String>, ApiError> {
    match value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) {
        Some(v) if v.chars().count() > 100 || v.chars().any(char::is_control) => {
            Err(ApiError::validation(field, "is invalid"))
        }
        other => Ok(other),
    }
}

/// Beginner-friendly issues collected from across GitHub by the Discover aggregator.
pub async fn issues(
    state: web::Data<AppState>,
    _user: AuthUser,
    query: web::Query<DiscoverQuery>,
) -> Result<HttpResponse, ApiError> {
    let query = query.into_inner();
    let (page, per_page) = PageQuery {
        page: query.page,
        per_page: query.per_page,
    }
    .resolve()?;
    let filter = DiscoverFilter {
        language: clean(query.language, "language")?,
        label: clean(query.label, "label")?,
        limit: i64::from(per_page),
        offset: i64::from(page - 1) * i64::from(per_page),
    };
    let (items, total) = Issue::search(&state.pool, &filter).await?;
    let total = u64::try_from(total).unwrap_or(0);
    let meta = PageMeta {
        page,
        per_page,
        has_next: u64::from(page) * u64::from(per_page) < total,
        total_count: Some(total),
    };
    Ok(response::paged(items, meta))
}

pub async fn languages(state: web::Data<AppState>, _user: AuthUser) -> Result<HttpResponse, ApiError> {
    Ok(response::ok(Issue::languages(&state.pool).await?))
}

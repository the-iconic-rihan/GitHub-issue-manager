mod account;
mod command;
mod dashboard;
mod discover;
mod health;
mod issues;
mod repos;

use actix_web::{web, HttpResponse};

use crate::error::{json_error_handler, path_error_handler, query_error_handler, ApiError};

/// Maximum accepted JSON body (issue bodies are capped at 65,536 characters, i.e. < 256 KiB).
const JSON_LIMIT: usize = 512 * 1024;

async fn not_found() -> Result<HttpResponse, ApiError> {
    Err(ApiError::NotFound("No such API endpoint.".into()))
}

/// Registers every route under `/api/v1`, plus extractor error handling.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.app_data(
        web::JsonConfig::default()
            .limit(JSON_LIMIT)
            .error_handler(json_error_handler),
    )
    .app_data(web::QueryConfig::default().error_handler(query_error_handler))
    .app_data(web::PathConfig::default().error_handler(path_error_handler))
    .service(
        web::scope("/api/v1")
            .route("/health/live", web::get().to(health::live))
            .route("/health/ready", web::get().to(health::ready))
            .route("/me", web::get().to(account::me))
            .route("/me", web::delete().to(account::delete_me))
            .route("/dashboard", web::get().to(dashboard::dashboard))
            .route("/repos", web::get().to(repos::list))
            .route("/repos/{owner}/{repo}", web::get().to(repos::get))
            .route("/repos/{owner}/{repo}/labels", web::get().to(repos::labels))
            .route("/repos/{owner}/{repo}/assignees", web::get().to(repos::assignees))
            .route("/repos/{owner}/{repo}/issues", web::get().to(issues::list))
            .route("/repos/{owner}/{repo}/issues", web::post().to(issues::create))
            .route("/repos/{owner}/{repo}/issues/{number}", web::get().to(issues::get))
            .route("/repos/{owner}/{repo}/issues/{number}", web::patch().to(issues::update))
            .route(
                "/repos/{owner}/{repo}/issues/{number}/comments",
                web::get().to(issues::comments),
            )
            .route(
                "/repos/{owner}/{repo}/issues/{number}/comments",
                web::post().to(issues::add_comment),
            )
            .route(
                "/repos/{owner}/{repo}/issues/{number}/labels",
                web::post().to(issues::add_labels),
            )
            .route(
                "/repos/{owner}/{repo}/issues/{number}/labels",
                web::delete().to(issues::remove_label),
            )
            .route("/tracked-repos", web::get().to(repos::tracked))
            .route("/tracked-repos/{owner}/{repo}", web::put().to(repos::track))
            .route("/tracked-repos/{owner}/{repo}", web::delete().to(repos::untrack))
            .route("/tracked-repos/{owner}/{repo}/sync", web::post().to(repos::sync))
            .route("/discover/issues", web::get().to(discover::issues))
            .route("/discover/languages", web::get().to(discover::languages))
            .route("/command", web::post().to(command::command))
            .default_service(web::to(not_found)),
    );
}

/// Fallback for paths outside `/api/v1`.
pub async fn fallback() -> Result<HttpResponse, ApiError> {
    not_found().await
}

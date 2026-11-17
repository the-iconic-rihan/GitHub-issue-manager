//! Operator commands for the Discover aggregator, compatible with `gi-cli control`:
//! `POST /api/v1/command` with `{"action": "insert"|"update", "target": "issues", "limit": N}`.

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use serde_json::json;

use crate::{auth::CommandAuth, error::ApiError, response, state::AppState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommandAction {
    Insert,
    Update,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandBody {
    pub action: CommandAction,
    pub target: String,
    pub limit: u16,
}

pub const MAX_COMMAND_LIMIT: u16 = 1000;

pub async fn command(
    state: web::Data<AppState>,
    auth: CommandAuth,
    body: web::Json<CommandBody>,
) -> Result<HttpResponse, ApiError> {
    let body = body.into_inner();
    if body.target != "issues" {
        return Err(ApiError::validation("target", "must be 'issues'"));
    }
    if body.limit == 0 || body.limit > MAX_COMMAND_LIMIT {
        return Err(ApiError::validation(
            "limit",
            &format!("must be between 1 and {MAX_COMMAND_LIMIT}"),
        ));
    }
    let discover = state
        .discover
        .as_ref()
        .ok_or_else(|| ApiError::Unavailable("Discover is disabled: set GITHUB_ACCESS_TOKEN on the server.".into()))?;
    let _guard = discover
        .lock
        .try_lock()
        .map_err(|_| ApiError::Conflict("A Discover job is already running. Try again later.".into()))?;

    tracing::info!(?auth, action = ?body.action, limit = body.limit, "discover command");
    let report = match body.action {
        CommandAction::Insert => {
            json!({ "action": "insert", "report": discover.routine.insert_issues(body.limit).await? })
        }
        CommandAction::Update => {
            json!({ "action": "update", "report": discover.routine.update_issues(body.limit).await? })
        }
    };
    Ok(response::ok(report))
}

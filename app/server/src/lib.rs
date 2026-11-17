//! HTTP API for GitHub Issue Manager (actix-web).
//!
//! Layout: `routes` (thin HTTP handlers) → `services` (business logic) →
//! `middleware` crate (GitHub clients and persistence).

pub mod auth;
pub mod config;
pub mod error;
pub mod response;
pub mod routes;
pub mod services;
pub mod state;
pub mod validate;

use actix_web::{
    body::MessageBody,
    dev::{ServiceFactory, ServiceRequest, ServiceResponse},
    http::header,
    middleware::{DefaultHeaders, Logger},
    web, App, Error,
};

pub use state::AppState;

/// Builds the application. Shared by `main` and the integration tests.
pub fn build_app(
    state: web::Data<AppState>,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<impl MessageBody>,
        Error = Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(state)
        .wrap(
            DefaultHeaders::new()
                .add((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
                .add((header::CACHE_CONTROL, "no-store")),
        )
        // Never log the Authorization header or bodies; the request line contains no secrets.
        .wrap(
            Logger::new(r#"%{r}a "%r" %s %b %Dms"#)
                .exclude("/api/v1/health/live")
                .exclude("/api/v1/health/ready"),
        )
        .configure(routes::configure)
        .default_service(web::to(routes::fallback))
}

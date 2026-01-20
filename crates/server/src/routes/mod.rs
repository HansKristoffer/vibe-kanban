use axum::{
    Router,
    middleware::from_fn_with_state,
    routing::{IntoMakeService, get},
};

use crate::{DeploymentImpl, middleware::require_authenticated_user};

pub mod approvals;
pub mod auth;
pub mod config;
pub mod containers;
pub mod filesystem;
// pub mod github;
pub mod events;
pub mod execution_processes;
pub mod frontend;
pub mod health;
pub mod images;
pub mod oauth;
pub mod organizations;
pub mod project_env_vars;
pub mod project_integrations;
pub mod projects;
pub mod inbox;
pub mod repo;
pub mod scratch;
pub mod sessions;
pub mod shared_tasks;
pub mod tags;
pub mod task_attempts;
pub mod tasks;
pub mod terminal;
pub mod public;
pub mod webhooks;

pub fn router(deployment: DeploymentImpl) -> IntoMakeService<Router> {
    let public_routes = Router::new()
        .route("/health", get(health::health_check))
        .merge(config::public_router())
        .merge(auth::router())
        .merge(oauth::router())
        .nest("/public", public::router())
        .nest("/webhooks", webhooks::router(&deployment));

    let protected_routes = Router::new()
        .merge(containers::router(&deployment))
        .merge(config::router())
        .merge(projects::router(&deployment))
        .merge(tasks::router(&deployment))
        .merge(shared_tasks::router())
        .merge(task_attempts::router(&deployment))
        .merge(execution_processes::router(&deployment))
        .merge(tags::router(&deployment))
        .merge(organizations::router())
        .merge(filesystem::router())
        .merge(repo::router())
        .merge(events::router(&deployment))
        .nest("/inbox", inbox::router(&deployment))
        .merge(approvals::router())
        .merge(scratch::router(&deployment))
        .merge(sessions::router(&deployment))
        .merge(terminal::router())
        .nest("/images", images::routes())
        .layer(from_fn_with_state(
            deployment.clone(),
            require_authenticated_user,
        ));

    let base_routes = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .with_state(deployment);

    Router::new()
        .route("/", get(frontend::serve_frontend_root))
        .route("/{*path}", get(frontend::serve_frontend))
        .nest("/api", base_routes)
        .into_make_service()
}

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use db::models::{
    auth_session::AuthSession,
    auth_user::AuthUser,
};
use deployment::Deployment;
use uuid::Uuid;

use crate::DeploymentImpl;

pub const SESSION_COOKIE_NAME: &str = "vk_session";

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
}

pub async fn require_authenticated_user(
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(session_id) = extract_session_cookie(&headers) else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let pool = &deployment.db().pool;
    let session = match AuthSession::find_by_id(pool, &session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if session.expires_at <= Utc::now() {
        let _ = AuthSession::delete(pool, &session_id).await;
        return Err(StatusCode::UNAUTHORIZED);
    }

    let user = match AuthUser::find_by_id(pool, session.user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let mut request = request;
    request.extensions_mut().insert(AuthenticatedUser {
        id: user.id,
        email: user.email,
        name: user.name,
        picture_url: user.picture_url,
    });

    Ok(next.run(request).await)
}

pub fn extract_session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            if cookie.starts_with(SESSION_COOKIE_NAME) {
                cookie
                    .strip_prefix(SESSION_COOKIE_NAME)
                    .and_then(|s| s.strip_prefix('='))
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
}

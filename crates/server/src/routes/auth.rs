use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, HeaderName, StatusCode, header::SET_COOKIE},
    response::{Json as ResponseJson, Response},
    routing::get,
};
use chrono::{Duration, Utc};
use db::models::{
    auth_oauth_state::AuthOAuthState,
    auth_session::AuthSession,
    auth_user::{AuthUser, UpsertAuthUser},
    project_member::ProjectMember,
};
use deployment::Deployment;
use rand::{Rng, distributions::Alphanumeric};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError, middleware::SESSION_COOKIE_NAME};

const SESSION_DURATION_HOURS: i64 = 24;
const STATE_DURATION_MINUTES: i64 = 10;

#[derive(Debug, Deserialize)]
pub struct GoogleStartQuery {
    pub return_to: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct GoogleStartResponse {
    pub authorize_url: String,
}

#[derive(Debug, Deserialize)]
pub struct GoogleCallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AuthUserDto {
    pub email: String,
    pub name: Option<String>,
    pub picture_url: Option<String>,
}

#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct AuthSessionResponse {
    pub authenticated: bool,
    pub user: Option<AuthUserDto>,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    id_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenInfo {
    email: String,
    email_verified: String,
    aud: String,
    name: Option<String>,
    picture: Option<String>,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/auth/google/start", get(google_start))
        .route("/auth/google/callback", get(google_callback))
        .route("/auth/session", get(get_session))
        .route("/auth/logout", get(logout))
}

async fn google_start(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<GoogleStartQuery>,
) -> Result<ResponseJson<ApiResponse<GoogleStartResponse>>, ApiError> {
    let config = google_oauth_config()?;
    let return_to = normalize_return_to(query.return_to);
    let state = generate_state();

    AuthOAuthState::create(
        &deployment.db().pool,
        &state,
        return_to.as_deref(),
        Utc::now() + Duration::minutes(STATE_DURATION_MINUTES),
    )
    .await
    .map_err(ApiError::Database)?;

    let authorize_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope=openid%20email%20profile&state={}&prompt=select_account",
        urlencoding::encode(&config.client_id),
        urlencoding::encode(&config.redirect_uri),
        urlencoding::encode(&state)
    );

    Ok(ResponseJson(ApiResponse::success(GoogleStartResponse {
        authorize_url,
    })))
}

async fn google_callback(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<GoogleCallbackQuery>,
) -> Result<Response<String>, ApiError> {
    let config = google_oauth_config()?;
    let pool = &deployment.db().pool;

    let state = AuthOAuthState::find_by_id(pool, &query.state)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::BadRequest("Invalid OAuth state".to_string()))?;

    if state.expires_at <= Utc::now() {
        let _ = AuthOAuthState::delete(pool, &query.state).await;
        return Err(ApiError::BadRequest("OAuth state expired".to_string()));
    }

    let _ = AuthOAuthState::delete(pool, &query.state).await;

    let token = exchange_code_for_token(&config, &query.code).await?;
    let token_info = validate_id_token(&config.client_id, &token.id_token).await?;

    if token_info.email_verified != "true" {
        return Err(ApiError::Unauthorized);
    }

    let user = AuthUser::upsert(
        pool,
        &UpsertAuthUser {
            email: token_info.email.clone(),
            name: token_info.name.clone(),
            picture_url: token_info.picture.clone(),
        },
    )
    .await
    .map_err(ApiError::Database)?;

    let session_id = generate_session_id();
    let expires_at = Utc::now() + Duration::hours(SESSION_DURATION_HOURS);
    AuthSession::create(pool, &session_id, user.id, expires_at)
        .await
        .map_err(ApiError::Database)?;

    // Bootstrap: assign any orphan projects (projects with no members) to this user
    match ProjectMember::assign_orphan_projects_to_user(pool, &token_info.email).await {
        Ok(count) if count > 0 => {
            tracing::info!(
                "Assigned {} orphan project(s) to user {}",
                count,
                token_info.email
            );
        }
        Ok(_) => {}
        Err(err) => {
            tracing::warn!("Failed to assign orphan projects: {}", err);
        }
    }

    let cookie = build_session_cookie(&session_id);
    let return_to = state
        .return_to
        .and_then(|value| normalize_return_to(Some(value)))
        .unwrap_or_else(|| "/projects".to_string());

    Ok(Response::builder()
        .status(StatusCode::FOUND)
        .header(SET_COOKIE, cookie)
        .header(HeaderName::from_static("location"), return_to)
        .body("Redirecting...".to_string())
        .unwrap())
}

async fn get_session(
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
) -> Result<ResponseJson<ApiResponse<AuthSessionResponse>>, ApiError> {
    // When auth is disabled, always return authenticated
    if std::env::var("AUTH_DISABLED")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        return Ok(ResponseJson(ApiResponse::success(AuthSessionResponse {
            authenticated: true,
            user: None,
        })));
    }

    let user = load_user_from_headers(&deployment, &headers).await?;
    Ok(ResponseJson(ApiResponse::success(AuthSessionResponse {
        authenticated: user.is_some(),
        user: user.map(|user| AuthUserDto {
            email: user.email,
            name: user.name,
            picture_url: user.picture_url,
        }),
    })))
}

async fn logout(
    State(deployment): State<DeploymentImpl>,
    headers: HeaderMap,
) -> Result<(StatusCode, [(HeaderName, String); 1]), ApiError> {
    if let Some(session_id) = crate::middleware::extract_session_cookie(&headers) {
        let _ = AuthSession::delete(&deployment.db().pool, &session_id).await;
    }
    Ok((
        StatusCode::NO_CONTENT,
        [(SET_COOKIE, clear_session_cookie())],
    ))
}

async fn exchange_code_for_token(
    config: &GoogleOAuthConfig,
    code: &str,
) -> Result<GoogleTokenResponse, ApiError> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", config.redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|err| ApiError::BadRequest(format!("OAuth token exchange failed: {}", err)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::BadRequest(format!(
            "OAuth token exchange failed: {} {}",
            status, body
        )));
    }

    response
        .json::<GoogleTokenResponse>()
        .await
        .map_err(|err| ApiError::BadRequest(format!("Invalid token response: {}", err)))
}

async fn validate_id_token(
    client_id: &str,
    id_token: &str,
) -> Result<GoogleTokenInfo, ApiError> {
    let url = format!(
        "https://oauth2.googleapis.com/tokeninfo?id_token={}",
        id_token
    );
    let response = reqwest::get(&url)
        .await
        .map_err(|err| ApiError::BadRequest(format!("Token validation failed: {}", err)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::BadRequest(format!(
            "Token validation failed: {} {}",
            status, body
        )));
    }

    let info = response
        .json::<GoogleTokenInfo>()
        .await
        .map_err(|err| ApiError::BadRequest(format!("Invalid token info: {}", err)))?;

    if info.aud != client_id {
        return Err(ApiError::BadRequest(
            "Token audience does not match client ID".to_string(),
        ));
    }

    Ok(info)
}

async fn load_user_from_headers(
    deployment: &DeploymentImpl,
    headers: &HeaderMap,
) -> Result<Option<AuthUser>, ApiError> {
    let Some(session_id) = crate::middleware::extract_session_cookie(headers) else {
        return Ok(None);
    };

    let pool = &deployment.db().pool;
    let session = match AuthSession::find_by_id(pool, &session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => return Ok(None),
        Err(err) => return Err(ApiError::Database(err)),
    };

    if session.expires_at <= Utc::now() {
        let _ = AuthSession::delete(pool, &session_id).await;
        return Ok(None);
    }

    let user = AuthUser::find_by_id(pool, session.user_id)
        .await
        .map_err(ApiError::Database)?;
    Ok(user)
}

fn normalize_return_to(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    if value.starts_with('/') && !value.starts_with("//") {
        Some(value)
    } else {
        None
    }
}

fn generate_state() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

fn generate_session_id() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

fn build_session_cookie(session_id: &str) -> String {
    let mut cookie = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        SESSION_COOKIE_NAME,
        session_id,
        SESSION_DURATION_HOURS * 60 * 60
    );
    if should_use_secure_cookie() {
        cookie.push_str("; Secure");
    }
    cookie
}

fn clear_session_cookie() -> String {
    let mut cookie = format!(
        "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
        SESSION_COOKIE_NAME
    );
    if should_use_secure_cookie() {
        cookie.push_str("; Secure");
    }
    cookie
}

fn should_use_secure_cookie() -> bool {
    std::env::var("VK_PUBLIC_BASE_URL")
        .ok()
        .map(|value| value.starts_with("https://"))
        .unwrap_or(false)
}

struct GoogleOAuthConfig {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

fn google_oauth_config() -> Result<GoogleOAuthConfig, ApiError> {
    let client_id =
        std::env::var("GOOGLE_CLIENT_ID").map_err(|_| ApiError::BadRequest(
            "GOOGLE_CLIENT_ID is not configured".to_string(),
        ))?;
    let client_secret =
        std::env::var("GOOGLE_CLIENT_SECRET").map_err(|_| ApiError::BadRequest(
            "GOOGLE_CLIENT_SECRET is not configured".to_string(),
        ))?;
    let base_url =
        std::env::var("VK_PUBLIC_BASE_URL").map_err(|_| ApiError::BadRequest(
            "VK_PUBLIC_BASE_URL is not configured".to_string(),
        ))?;
    let redirect_uri = format!("{}/api/auth/google/callback", base_url.trim_end_matches('/'));

    Ok(GoogleOAuthConfig {
        client_id,
        client_secret,
        redirect_uri,
    })
}

/// Validates that all required Google OAuth environment variables are set.
/// Call this at server startup to fail fast if misconfigured.
/// Skips validation when AUTH_DISABLED is set.
pub fn validate_oauth_config() -> Result<(), String> {
    // Skip OAuth validation when auth is disabled
    if std::env::var("AUTH_DISABLED")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false)
    {
        return Ok(());
    }

    let mut missing = Vec::new();
    if std::env::var("GOOGLE_CLIENT_ID").is_err() {
        missing.push("GOOGLE_CLIENT_ID");
    }
    if std::env::var("GOOGLE_CLIENT_SECRET").is_err() {
        missing.push("GOOGLE_CLIENT_SECRET");
    }
    if std::env::var("VK_PUBLIC_BASE_URL").is_err() {
        missing.push("VK_PUBLIC_BASE_URL");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Missing required OAuth environment variables: {}",
            missing.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_return_to;

    #[test]
    fn normalize_return_to_allows_relative_paths() {
        assert_eq!(
            normalize_return_to(Some("/projects?tab=all".to_string())),
            Some("/projects?tab=all".to_string())
        );
    }

    #[test]
    fn normalize_return_to_rejects_absolute_urls() {
        assert_eq!(
            normalize_return_to(Some("https://example.com".to_string())),
            None
        );
    }
}

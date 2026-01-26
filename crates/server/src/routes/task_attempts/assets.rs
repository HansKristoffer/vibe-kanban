//! API routes for workspace assets (screenshots and videos).
//!
//! These routes allow the frontend to list and serve assets from a workspace's
//! `.vibe-assets/` directory.

use axum::{
    Extension, Router,
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::from_fn_with_state,
    response::{Json as ResponseJson, Response},
    routing::get,
};
use db::models::workspace::Workspace;
use deployment::Deployment;
use serde::Serialize;
use services::services::{container::ContainerService, workspace_assets::WorkspaceAssetService};
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use utils::{
    response::ApiResponse,
    workspace_assets::{AssetEntry, AssetType},
};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::load_workspace_middleware};

/// Response type for a single asset
#[derive(Debug, Clone, Serialize)]
pub struct AssetResponse {
    pub id: String,
    pub asset_type: String,
    pub filename: String,
    pub description: Option<String>,
    pub related_files: Vec<String>,
    pub captured_at: String,
    pub duration_ms: Option<u64>,
    pub size_bytes: Option<u64>,
    pub url: String,
}

impl AssetResponse {
    fn from_entry(entry: &AssetEntry, workspace_id: Uuid) -> Self {
        let asset_type = match entry.asset_type {
            AssetType::Screenshot => "screenshot",
            AssetType::Video => "video",
        };
        Self {
            id: entry.id.clone(),
            asset_type: asset_type.to_string(),
            filename: entry.filename.clone(),
            description: entry.description.clone(),
            related_files: entry.related_files.clone(),
            captured_at: entry.captured_at.to_rfc3339(),
            duration_ms: entry.duration_ms,
            size_bytes: entry.size_bytes,
            url: format!("/api/task-attempts/{}/assets/{}", workspace_id, entry.id),
        }
    }
}

/// Response for listing assets
#[derive(Debug, Clone, Serialize)]
pub struct ListAssetsResponse {
    pub assets: Vec<AssetResponse>,
    pub total: usize,
}

/// List all assets in a workspace
pub async fn list_assets(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ListAssetsResponse>>, ApiError> {
    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = std::path::PathBuf::from(container_ref);
    let base_path = match workspace.agent_working_dir.as_deref() {
        Some(dir) if !dir.is_empty() => workspace_path.join(dir),
        _ => workspace_path,
    };

    let service = WorkspaceAssetService::new();
    let assets = service.get_assets(&base_path).unwrap_or_default();

    let response = ListAssetsResponse {
        total: assets.len(),
        assets: assets
            .iter()
            .map(|a| AssetResponse::from_entry(a, workspace.id))
            .collect(),
    };

    Ok(ResponseJson(ApiResponse::success(response)))
}

/// Get a single asset metadata
pub async fn get_asset(
    axum::extract::Path((_id, asset_id)): axum::extract::Path<(Uuid, String)>,
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<AssetResponse>>, ApiError> {
    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = std::path::PathBuf::from(container_ref);
    let base_path = match workspace.agent_working_dir.as_deref() {
        Some(dir) if !dir.is_empty() => workspace_path.join(dir),
        _ => workspace_path,
    };

    let service = WorkspaceAssetService::new();
    let asset = service
        .find_asset(&base_path, &asset_id)
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(ResponseJson(ApiResponse::success(AssetResponse::from_entry(
        &asset,
        workspace.id,
    ))))
}

/// Serve an asset file
pub async fn serve_asset(
    axum::extract::Path((_id, asset_id)): axum::extract::Path<(Uuid, String)>,
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
) -> Result<Response, ApiError> {
    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_path = std::path::PathBuf::from(container_ref);
    let base_path = match workspace.agent_working_dir.as_deref() {
        Some(dir) if !dir.is_empty() => workspace_path.join(dir),
        _ => workspace_path,
    };

    let service = WorkspaceAssetService::new();

    // Get asset metadata
    let asset = service
        .find_asset(&base_path, &asset_id)
        .map_err(|e| ApiError::BadRequest(format!("Asset not found: {}", e)))?;

    // Get asset file path (with security checks)
    let asset_path = service
        .get_asset_path(&base_path, &asset_id)
        .map_err(|e| ApiError::BadRequest(format!("Asset file not found: {}", e)))?;

    // Open and stream the file
    let file = File::open(&asset_path)
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to open asset: {}", e)))?;

    let metadata = file
        .metadata()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Failed to read asset metadata: {}", e)))?;

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    // Determine content type
    let content_type = asset.asset_type.mime_type();

    // For videos, support range requests (partial content)
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, metadata.len())
        .header(header::CACHE_CONTROL, "public, max-age=31536000")
        .header(header::ACCEPT_RANGES, "bytes")
        .body(body)
        .map_err(|e| ApiError::BadRequest(format!("Failed to build response: {}", e)))?;

    Ok(response)
}

/// Middleware to load workspace for routes with path params
async fn load_workspace_with_asset_id(
    State(deployment): State<DeploymentImpl>,
    axum::extract::Path((id, _asset_id)): axum::extract::Path<(Uuid, String)>,
    mut request: Request,
    next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
    let workspace = match Workspace::find_by_id(&deployment.db().pool, id).await {
        Ok(Some(w)) => w,
        Ok(None) => return Err(StatusCode::NOT_FOUND),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    request.extensions_mut().insert(workspace);
    Ok(next.run(request).await)
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    // List assets route (needs workspace loaded via standard middleware)
    let list_router = Router::new()
        .route("/", get(list_assets))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_workspace_middleware,
        ));

    // Individual asset routes (need custom middleware for path params)
    let asset_router = Router::new()
        .route("/{asset_id}", get(get_asset))
        .route("/{asset_id}/file", get(serve_asset))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_workspace_with_asset_id,
        ));

    list_router.merge(asset_router)
}

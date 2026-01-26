//! Workspace asset service for reading and serving task documentation assets.
//!
//! This service reads assets from the `.vibe-assets/` directory within a workspace worktree.
//! Assets are managed by the vibe-recorder MCP server; this service only reads and serves them.

use std::{
    fs,
    path::{Path, PathBuf},
};

use utils::{
    path::{VIBE_ASSETS_DIR, VIBE_ASSETS_MANIFEST},
    workspace_assets::{AssetEntry, AssetManifest, AssetType},
};

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceAssetError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Asset not found: {0}")]
    NotFound(String),

    #[error("Workspace not found: {0}")]
    WorkspaceNotFound(String),

    #[error("Invalid asset path")]
    InvalidPath,
}

/// Service for reading workspace assets from the worktree
#[derive(Clone)]
pub struct WorkspaceAssetService;

impl WorkspaceAssetService {
    pub fn new() -> Self {
        Self
    }

    /// Get the assets directory path for a worktree
    pub fn get_assets_dir(worktree_path: &Path) -> PathBuf {
        worktree_path.join(VIBE_ASSETS_DIR)
    }

    /// Get the manifest file path for a worktree
    pub fn get_manifest_path(worktree_path: &Path) -> PathBuf {
        Self::get_assets_dir(worktree_path).join(VIBE_ASSETS_MANIFEST)
    }

    /// Read the asset manifest from a worktree
    pub fn read_manifest(&self, worktree_path: &Path) -> Result<AssetManifest, WorkspaceAssetError> {
        let manifest_path = Self::get_manifest_path(worktree_path);

        if !manifest_path.exists() {
            // No manifest = no assets, return empty manifest
            return Ok(AssetManifest::new());
        }

        let content = fs::read_to_string(&manifest_path)?;
        let manifest: AssetManifest = serde_json::from_str(&content)?;
        Ok(manifest)
    }

    /// Get all assets for a worktree
    pub fn get_assets(&self, worktree_path: &Path) -> Result<Vec<AssetEntry>, WorkspaceAssetError> {
        let manifest = self.read_manifest(worktree_path)?;
        Ok(manifest.assets)
    }

    /// Find a specific asset by ID
    pub fn find_asset(
        &self,
        worktree_path: &Path,
        asset_id: &str,
    ) -> Result<AssetEntry, WorkspaceAssetError> {
        let manifest = self.read_manifest(worktree_path)?;
        manifest
            .find_asset(asset_id)
            .cloned()
            .ok_or_else(|| WorkspaceAssetError::NotFound(asset_id.to_string()))
    }

    /// Get the file path for an asset
    pub fn get_asset_path(
        &self,
        worktree_path: &Path,
        asset_id: &str,
    ) -> Result<PathBuf, WorkspaceAssetError> {
        let asset = self.find_asset(worktree_path, asset_id)?;
        let asset_path = Self::get_assets_dir(worktree_path).join(&asset.filename);

        if !asset_path.exists() {
            return Err(WorkspaceAssetError::NotFound(asset_id.to_string()));
        }

        // Security check: ensure the path is within the assets directory
        let canonical_assets_dir = Self::get_assets_dir(worktree_path)
            .canonicalize()
            .map_err(|_| WorkspaceAssetError::InvalidPath)?;
        let canonical_asset_path = asset_path
            .canonicalize()
            .map_err(|_| WorkspaceAssetError::NotFound(asset_id.to_string()))?;

        if !canonical_asset_path.starts_with(&canonical_assets_dir) {
            return Err(WorkspaceAssetError::InvalidPath);
        }

        Ok(canonical_asset_path)
    }

    /// Get the MIME type for an asset
    pub fn get_asset_mime_type(&self, asset: &AssetEntry) -> &'static str {
        asset.asset_type.mime_type()
    }

    /// Read asset file contents
    pub fn read_asset_bytes(
        &self,
        worktree_path: &Path,
        asset_id: &str,
    ) -> Result<Vec<u8>, WorkspaceAssetError> {
        let asset_path = self.get_asset_path(worktree_path, asset_id)?;
        let bytes = fs::read(&asset_path)?;
        Ok(bytes)
    }

    /// Check if assets directory exists for a worktree
    pub fn has_assets(&self, worktree_path: &Path) -> bool {
        Self::get_assets_dir(worktree_path).exists()
    }

    /// Get summary stats for assets in a worktree
    pub fn get_stats(
        &self,
        worktree_path: &Path,
    ) -> Result<WorkspaceAssetStats, WorkspaceAssetError> {
        let assets = self.get_assets(worktree_path)?;

        let screenshot_count = assets
            .iter()
            .filter(|a| a.asset_type == AssetType::Screenshot)
            .count();
        let video_count = assets
            .iter()
            .filter(|a| a.asset_type == AssetType::Video)
            .count();
        let total_size: u64 = assets.iter().filter_map(|a| a.size_bytes).sum();

        Ok(WorkspaceAssetStats {
            total_assets: assets.len(),
            screenshot_count,
            video_count,
            total_size_bytes: total_size,
        })
    }
}

impl Default for WorkspaceAssetService {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for workspace assets
#[derive(Debug, Clone, serde::Serialize)]
pub struct WorkspaceAssetStats {
    pub total_assets: usize,
    pub screenshot_count: usize,
    pub video_count: usize,
    pub total_size_bytes: u64,
}

/// Format assets as markdown for inclusion in PR descriptions
///
/// Uses vibe-kanban API URLs for serving assets. The `base_url` should be
/// the externally accessible URL of the vibe-kanban instance.
///
/// # Arguments
/// * `assets` - List of assets to format
/// * `workspace_id` - The workspace ID for building URLs
/// * `base_url` - Optional base URL for the vibe-kanban instance (e.g., "http://localhost:3000")
///
/// # Returns
/// Markdown formatted string with asset previews
pub fn format_assets_as_markdown(
    assets: &[AssetEntry],
    workspace_id: uuid::Uuid,
    base_url: Option<&str>,
) -> String {
    if assets.is_empty() {
        return String::new();
    }

    let base = base_url.unwrap_or("");
    let mut md = String::from("\n\n## UI Documentation\n\n");

    for asset in assets {
        let url = format!(
            "{}/api/task-attempts/{}/assets/{}/file",
            base, workspace_id, asset.id
        );

        match asset.asset_type {
            AssetType::Screenshot => {
                if let Some(desc) = &asset.description {
                    md.push_str(&format!("### {}\n\n", desc));
                }
                md.push_str(&format!("![Screenshot]({})\n\n", url));
            }
            AssetType::Video => {
                let desc = asset
                    .description
                    .as_deref()
                    .unwrap_or("Video Recording");
                // Videos can't be embedded in GitHub markdown, so provide a link
                md.push_str(&format!(
                    "### {}\n\n[View Video Recording]({})\n\n",
                    desc, url
                ));
            }
        }
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_read_empty_manifest() {
        let temp = tempdir().unwrap();
        let service = WorkspaceAssetService::new();

        // No manifest file = empty manifest
        let manifest = service.read_manifest(temp.path()).unwrap();
        assert!(manifest.assets.is_empty());
    }

    #[test]
    fn test_read_manifest_with_assets() {
        let temp = tempdir().unwrap();
        let assets_dir = temp.path().join(VIBE_ASSETS_DIR);
        fs::create_dir_all(&assets_dir).unwrap();

        let manifest = AssetManifest {
            version: 1,
            assets: vec![AssetEntry::new_screenshot(
                "test-id".to_string(),
                "test.png".to_string(),
                Some("Test screenshot".to_string()),
            )],
        };

        let manifest_path = assets_dir.join(VIBE_ASSETS_MANIFEST);
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        let service = WorkspaceAssetService::new();
        let read_manifest = service.read_manifest(temp.path()).unwrap();

        assert_eq!(read_manifest.assets.len(), 1);
        assert_eq!(read_manifest.assets[0].id, "test-id");
    }

    #[test]
    fn test_find_asset() {
        let temp = tempdir().unwrap();
        let assets_dir = temp.path().join(VIBE_ASSETS_DIR);
        fs::create_dir_all(&assets_dir).unwrap();

        let manifest = AssetManifest {
            version: 1,
            assets: vec![
                AssetEntry::new_screenshot(
                    "id1".to_string(),
                    "file1.png".to_string(),
                    Some("First".to_string()),
                ),
                AssetEntry::new_video(
                    "id2".to_string(),
                    "file2.mp4".to_string(),
                    Some("Second".to_string()),
                    Some(5000),
                ),
            ],
        };

        let manifest_path = assets_dir.join(VIBE_ASSETS_MANIFEST);
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        let service = WorkspaceAssetService::new();

        let found = service.find_asset(temp.path(), "id1").unwrap();
        assert_eq!(found.filename, "file1.png");

        let found2 = service.find_asset(temp.path(), "id2").unwrap();
        assert_eq!(found2.asset_type, AssetType::Video);

        let not_found = service.find_asset(temp.path(), "nonexistent");
        assert!(not_found.is_err());
    }

    #[test]
    fn test_get_stats() {
        let temp = tempdir().unwrap();
        let assets_dir = temp.path().join(VIBE_ASSETS_DIR);
        fs::create_dir_all(&assets_dir).unwrap();

        let manifest = AssetManifest {
            version: 1,
            assets: vec![
                AssetEntry::new_screenshot("id1".to_string(), "f1.png".to_string(), None)
                    .with_size(1000),
                AssetEntry::new_screenshot("id2".to_string(), "f2.png".to_string(), None)
                    .with_size(2000),
                AssetEntry::new_video("id3".to_string(), "f3.mp4".to_string(), None, Some(5000))
                    .with_size(50000),
            ],
        };

        let manifest_path = assets_dir.join(VIBE_ASSETS_MANIFEST);
        fs::write(&manifest_path, serde_json::to_string(&manifest).unwrap()).unwrap();

        let service = WorkspaceAssetService::new();
        let stats = service.get_stats(temp.path()).unwrap();

        assert_eq!(stats.total_assets, 3);
        assert_eq!(stats.screenshot_count, 2);
        assert_eq!(stats.video_count, 1);
        assert_eq!(stats.total_size_bytes, 53000);
    }
}

//! Workspace asset types for task documentation (screenshots, videos).
//!
//! Assets are stored in `.vibe-assets/` directory within each workspace worktree.
//! The manifest.json file tracks all assets with their metadata.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Asset manifest stored in .vibe-assets/manifest.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetManifest {
    /// Manifest format version
    pub version: u32,
    /// List of all assets in this workspace
    pub assets: Vec<AssetEntry>,
}

impl AssetManifest {
    /// Current manifest version
    pub const CURRENT_VERSION: u32 = 1;

    /// Create a new empty manifest
    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            assets: Vec::new(),
        }
    }

    /// Add an asset entry to the manifest
    pub fn add_asset(&mut self, entry: AssetEntry) {
        self.assets.push(entry);
    }

    /// Remove an asset by ID
    pub fn remove_asset(&mut self, id: &str) -> Option<AssetEntry> {
        if let Some(pos) = self.assets.iter().position(|a| a.id == id) {
            Some(self.assets.remove(pos))
        } else {
            None
        }
    }

    /// Find an asset by ID
    pub fn find_asset(&self, id: &str) -> Option<&AssetEntry> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// Clear all assets
    pub fn clear(&mut self) {
        self.assets.clear();
    }
}

/// Type of asset
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    /// Screenshot image (PNG)
    Screenshot,
    /// Video recording (MP4)
    Video,
}

impl AssetType {
    /// Get the file extension for this asset type
    pub fn extension(&self) -> &'static str {
        match self {
            AssetType::Screenshot => "png",
            AssetType::Video => "mp4",
        }
    }

    /// Get the MIME type for this asset type
    pub fn mime_type(&self) -> &'static str {
        match self {
            AssetType::Screenshot => "image/png",
            AssetType::Video => "video/mp4",
        }
    }
}

/// A single asset entry in the manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetEntry {
    /// Unique identifier (UUID)
    pub id: String,
    /// Type of asset
    pub asset_type: AssetType,
    /// Filename relative to .vibe-assets/ (e.g., "abc123.png")
    pub filename: String,
    /// AI-provided description of what this asset shows
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Files that were changed when this asset was captured
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_files: Vec<String>,
    /// When the asset was captured
    pub captured_at: DateTime<Utc>,
    /// Duration in milliseconds (for videos only)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// File size in bytes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

impl AssetEntry {
    /// Create a new screenshot entry
    pub fn new_screenshot(id: String, filename: String, description: Option<String>) -> Self {
        Self {
            id,
            asset_type: AssetType::Screenshot,
            filename,
            description,
            related_files: Vec::new(),
            captured_at: Utc::now(),
            duration_ms: None,
            size_bytes: None,
        }
    }

    /// Create a new video entry
    pub fn new_video(
        id: String,
        filename: String,
        description: Option<String>,
        duration_ms: Option<u64>,
    ) -> Self {
        Self {
            id,
            asset_type: AssetType::Video,
            filename,
            description,
            related_files: Vec::new(),
            captured_at: Utc::now(),
            duration_ms,
            size_bytes: None,
        }
    }

    /// Set the file size
    pub fn with_size(mut self, size_bytes: u64) -> Self {
        self.size_bytes = Some(size_bytes);
        self
    }

    /// Set related files
    pub fn with_related_files(mut self, files: Vec<String>) -> Self {
        self.related_files = files;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_operations() {
        let mut manifest = AssetManifest::new();
        assert_eq!(manifest.version, AssetManifest::CURRENT_VERSION);
        assert!(manifest.assets.is_empty());

        // Add an asset
        let entry = AssetEntry::new_screenshot(
            "test-id".to_string(),
            "test.png".to_string(),
            Some("Test screenshot".to_string()),
        );
        manifest.add_asset(entry);
        assert_eq!(manifest.assets.len(), 1);

        // Find the asset
        let found = manifest.find_asset("test-id");
        assert!(found.is_some());
        assert_eq!(found.unwrap().filename, "test.png");

        // Remove the asset
        let removed = manifest.remove_asset("test-id");
        assert!(removed.is_some());
        assert!(manifest.assets.is_empty());
    }

    #[test]
    fn test_asset_type_extension() {
        assert_eq!(AssetType::Screenshot.extension(), "png");
        assert_eq!(AssetType::Video.extension(), "mp4");
    }

    #[test]
    fn test_asset_type_mime() {
        assert_eq!(AssetType::Screenshot.mime_type(), "image/png");
        assert_eq!(AssetType::Video.mime_type(), "video/mp4");
    }

    #[test]
    fn test_manifest_serialization() {
        let mut manifest = AssetManifest::new();
        manifest.add_asset(AssetEntry::new_screenshot(
            "abc123".to_string(),
            "abc123.png".to_string(),
            Some("Login page after changes".to_string()),
        ));

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        assert!(json.contains("abc123"));
        assert!(json.contains("screenshot"));

        // Deserialize back
        let parsed: AssetManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.assets.len(), 1);
        assert_eq!(parsed.assets[0].id, "abc123");
    }
}

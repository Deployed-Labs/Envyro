//! Environment Registry - Share and Discover Environments
//!
//! This module provides the client-side logic for interacting with an
//! Enviro registry (similar to Docker Hub, but for Enviro environments).
//!
//! Users can push, pull, and search environments through the registry.
//!
//! # Architecture:
//! - Local storage under `~/.enviro/environments/`
//! - Manifest-based tracking with JSON metadata
//! - Registry API client for remote operations (future: HTTP API)

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Local storage directory for environments
    pub storage_dir: PathBuf,
    /// Remote registry URL (future use)
    pub remote_url: String,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        let storage_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".enviro")
            .join("environments");

        Self {
            storage_dir,
            remote_url: "https://registry.enviro.dev".to_string(),
        }
    }
}

/// An environment manifest stored in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentManifest {
    /// Unique identifier
    pub id: String,
    /// Human-readable name (e.g., "myuser/web-app")
    pub name: String,
    /// Version string
    pub version: String,
    /// Description
    pub description: String,
    /// Author
    pub author: String,
    /// Tags for search
    pub tags: Vec<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Size in bytes of the environment archive
    pub size_bytes: u64,
    /// SHA256 hash of the environment archive
    pub checksum: String,
    /// Base environment this extends
    pub base: String,
    /// Exposed ports
    pub ports: Vec<u16>,
}

/// Local registry client for managing environments
pub struct Registry {
    config: RegistryConfig,
}

impl Registry {
    /// Create a new registry client with default configuration
    pub fn new() -> Result<Self> {
        Self::with_config(RegistryConfig::default())
    }

    /// Create a new registry client with custom configuration
    pub fn with_config(config: RegistryConfig) -> Result<Self> {
        // Ensure storage directory exists
        std::fs::create_dir_all(&config.storage_dir).with_context(|| {
            format!(
                "Failed to create registry storage at {}",
                config.storage_dir.display()
            )
        })?;

        Ok(Self { config })
    }

    /// List all locally stored environments
    pub fn list(&self) -> Result<Vec<EnvironmentManifest>> {
        let index_path = self.index_path();
        if !index_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&index_path)
            .context("Failed to read registry index")?;
        let index: RegistryIndex =
            serde_json::from_str(&content).context("Failed to parse registry index")?;

        Ok(index.environments)
    }

    /// Search local environments by name or tag
    pub fn search(&self, query: &str) -> Result<Vec<EnvironmentManifest>> {
        let all = self.list()?;
        let query_lower = query.to_lowercase();

        let results: Vec<EnvironmentManifest> = all
            .into_iter()
            .filter(|env| {
                env.name.to_lowercase().contains(&query_lower)
                    || env.description.to_lowercase().contains(&query_lower)
                    || env.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect();

        Ok(results)
    }

    /// Store an environment in the local registry
    pub fn store(&self, manifest: EnvironmentManifest) -> Result<()> {
        let mut index = self.load_index()?;

        // Remove existing entry with same name+version if present
        index
            .environments
            .retain(|e| !(e.name == manifest.name && e.version == manifest.version));

        index.environments.push(manifest);
        self.save_index(&index)?;

        Ok(())
    }

    /// Remove an environment from the local registry by name
    pub fn remove(&self, name: &str) -> Result<bool> {
        let mut index = self.load_index()?;
        let initial_len = index.environments.len();

        index.environments.retain(|e| e.name != name);

        if index.environments.len() < initial_len {
            self.save_index(&index)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get an environment by name (latest version)
    pub fn get(&self, name: &str) -> Result<Option<EnvironmentManifest>> {
        let all = self.list()?;
        Ok(all.into_iter().find(|e| e.name == name))
    }

    /// Create a manifest from an Envirofile
    pub fn manifest_from_envirofile(
        envirofile: &crate::envirofile::Envirofile,
    ) -> EnvironmentManifest {
        let now = Utc::now();
        EnvironmentManifest {
            id: Uuid::new_v4().to_string(),
            name: envirofile.environment.name.clone(),
            version: envirofile.environment.version.clone(),
            description: envirofile.environment.description.clone(),
            author: envirofile.environment.author.clone(),
            tags: envirofile.environment.tags.clone(),
            created_at: now,
            updated_at: now,
            size_bytes: 0,
            checksum: String::new(),
            base: envirofile.environment.base.clone(),
            ports: envirofile.run.ports.clone(),
        }
    }

    /// Get storage directory path
    pub fn storage_dir(&self) -> &Path {
        &self.config.storage_dir
    }

    // --- Private helpers ---

    fn index_path(&self) -> PathBuf {
        self.config.storage_dir.join("index.json")
    }

    fn load_index(&self) -> Result<RegistryIndex> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(RegistryIndex {
                environments: Vec::new(),
            });
        }

        let content =
            std::fs::read_to_string(&path).context("Failed to read registry index")?;
        serde_json::from_str(&content).context("Failed to parse registry index")
    }

    fn save_index(&self, index: &RegistryIndex) -> Result<()> {
        let content =
            serde_json::to_string_pretty(index).context("Failed to serialize registry index")?;
        std::fs::write(self.index_path(), content).context("Failed to write registry index")
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new().expect("Failed to create default registry")
    }
}

/// Internal registry index structure
#[derive(Debug, Serialize, Deserialize)]
struct RegistryIndex {
    environments: Vec<EnvironmentManifest>,
}

/// Format a byte count as a human-readable string
pub fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(dir: &Path) -> RegistryConfig {
        RegistryConfig {
            storage_dir: dir.to_path_buf(),
            remote_url: "https://test.example.com".to_string(),
        }
    }

    fn sample_manifest(name: &str) -> EnvironmentManifest {
        let now = Utc::now();
        EnvironmentManifest {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "Test environment".to_string(),
            author: "tester".to_string(),
            tags: vec!["test".to_string(), "example".to_string()],
            created_at: now,
            updated_at: now,
            size_bytes: 1024,
            checksum: "abc123".to_string(),
            base: "ubuntu:22.04".to_string(),
            ports: vec![8080],
        }
    }

    #[test]
    fn test_registry_creation() {
        let tmp = TempDir::new().unwrap();
        let reg = Registry::with_config(test_config(tmp.path())).unwrap();
        assert!(reg.storage_dir().exists());
    }

    #[test]
    fn test_store_and_list() {
        let tmp = TempDir::new().unwrap();
        let reg = Registry::with_config(test_config(tmp.path())).unwrap();

        // Empty initially
        let envs = reg.list().unwrap();
        assert!(envs.is_empty());

        // Store one
        reg.store(sample_manifest("test/app")).unwrap();
        let envs = reg.list().unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "test/app");

        // Store another
        reg.store(sample_manifest("test/api")).unwrap();
        let envs = reg.list().unwrap();
        assert_eq!(envs.len(), 2);
    }

    #[test]
    fn test_store_replaces_same_version() {
        let tmp = TempDir::new().unwrap();
        let reg = Registry::with_config(test_config(tmp.path())).unwrap();

        reg.store(sample_manifest("test/app")).unwrap();
        reg.store(sample_manifest("test/app")).unwrap();

        let envs = reg.list().unwrap();
        assert_eq!(envs.len(), 1);
    }

    #[test]
    fn test_search() {
        let tmp = TempDir::new().unwrap();
        let reg = Registry::with_config(test_config(tmp.path())).unwrap();

        reg.store(sample_manifest("web/frontend")).unwrap();
        reg.store(sample_manifest("web/backend")).unwrap();
        reg.store(sample_manifest("data/pipeline")).unwrap();

        let results = reg.search("web").unwrap();
        assert_eq!(results.len(), 2);

        let results = reg.search("pipeline").unwrap();
        assert_eq!(results.len(), 1);

        let results = reg.search("test").unwrap();
        assert_eq!(results.len(), 3); // all have "test" tag
    }

    #[test]
    fn test_remove() {
        let tmp = TempDir::new().unwrap();
        let reg = Registry::with_config(test_config(tmp.path())).unwrap();

        reg.store(sample_manifest("test/app")).unwrap();
        assert!(reg.remove("test/app").unwrap());
        assert!(!reg.remove("test/app").unwrap()); // already removed

        let envs = reg.list().unwrap();
        assert!(envs.is_empty());
    }

    #[test]
    fn test_get() {
        let tmp = TempDir::new().unwrap();
        let reg = Registry::with_config(test_config(tmp.path())).unwrap();

        reg.store(sample_manifest("test/app")).unwrap();

        assert!(reg.get("test/app").unwrap().is_some());
        assert!(reg.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_manifest_from_envirofile() {
        let ef = crate::envirofile::parse(
            r#"
[environment]
name = "my-app"
version = "2.0.0"
description = "My app"
author = "me"
tags = ["rust"]

[run]
ports = [3000, 8080]
"#,
        )
        .unwrap();

        let manifest = Registry::manifest_from_envirofile(&ef);
        assert_eq!(manifest.name, "my-app");
        assert_eq!(manifest.version, "2.0.0");
        assert_eq!(manifest.ports, vec![3000, 8080]);
        assert!(!manifest.id.is_empty());
    }
}

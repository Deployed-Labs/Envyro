//! Envirofile - Simple, Declarative Environment Definitions
//!
//! An Envirofile is a TOML-based environment specification that replaces
//! Dockerfiles with a simpler, more intuitive format. Instead of imperative
//! build steps, Envirofiles declare what the environment should contain.
//!
//! # Example Envirofile (Envirofile.toml):
//! ```toml
//! [environment]
//! name = "my-web-app"
//! base = "ubuntu:22.04"
//! description = "A simple web application"
//!
//! [packages]
//! apt = ["nginx", "curl", "git"]
//! pip = ["flask", "gunicorn"]
//!
//! [files]
//! copy = [
//!   { src = "./app", dest = "/opt/app" },
//!   { src = "./config.yml", dest = "/etc/app/config.yml" },
//! ]
//!
//! [env]
//! PORT = "8080"
//! APP_ENV = "production"
//!
//! [run]
//! command = "gunicorn app:app"
//! workdir = "/opt/app"
//! ports = [8080]
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Top-level Envirofile definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envirofile {
    /// Environment metadata
    pub environment: EnvironmentConfig,
    /// Package installation
    #[serde(default)]
    pub packages: PackageConfig,
    /// File operations
    #[serde(default)]
    pub files: FileConfig,
    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Runtime configuration
    #[serde(default)]
    pub run: RunConfig,
    /// Resource limits
    #[serde(default)]
    pub resources: ResourceConfig,
    /// Health check configuration
    #[serde(default)]
    pub health: Option<HealthConfig>,
}

/// Environment metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    /// Name of the environment
    pub name: String,
    /// Base image or environment to extend
    #[serde(default = "default_base")]
    pub base: String,
    /// Human-readable description
    #[serde(default)]
    pub description: String,
    /// Version string
    #[serde(default = "default_version")]
    pub version: String,
    /// Author information
    #[serde(default)]
    pub author: String,
    /// Tags for registry search
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_base() -> String {
    "ubuntu:22.04".to_string()
}

fn default_version() -> String {
    "0.1.0".to_string()
}

/// Package manager configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageConfig {
    /// APT packages (Debian/Ubuntu)
    #[serde(default)]
    pub apt: Vec<String>,
    /// pip packages (Python)
    #[serde(default)]
    pub pip: Vec<String>,
    /// npm packages (Node.js)
    #[serde(default)]
    pub npm: Vec<String>,
    /// Cargo crates (Rust)
    #[serde(default)]
    pub cargo: Vec<String>,
    /// Custom install commands
    #[serde(default)]
    pub custom: Vec<String>,
}

/// File operations (copy, mount)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileConfig {
    /// Files to copy into the environment
    #[serde(default)]
    pub copy: Vec<CopySpec>,
    /// Volumes to mount
    #[serde(default)]
    pub volumes: Vec<VolumeSpec>,
}

/// Specification for copying a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopySpec {
    /// Source path on the host
    pub src: String,
    /// Destination path in the environment
    pub dest: String,
}

/// Specification for mounting a volume
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSpec {
    /// Host path or volume name
    pub source: String,
    /// Mount point inside the environment
    pub target: String,
    /// Read-only flag
    #[serde(default)]
    pub readonly: bool,
}

/// Runtime configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    /// Command to run
    #[serde(default = "default_command")]
    pub command: String,
    /// Working directory
    #[serde(default = "default_workdir")]
    pub workdir: String,
    /// Ports to expose
    #[serde(default)]
    pub ports: Vec<u16>,
    /// Run as specific user
    #[serde(default)]
    pub user: Option<String>,
}

fn default_command() -> String {
    "/bin/sh".to_string()
}

fn default_workdir() -> String {
    "/".to_string()
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            command: default_command(),
            workdir: default_workdir(),
            ports: Vec::new(),
            user: None,
        }
    }
}

/// Resource limits for the environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceConfig {
    /// CPU cores (fractional)
    #[serde(default = "default_cpu")]
    pub cpu: f64,
    /// Memory limit (e.g., "512m", "2g")
    #[serde(default = "default_memory")]
    pub memory: String,
    /// PID limit
    #[serde(default = "default_pids")]
    pub pids: u32,
}

fn default_cpu() -> f64 {
    1.0
}
fn default_memory() -> String {
    "512m".to_string()
}
fn default_pids() -> u32 {
    100
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            cpu: default_cpu(),
            memory: default_memory(),
            pids: default_pids(),
        }
    }
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Command to check health
    pub command: String,
    /// Interval between checks in seconds
    #[serde(default = "default_interval")]
    pub interval: u64,
    /// Timeout for each check in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Number of retries before marking unhealthy
    #[serde(default = "default_retries")]
    pub retries: u32,
}

fn default_interval() -> u64 {
    30
}
fn default_timeout() -> u64 {
    10
}
fn default_retries() -> u32 {
    3
}

/// Parse an Envirofile from a TOML string
pub fn parse(content: &str) -> Result<Envirofile> {
    toml::from_str(content).context("Failed to parse Envirofile")
}

/// Parse an Envirofile from a file path
pub fn parse_file(path: &Path) -> Result<Envirofile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read Envirofile at {}", path.display()))?;
    parse(&content)
}

/// Find and parse an Envirofile in the given directory
///
/// Searches for (in order): Envirofile.toml, envirofile.toml, .envirofile
pub fn find_and_parse(dir: &Path) -> Result<(PathBuf, Envirofile)> {
    let candidates = ["Envirofile.toml", "envirofile.toml", ".envirofile"];

    for name in &candidates {
        let path = dir.join(name);
        if path.exists() {
            let envirofile = parse_file(&path)?;
            return Ok((path, envirofile));
        }
    }

    anyhow::bail!(
        "No Envirofile found in {}. Create one with 'enviro init'.",
        dir.display()
    )
}

/// Parse a memory string like "512m", "2g", "1024k" into bytes
pub fn parse_memory(s: &str) -> Result<u64> {
    let s = s.trim().to_lowercase();
    if s.is_empty() {
        anyhow::bail!("Empty memory string");
    }

    let (num_str, multiplier) = if s.ends_with('g') {
        (&s[..s.len() - 1], 1024 * 1024 * 1024u64)
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], 1024 * 1024u64)
    } else if s.ends_with('k') {
        (&s[..s.len() - 1], 1024u64)
    } else {
        (s.as_str(), 1u64)
    };

    let num: u64 = num_str
        .parse()
        .with_context(|| format!("Invalid memory value: {}", s))?;

    Ok(num * multiplier)
}

/// Generate a scaffold Envirofile.toml for `enviro init`
pub fn scaffold(name: &str) -> String {
    format!(
        r#"# Envirofile - Environment definition for Enviro
# Docs: https://github.com/Deployed-Labs/Envyro

[environment]
name = "{name}"
base = "ubuntu:22.04"
description = "A new Enviro environment"
version = "0.1.0"
tags = []

[packages]
apt = []
# pip = []
# npm = []

[files]
copy = []
# volumes = []

[env]
# KEY = "value"

[run]
command = "/bin/sh"
workdir = "/"
ports = []

[resources]
cpu = 1.0
memory = "512m"
pids = 100

# [health]
# command = "curl -f http://localhost:8080/health"
# interval = 30
# timeout = 10
# retries = 3
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_envirofile() {
        let toml = r#"
[environment]
name = "test-env"
"#;
        let ef = parse(toml).unwrap();
        assert_eq!(ef.environment.name, "test-env");
        assert_eq!(ef.environment.base, "ubuntu:22.04");
        assert_eq!(ef.environment.version, "0.1.0");
    }

    #[test]
    fn test_parse_full_envirofile() {
        let toml = r#"
[environment]
name = "web-app"
base = "node:20"
description = "My web app"
version = "1.0.0"
author = "Dev"
tags = ["web", "node"]

[packages]
apt = ["curl", "git"]
npm = ["express"]

[files]
copy = [
  { src = "./src", dest = "/app/src" },
]
volumes = [
  { source = "data", target = "/data", readonly = true },
]

[env]
PORT = "3000"
NODE_ENV = "production"

[run]
command = "node server.js"
workdir = "/app"
ports = [3000]
user = "node"

[resources]
cpu = 2.0
memory = "1g"
pids = 200

[health]
command = "curl -f http://localhost:3000/health"
interval = 15
timeout = 5
retries = 5
"#;
        let ef = parse(toml).unwrap();
        assert_eq!(ef.environment.name, "web-app");
        assert_eq!(ef.environment.base, "node:20");
        assert_eq!(ef.environment.tags, vec!["web", "node"]);
        assert_eq!(ef.packages.apt, vec!["curl", "git"]);
        assert_eq!(ef.packages.npm, vec!["express"]);
        assert_eq!(ef.files.copy.len(), 1);
        assert_eq!(ef.files.volumes.len(), 1);
        assert!(ef.files.volumes[0].readonly);
        assert_eq!(ef.env.get("PORT").unwrap(), "3000");
        assert_eq!(ef.run.command, "node server.js");
        assert_eq!(ef.run.ports, vec![3000]);
        assert_eq!(ef.resources.cpu, 2.0);
        assert_eq!(ef.resources.memory, "1g");
        let health = ef.health.unwrap();
        assert_eq!(health.interval, 15);
        assert_eq!(health.retries, 5);
    }

    #[test]
    fn test_parse_memory() {
        assert_eq!(parse_memory("512m").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_memory("2g").unwrap(), 2 * 1024 * 1024 * 1024);
        assert_eq!(parse_memory("1024k").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory("1024").unwrap(), 1024);
        assert!(parse_memory("").is_err());
        assert!(parse_memory("abc").is_err());
    }

    #[test]
    fn test_scaffold() {
        let content = scaffold("my-project");
        assert!(content.contains("name = \"my-project\""));
        assert!(content.contains("[environment]"));
        assert!(content.contains("[packages]"));
        assert!(content.contains("[run]"));
        // Verify the scaffold parses successfully
        let ef = parse(&content).unwrap();
        assert_eq!(ef.environment.name, "my-project");
    }

    #[test]
    fn test_parse_invalid_toml() {
        assert!(parse("not valid toml {{{").is_err());
    }

    #[test]
    fn test_find_and_parse_no_file() {
        let result = find_and_parse(Path::new("/tmp/nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn test_default_configs() {
        let run = RunConfig::default();
        assert_eq!(run.command, "/bin/sh");
        assert_eq!(run.workdir, "/");
        assert!(run.ports.is_empty());

        let res = ResourceConfig::default();
        assert_eq!(res.cpu, 1.0);
        assert_eq!(res.memory, "512m");
        assert_eq!(res.pids, 100);
    }
}

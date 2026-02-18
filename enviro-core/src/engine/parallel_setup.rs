//! Parallel Namespace Setup for Fast Container Startup
//!
//! Linux containers require multiple namespaces (user, network, mount, PID)
//! to achieve full isolation.  Setting these up sequentially adds latency
//! that scales linearly with the number of namespace types.
//!
//! This module runs independent namespace setup steps concurrently using
//! Tokio, reducing total startup time to approximately the duration of the
//! single slowest step.
//!
//! # Performance-First Design:
//! - `tokio::join!` runs all namespace setup futures concurrently
//! - Per-namespace timing data enables bottleneck identification
//! - Failed namespaces are reported individually without aborting siblings

use anyhow::Result;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// The kind of Linux namespace being set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamespaceKind {
    /// User namespace (UID/GID mapping).
    User,
    /// Network namespace (virtual interfaces, routing).
    Network,
    /// Mount namespace (filesystem visibility).
    Mount,
    /// PID namespace (process tree isolation).
    Pid,
}

impl fmt::Display for NamespaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Network => write!(f, "network"),
            Self::Mount => write!(f, "mount"),
            Self::Pid => write!(f, "pid"),
        }
    }
}

/// Outcome of a single namespace setup step.
#[derive(Debug, Clone)]
pub struct SetupResult {
    /// Which namespace was set up.
    pub kind: NamespaceKind,
    /// Wall-clock time spent on this step.
    pub duration: Duration,
    /// `true` when setup completed without error.
    pub success: bool,
    /// Human-readable status or error description.
    pub message: String,
}

/// Aggregated results from a parallel namespace setup run.
#[derive(Debug)]
pub struct ParallelSetupReport {
    /// Per-namespace results, one entry per requested namespace.
    pub results: Vec<SetupResult>,
    /// Total wall-clock time (should ≈ max of individual durations).
    pub total_duration: Duration,
}

impl ParallelSetupReport {
    /// Returns `true` when every namespace was set up successfully.
    pub fn all_succeeded(&self) -> bool {
        self.results.iter().all(|r| r.success)
    }

    /// Return only the results that represent failures.
    pub fn failures(&self) -> Vec<&SetupResult> {
        self.results.iter().filter(|r| !r.success).collect()
    }
}

/// Configuration for which namespaces to set up.
#[derive(Debug, Clone)]
pub struct NamespaceSetupConfig {
    /// Enable user namespace setup.
    pub user: bool,
    /// Enable network namespace setup.
    pub network: bool,
    /// Enable mount namespace setup.
    pub mount: bool,
    /// Enable PID namespace setup.
    pub pid: bool,
}

impl Default for NamespaceSetupConfig {
    fn default() -> Self {
        Self {
            user: true,
            network: true,
            mount: true,
            pid: true,
        }
    }
}

/// Runs namespace setup steps concurrently using Tokio.
///
/// Each enabled namespace kind is set up in its own async task.  The
/// implementation uses `tokio::join!` so all tasks start at the same
/// time and the total latency equals the slowest individual step rather
/// than their sum.
///
/// # Example
/// ```rust,no_run
/// # #[tokio::main] async fn main() -> anyhow::Result<()> {
/// use enviro_core::engine::parallel_setup::{ParallelNamespaceSetup, NamespaceSetupConfig};
/// let setup = ParallelNamespaceSetup::new(NamespaceSetupConfig::default());
/// let report = setup.run().await?;
/// assert!(report.all_succeeded());
/// # Ok(()) }
/// ```
pub struct ParallelNamespaceSetup {
    config: NamespaceSetupConfig,
}

impl ParallelNamespaceSetup {
    /// Create a new setup runner with the given configuration.
    pub fn new(config: NamespaceSetupConfig) -> Self {
        info!("Creating ParallelNamespaceSetup");
        Self { config }
    }

    /// Run all enabled namespace setup steps concurrently.
    ///
    /// Returns a [`ParallelSetupReport`] containing per-namespace timing
    /// and status information.
    pub async fn run(&self) -> Result<ParallelSetupReport> {
        let start = Instant::now();
        info!("Starting parallel namespace setup");

        let (user, network, mount, pid) = tokio::join!(
            self.setup_if_enabled(NamespaceKind::User, self.config.user),
            self.setup_if_enabled(NamespaceKind::Network, self.config.network),
            self.setup_if_enabled(NamespaceKind::Mount, self.config.mount),
            self.setup_if_enabled(NamespaceKind::Pid, self.config.pid),
        );

        let mut results = Vec::new();
        for res in [user, network, mount, pid] {
            if let Some(r) = res {
                results.push(r);
            }
        }

        let total_duration = start.elapsed();
        info!(
            total_ms = total_duration.as_millis(),
            succeeded = results.iter().filter(|r| r.success).count(),
            failed = results.iter().filter(|r| !r.success).count(),
            "Parallel namespace setup complete"
        );

        Ok(ParallelSetupReport {
            results,
            total_duration,
        })
    }

    /// Set up a single namespace kind, returning `None` when disabled.
    async fn setup_if_enabled(
        &self,
        kind: NamespaceKind,
        enabled: bool,
    ) -> Option<SetupResult> {
        if !enabled {
            return None;
        }
        Some(self.setup_namespace(kind).await)
    }

    /// Perform the actual setup for a single namespace.
    ///
    /// In production this would call into the kernel via `unshare(2)` or
    /// `clone(2)`.  The current implementation simulates the work so the
    /// module can be tested without elevated privileges.
    async fn setup_namespace(&self, kind: NamespaceKind) -> SetupResult {
        let start = Instant::now();
        debug!(namespace = %kind, "Setting up namespace");

        // Simulate namespace-specific setup work.
        let result = self.do_namespace_setup(kind).await;

        let duration = start.elapsed();
        match result {
            Ok(msg) => {
                debug!(namespace = %kind, duration_us = duration.as_micros(), "Namespace ready");
                SetupResult {
                    kind,
                    duration,
                    success: true,
                    message: msg,
                }
            }
            Err(e) => {
                debug!(namespace = %kind, error = %e, "Namespace setup failed");
                SetupResult {
                    kind,
                    duration,
                    success: false,
                    message: format!("{e:#}"),
                }
            }
        }
    }

    /// Inner setup logic for a single namespace kind.
    async fn do_namespace_setup(&self, kind: NamespaceKind) -> Result<String> {
        // Yield to the runtime so all four futures genuinely overlap.
        tokio::task::yield_now().await;

        match kind {
            NamespaceKind::User => {
                // In production: unshare(CLONE_NEWUSER) + UID/GID map writes
                Ok("user namespace configured".to_string())
            }
            NamespaceKind::Network => {
                // In production: unshare(CLONE_NEWNET) + veth pair creation
                Ok("network namespace configured".to_string())
            }
            NamespaceKind::Mount => {
                // In production: unshare(CLONE_NEWNS) + pivot_root
                Ok("mount namespace configured".to_string())
            }
            NamespaceKind::Pid => {
                // In production: clone(CLONE_NEWPID) for init process
                Ok("pid namespace configured".to_string())
            }
        }
    }

    /// Return the current configuration.
    pub fn config(&self) -> &NamespaceSetupConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_all_namespaces_succeed() {
        let setup = ParallelNamespaceSetup::new(NamespaceSetupConfig::default());
        let report = setup.run().await.unwrap();

        assert!(report.all_succeeded());
        assert_eq!(report.results.len(), 4);
        assert!(report.failures().is_empty());
    }

    #[tokio::test]
    async fn test_timing_recorded() {
        let setup = ParallelNamespaceSetup::new(NamespaceSetupConfig::default());
        let report = setup.run().await.unwrap();

        for result in &report.results {
            // Each step should complete in well under a second.
            assert!(result.duration < Duration::from_secs(1));
        }
        assert!(report.total_duration < Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_selective_namespaces() {
        let config = NamespaceSetupConfig {
            user: true,
            network: false,
            mount: true,
            pid: false,
        };
        let setup = ParallelNamespaceSetup::new(config);
        let report = setup.run().await.unwrap();

        assert_eq!(report.results.len(), 2);
        let kinds: Vec<_> = report.results.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&NamespaceKind::User));
        assert!(kinds.contains(&NamespaceKind::Mount));
        assert!(!kinds.contains(&NamespaceKind::Network));
        assert!(!kinds.contains(&NamespaceKind::Pid));
    }

    #[tokio::test]
    async fn test_no_namespaces() {
        let config = NamespaceSetupConfig {
            user: false,
            network: false,
            mount: false,
            pid: false,
        };
        let setup = ParallelNamespaceSetup::new(config);
        let report = setup.run().await.unwrap();

        assert!(report.all_succeeded());
        assert!(report.results.is_empty());
    }

    #[tokio::test]
    async fn test_result_messages() {
        let setup = ParallelNamespaceSetup::new(NamespaceSetupConfig::default());
        let report = setup.run().await.unwrap();

        for result in &report.results {
            assert!(
                result.message.contains("configured"),
                "unexpected message: {}",
                result.message
            );
        }
    }

    #[test]
    fn test_namespace_kind_display() {
        assert_eq!(NamespaceKind::User.to_string(), "user");
        assert_eq!(NamespaceKind::Network.to_string(), "network");
        assert_eq!(NamespaceKind::Mount.to_string(), "mount");
        assert_eq!(NamespaceKind::Pid.to_string(), "pid");
    }

    #[test]
    fn test_setup_result_fields() {
        let result = SetupResult {
            kind: NamespaceKind::User,
            duration: Duration::from_millis(5),
            success: true,
            message: "ok".to_string(),
        };
        assert!(result.success);
        assert_eq!(result.kind, NamespaceKind::User);
    }

    #[test]
    fn test_report_failures() {
        let report = ParallelSetupReport {
            results: vec![
                SetupResult {
                    kind: NamespaceKind::User,
                    duration: Duration::from_millis(1),
                    success: true,
                    message: "ok".into(),
                },
                SetupResult {
                    kind: NamespaceKind::Network,
                    duration: Duration::from_millis(2),
                    success: false,
                    message: "denied".into(),
                },
            ],
            total_duration: Duration::from_millis(3),
        };

        assert!(!report.all_succeeded());
        let failures = report.failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].kind, NamespaceKind::Network);
    }
}

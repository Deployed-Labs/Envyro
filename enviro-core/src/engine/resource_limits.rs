//! Resource Limit Optimization via Batched Cgroup Operations
//!
//! Applying cgroup resource limits one at a time (memory, CPU, I/O) requires
//! a separate filesystem write for each parameter.  When setting many limits
//! during container startup this generates a burst of small writes that
//! contend on the cgroup VFS locks.
//!
//! This module batches multiple limit changes into a single logical operation
//! so they can be applied in one pass, reducing both syscall count and lock
//! contention.
//!
//! # Performance-First Design:
//! - `ResourceLimitBatch` collects changes and applies them in one pass
//! - Preset `ResourceProfile`s avoid per-container configuration overhead
//! - Timing data from `apply_batch` enables startup optimization

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Identifies a single cgroup resource parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    /// Memory limit in bytes (e.g. `memory.max`).
    MemoryMax,
    /// Memory soft limit / high watermark (`memory.high`).
    MemoryHigh,
    /// CPU weight (1–10000, maps to `cpu.weight`).
    CpuWeight,
    /// Maximum CPU bandwidth in microseconds per period (`cpu.max`).
    CpuMaxMicros,
    /// I/O weight (1–10000, maps to `io.weight`).
    IoWeight,
    /// Maximum number of PIDs (`pids.max`).
    PidsMax,
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MemoryMax => write!(f, "memory.max"),
            Self::MemoryHigh => write!(f, "memory.high"),
            Self::CpuWeight => write!(f, "cpu.weight"),
            Self::CpuMaxMicros => write!(f, "cpu.max"),
            Self::IoWeight => write!(f, "io.weight"),
            Self::PidsMax => write!(f, "pids.max"),
        }
    }
}

/// A single resource limit change.
#[derive(Debug, Clone)]
struct LimitEntry {
    kind: ResourceKind,
    value: u64,
}

/// Collects multiple resource limit changes and applies them in a single pass.
///
/// # Performance Pattern: Batched Cgroup Writes
/// ```rust,no_run
/// # use enviro_core::engine::resource_limits::{ResourceLimitBatch, ResourceKind};
/// let mut batch = ResourceLimitBatch::new();
/// batch.add_limit(ResourceKind::MemoryMax, 512 * 1024 * 1024);
/// batch.add_limit(ResourceKind::CpuWeight, 100);
/// let report = batch.apply_batch().unwrap();
/// ```
pub struct ResourceLimitBatch {
    entries: Vec<LimitEntry>,
}

impl ResourceLimitBatch {
    /// Create a new, empty batch.
    pub fn new() -> Self {
        debug!("Creating ResourceLimitBatch");
        Self {
            entries: Vec::new(),
        }
    }

    /// Add a limit change to the batch.
    ///
    /// Multiple changes for the same [`ResourceKind`] are allowed; the last
    /// one added wins when the batch is applied.
    pub fn add_limit(&mut self, kind: ResourceKind, value: u64) {
        debug!(resource = %kind, value, "Queuing resource limit");
        self.entries.push(LimitEntry { kind, value });
    }

    /// Apply all queued limit changes in one pass.
    ///
    /// Returns a [`BatchApplyReport`] with per-entry timing.  In production
    /// each entry would write to the corresponding cgroup control file;
    /// batching ensures the writes happen back-to-back with no intervening
    /// user-space work.
    pub fn apply_batch(&self) -> Result<BatchApplyReport> {
        let start = Instant::now();
        info!(count = self.entries.len(), "Applying resource limit batch");

        // Deduplicate: last writer wins (same semantics as cgroup FS).
        let mut deduped: HashMap<&ResourceKind, u64> = HashMap::new();
        for entry in &self.entries {
            deduped.insert(&entry.kind, entry.value);
        }

        let mut results = Vec::with_capacity(deduped.len());
        for (kind, value) in &deduped {
            let entry_start = Instant::now();
            // In production: write `value` to `/sys/fs/cgroup/<container>/<kind>`
            Self::apply_single(kind, *value)
                .with_context(|| format!("Failed to apply {kind}"))?;
            results.push(LimitApplyResult {
                kind: (*kind).clone(),
                value: *value,
                duration: entry_start.elapsed(),
            });
        }

        let total_duration = start.elapsed();
        debug!(
            total_ms = total_duration.as_millis(),
            applied = results.len(),
            "Batch apply complete"
        );

        Ok(BatchApplyReport {
            results,
            total_duration,
        })
    }

    /// Apply a single limit (simulated).
    fn apply_single(kind: &ResourceKind, value: u64) -> Result<()> {
        debug!(resource = %kind, value, "Writing cgroup control file");
        // Real implementation writes to /sys/fs/cgroup/…
        Ok(())
    }

    /// Return the number of limit changes currently queued.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no limit changes are queued.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for ResourceLimitBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of applying a single resource limit.
#[derive(Debug, Clone)]
pub struct LimitApplyResult {
    /// Which resource was set.
    pub kind: ResourceKind,
    /// The value that was written.
    pub value: u64,
    /// Time spent writing this single control file.
    pub duration: Duration,
}

/// Aggregated results from [`ResourceLimitBatch::apply_batch`].
#[derive(Debug)]
pub struct BatchApplyReport {
    /// Per-limit results.
    pub results: Vec<LimitApplyResult>,
    /// Total wall-clock time for the entire batch.
    pub total_duration: Duration,
}

/// Preset resource profiles for common workload shapes.
///
/// Using a profile avoids manually specifying individual limits and
/// ensures consistent configuration across containers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceProfile {
    /// Low-resource profile for sidecar / init containers.
    Minimal,
    /// Balanced defaults suitable for most workloads.
    Standard,
    /// High-resource profile for compute-intensive tasks.
    Performance,
    /// Fully user-defined limits.
    Custom(HashMap<String, u64>),
}

/// Pre-configured resource limits for container workloads.
///
/// `OptimizedResourceLimits` combines a [`ResourceProfile`] with the
/// ability to build and apply a [`ResourceLimitBatch`] in one call.
///
/// # Performance Pattern: Profile-Based Setup
/// ```rust,no_run
/// # use enviro_core::engine::resource_limits::{OptimizedResourceLimits, ResourceProfile};
/// let limits = OptimizedResourceLimits::from_profile(ResourceProfile::Standard);
/// let report = limits.apply().unwrap();
/// ```
pub struct OptimizedResourceLimits {
    profile: ResourceProfile,
    overrides: Vec<LimitEntry>,
}

impl OptimizedResourceLimits {
    /// Create limits from a preset profile.
    pub fn from_profile(profile: ResourceProfile) -> Self {
        info!(profile = ?profile, "Creating OptimizedResourceLimits");
        Self {
            profile,
            overrides: Vec::new(),
        }
    }

    /// Override a single limit after selecting a profile.
    pub fn set_override(&mut self, kind: ResourceKind, value: u64) {
        debug!(resource = %kind, value, "Adding limit override");
        self.overrides.push(LimitEntry { kind, value });
    }

    /// Build a [`ResourceLimitBatch`] from the profile defaults plus
    /// any overrides, then apply it.
    pub fn apply(&self) -> Result<BatchApplyReport> {
        let mut batch = self.build_batch();
        for entry in &self.overrides {
            batch.add_limit(entry.kind.clone(), entry.value);
        }
        batch.apply_batch()
    }

    /// Return a snapshot of the current effective limits (profile + overrides).
    pub fn get_current_limits(&self) -> HashMap<ResourceKind, u64> {
        let mut limits = self.profile_defaults();
        for entry in &self.overrides {
            limits.insert(entry.kind.clone(), entry.value);
        }
        limits
    }

    /// Return the active profile.
    pub fn profile(&self) -> &ResourceProfile {
        &self.profile
    }

    // ── private helpers ───────────────────────────────────────────────

    fn build_batch(&self) -> ResourceLimitBatch {
        let defaults = self.profile_defaults();
        let mut batch = ResourceLimitBatch::new();
        for (kind, value) in defaults {
            batch.add_limit(kind, value);
        }
        batch
    }

    fn profile_defaults(&self) -> HashMap<ResourceKind, u64> {
        match &self.profile {
            ResourceProfile::Minimal => HashMap::from([
                (ResourceKind::MemoryMax, 128 * 1024 * 1024),   // 128 MiB
                (ResourceKind::MemoryHigh, 96 * 1024 * 1024),   // 96 MiB
                (ResourceKind::CpuWeight, 50),
                (ResourceKind::CpuMaxMicros, 50_000),           // 50 ms / period
                (ResourceKind::IoWeight, 50),
                (ResourceKind::PidsMax, 64),
            ]),
            ResourceProfile::Standard => HashMap::from([
                (ResourceKind::MemoryMax, 512 * 1024 * 1024),   // 512 MiB
                (ResourceKind::MemoryHigh, 384 * 1024 * 1024),  // 384 MiB
                (ResourceKind::CpuWeight, 100),
                (ResourceKind::CpuMaxMicros, 100_000),          // 100 ms / period
                (ResourceKind::IoWeight, 100),
                (ResourceKind::PidsMax, 512),
            ]),
            ResourceProfile::Performance => HashMap::from([
                (ResourceKind::MemoryMax, 4 * 1024 * 1024 * 1024),  // 4 GiB
                (ResourceKind::MemoryHigh, 3 * 1024 * 1024 * 1024), // 3 GiB
                (ResourceKind::CpuWeight, 1000),
                (ResourceKind::CpuMaxMicros, 1_000_000),            // 1 s / period
                (ResourceKind::IoWeight, 500),
                (ResourceKind::PidsMax, 4096),
            ]),
            ResourceProfile::Custom(map) => {
                // Convert string keys back to ResourceKind where recognized.
                let mut defaults = HashMap::new();
                for (key, value) in map {
                    if let Some(kind) = Self::parse_kind(key) {
                        defaults.insert(kind, *value);
                    }
                }
                defaults
            }
        }
    }

    fn parse_kind(key: &str) -> Option<ResourceKind> {
        match key {
            "memory.max" => Some(ResourceKind::MemoryMax),
            "memory.high" => Some(ResourceKind::MemoryHigh),
            "cpu.weight" => Some(ResourceKind::CpuWeight),
            "cpu.max" => Some(ResourceKind::CpuMaxMicros),
            "io.weight" => Some(ResourceKind::IoWeight),
            "pids.max" => Some(ResourceKind::PidsMax),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ResourceLimitBatch tests ──────────────────────────────────────

    #[test]
    fn test_batch_new_empty() {
        let batch = ResourceLimitBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
    }

    #[test]
    fn test_batch_add_and_len() {
        let mut batch = ResourceLimitBatch::new();
        batch.add_limit(ResourceKind::MemoryMax, 1024);
        batch.add_limit(ResourceKind::CpuWeight, 100);
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn test_batch_apply_empty() {
        let batch = ResourceLimitBatch::new();
        let report = batch.apply_batch().unwrap();
        assert!(report.results.is_empty());
    }

    #[test]
    fn test_batch_apply_deduplicates() {
        let mut batch = ResourceLimitBatch::new();
        batch.add_limit(ResourceKind::MemoryMax, 100);
        batch.add_limit(ResourceKind::MemoryMax, 200);
        let report = batch.apply_batch().unwrap();
        // After dedup, only one MemoryMax entry should be applied.
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].value, 200);
    }

    #[test]
    fn test_batch_apply_timing() {
        let mut batch = ResourceLimitBatch::new();
        batch.add_limit(ResourceKind::CpuWeight, 500);
        let report = batch.apply_batch().unwrap();
        assert!(report.total_duration < Duration::from_secs(1));
    }

    #[test]
    fn test_batch_default() {
        let batch = ResourceLimitBatch::default();
        assert!(batch.is_empty());
    }

    // ── ResourceProfile tests ─────────────────────────────────────────

    #[test]
    fn test_profile_minimal() {
        let limits = OptimizedResourceLimits::from_profile(ResourceProfile::Minimal);
        let current = limits.get_current_limits();
        assert_eq!(current[&ResourceKind::MemoryMax], 128 * 1024 * 1024);
        assert_eq!(current[&ResourceKind::PidsMax], 64);
    }

    #[test]
    fn test_profile_standard() {
        let limits = OptimizedResourceLimits::from_profile(ResourceProfile::Standard);
        let current = limits.get_current_limits();
        assert_eq!(current[&ResourceKind::MemoryMax], 512 * 1024 * 1024);
        assert_eq!(current[&ResourceKind::CpuWeight], 100);
    }

    #[test]
    fn test_profile_performance() {
        let limits = OptimizedResourceLimits::from_profile(ResourceProfile::Performance);
        let current = limits.get_current_limits();
        assert_eq!(current[&ResourceKind::MemoryMax], 4 * 1024 * 1024 * 1024);
        assert_eq!(current[&ResourceKind::PidsMax], 4096);
    }

    #[test]
    fn test_profile_custom() {
        let custom = HashMap::from([
            ("memory.max".to_string(), 256_u64),
            ("cpu.weight".to_string(), 75),
        ]);
        let limits = OptimizedResourceLimits::from_profile(ResourceProfile::Custom(custom));
        let current = limits.get_current_limits();
        assert_eq!(current[&ResourceKind::MemoryMax], 256);
        assert_eq!(current[&ResourceKind::CpuWeight], 75);
    }

    #[test]
    fn test_override() {
        let mut limits = OptimizedResourceLimits::from_profile(ResourceProfile::Standard);
        limits.set_override(ResourceKind::MemoryMax, 1024);
        let current = limits.get_current_limits();
        assert_eq!(current[&ResourceKind::MemoryMax], 1024);
        // Non-overridden values should remain at profile defaults.
        assert_eq!(current[&ResourceKind::CpuWeight], 100);
    }

    #[test]
    fn test_apply_succeeds() {
        let limits = OptimizedResourceLimits::from_profile(ResourceProfile::Standard);
        let report = limits.apply().unwrap();
        assert!(!report.results.is_empty());
        assert!(report.total_duration < Duration::from_secs(1));
    }

    #[test]
    fn test_apply_with_overrides() {
        let mut limits = OptimizedResourceLimits::from_profile(ResourceProfile::Minimal);
        limits.set_override(ResourceKind::PidsMax, 256);
        let report = limits.apply().unwrap();
        // The override should be reflected in the applied values.
        let pids_result = report
            .results
            .iter()
            .find(|r| r.kind == ResourceKind::PidsMax)
            .unwrap();
        assert_eq!(pids_result.value, 256);
    }

    #[test]
    fn test_resource_kind_display() {
        assert_eq!(ResourceKind::MemoryMax.to_string(), "memory.max");
        assert_eq!(ResourceKind::CpuWeight.to_string(), "cpu.weight");
        assert_eq!(ResourceKind::PidsMax.to_string(), "pids.max");
    }

    #[test]
    fn test_profile_accessor() {
        let limits = OptimizedResourceLimits::from_profile(ResourceProfile::Performance);
        assert_eq!(*limits.profile(), ResourceProfile::Performance);
    }
}

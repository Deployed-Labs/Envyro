//! Memory Pool for Container Execution Contexts
//!
//! This module provides a pool of pre-allocated [`ExecutionContext`] slots that
//! can be reused across container lifecycles.  Allocating and dropping contexts
//! on every request adds GC-like pressure to the global allocator; pooling
//! amortizes that cost by recycling contexts through a free list.
//!
//! # Performance-First Design:
//! - `VecDeque`-backed free list for O(1) acquire/release
//! - Tracks peak usage so operators can right-size the pool
//! - `shrink_to_fit()` reclaims excess capacity during quiet periods

use std::collections::{HashMap, VecDeque};
use tracing::{debug, info};

use crate::executor::{ExecutionContext, NetworkConfig, ResourceLimits};

/// Statistics about pool utilization.
///
/// Operators can use these counters to tune the initial pool size and
/// decide when to call [`ContextPool::shrink_to_fit`].
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total slots currently held by the pool (free + active).
    pub pool_size: usize,
    /// Slots currently checked out via [`ContextPool::acquire`].
    pub active_count: usize,
    /// Cumulative number of times a slot was returned and reused.
    pub recycled_count: u64,
    /// Highest `active_count` observed since the pool was created.
    pub peak_usage: usize,
}

/// A pool of reusable [`ExecutionContext`] instances.
///
/// # Performance Pattern: Object Reuse via Free List
/// ```rust,no_run
/// # use enviro_core::engine::memory_pool::ContextPool;
/// let mut pool = ContextPool::new(8);
/// let ctx = pool.acquire("ctr-1");
/// // … use ctx …
/// pool.release(ctx);
/// ```
pub struct ContextPool {
    /// Free list of contexts available for reuse.
    free_list: VecDeque<ExecutionContext>,
    /// Number of contexts currently checked out.
    active_count: usize,
    /// Cumulative recycle counter.
    recycled_count: u64,
    /// High-water mark for active contexts.
    peak_usage: usize,
}

impl ContextPool {
    /// Create a new pool pre-populated with `capacity` default contexts.
    pub fn new(capacity: usize) -> Self {
        info!(capacity, "Creating ContextPool");
        let mut free_list = VecDeque::with_capacity(capacity);
        for _ in 0..capacity {
            free_list.push_back(Self::default_context());
        }
        Self {
            free_list,
            active_count: 0,
            recycled_count: 0,
            peak_usage: 0,
        }
    }

    /// Acquire a context from the pool, reusing a free slot when available.
    ///
    /// If the free list is empty a fresh context is allocated on the fly.
    /// The returned context has its `container_id` set to `container_id`.
    pub fn acquire(&mut self, container_id: impl Into<String>) -> ExecutionContext {
        let mut ctx = self.free_list.pop_front().unwrap_or_else(|| {
            debug!("ContextPool exhausted – allocating new slot");
            Self::default_context()
        });
        ctx.container_id = container_id.into();
        self.active_count += 1;
        if self.active_count > self.peak_usage {
            self.peak_usage = self.active_count;
        }
        debug!(
            container_id = %ctx.container_id,
            active = self.active_count,
            "Context acquired from pool"
        );
        ctx
    }

    /// Return a context to the pool for future reuse.
    ///
    /// The context is reset to defaults before being pushed onto the free
    /// list so that stale data is never leaked between containers.
    pub fn release(&mut self, mut ctx: ExecutionContext) {
        debug!(container_id = %ctx.container_id, "Releasing context back to pool");
        // Reset mutable fields to prevent data leakage.
        ctx.container_id.clear();
        ctx.env.clear();
        self.active_count = self.active_count.saturating_sub(1);
        self.recycled_count += 1;
        self.free_list.push_back(ctx);
    }

    /// Return a snapshot of current pool statistics.
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            pool_size: self.free_list.len() + self.active_count,
            active_count: self.active_count,
            recycled_count: self.recycled_count,
            peak_usage: self.peak_usage,
        }
    }

    /// Shrink the free list to match the peak observed usage.
    ///
    /// Call this during quiet periods to release memory that is unlikely to
    /// be needed again.
    pub fn shrink_to_fit(&mut self) {
        let target = self.peak_usage.max(1);
        while self.free_list.len() > target {
            self.free_list.pop_back();
        }
        self.free_list.shrink_to_fit();
        debug!(
            new_capacity = self.free_list.len(),
            "ContextPool shrunk to fit"
        );
    }

    /// Build a default, empty execution context.
    fn default_context() -> ExecutionContext {
        ExecutionContext {
            container_id: String::new(),
            env: HashMap::new(),
            workdir: "/".to_string(),
            limits: ResourceLimits {
                cpu_cores: 1.0,
                memory_bytes: 256 * 1024 * 1024,
                pid_limit: 128,
            },
            network: NetworkConfig {
                isolated: true,
                ip_address: None,
                dns_servers: Vec::new(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_acquire_returns_context() {
        let mut pool = ContextPool::new(2);
        let ctx = pool.acquire("ctr-1");
        assert_eq!(ctx.container_id, "ctr-1");
    }

    #[test]
    fn test_pool_release_and_reuse() {
        let mut pool = ContextPool::new(1);
        let ctx = pool.acquire("ctr-1");
        pool.release(ctx);

        let ctx2 = pool.acquire("ctr-2");
        assert_eq!(ctx2.container_id, "ctr-2");
        // The recycled context should have had its env cleared.
        assert!(ctx2.env.is_empty());
    }

    #[test]
    fn test_pool_stats_tracking() {
        let mut pool = ContextPool::new(4);
        assert_eq!(pool.stats().pool_size, 4);
        assert_eq!(pool.stats().active_count, 0);

        let c1 = pool.acquire("a");
        let c2 = pool.acquire("b");
        assert_eq!(pool.stats().active_count, 2);
        assert_eq!(pool.stats().peak_usage, 2);

        pool.release(c1);
        assert_eq!(pool.stats().active_count, 1);
        assert_eq!(pool.stats().recycled_count, 1);

        pool.release(c2);
        assert_eq!(pool.stats().active_count, 0);
        assert_eq!(pool.stats().recycled_count, 2);
        // Peak should still be 2.
        assert_eq!(pool.stats().peak_usage, 2);
    }

    #[test]
    fn test_pool_grows_beyond_capacity() {
        let mut pool = ContextPool::new(1);
        let c1 = pool.acquire("a");
        let c2 = pool.acquire("b"); // exceeds initial capacity
        assert_eq!(pool.stats().active_count, 2);
        pool.release(c1);
        pool.release(c2);
    }

    #[test]
    fn test_shrink_to_fit() {
        let mut pool = ContextPool::new(8);
        let c1 = pool.acquire("a");
        pool.release(c1);
        // peak_usage == 1, free_list has 8 items
        pool.shrink_to_fit();
        assert!(pool.free_list.len() <= 1);
    }

    #[test]
    fn test_release_clears_sensitive_fields() {
        let mut pool = ContextPool::new(1);
        let mut ctx = pool.acquire("secret");
        ctx.env.insert("TOKEN".into(), "abc123".into());
        pool.release(ctx);

        let reused = pool.acquire("other");
        assert!(reused.env.is_empty());
        assert!(reused.container_id == "other");
    }
}

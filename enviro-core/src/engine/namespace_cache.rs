//! Cached Namespace Templates for Fast Container Startup
//!
//! Creating Linux namespaces involves several kernel interactions (clone flags,
//! UID/GID map writes, setgroups deny).  When many containers share similar
//! isolation profiles, re-computing the same configuration for each launch is
//! wasteful.
//!
//! This module caches pre-computed [`NamespaceTemplate`]s keyed by a caller-
//! supplied name, so repeated container starts with the same profile hit the
//! cache instead of rebuilding from scratch.
//!
//! # Performance-First Design:
//! - O(1) lookup of previously computed namespace configurations
//! - Templates are cheaply cloneable (small struct of primitive fields)
//! - Explicit invalidation keeps stale entries under the caller's control

use std::collections::HashMap;
use tracing::{debug, info};

/// A pre-computed namespace configuration that can be applied to new containers.
///
/// `NamespaceTemplate` captures the full set of parameters needed to create an
/// isolated namespace—clone flags, UID/GID ranges, and toggle flags for
/// individual namespace types.  Templates are [`Clone`] so they can be stored
/// in a cache and handed out without ownership transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceTemplate {
    /// Human-readable name for this template (e.g. `"default"`, `"network-only"`).
    pub name: String,
    /// Host UID mapped to container root.
    pub host_uid: u32,
    /// Host GID mapped to container root.
    pub host_gid: u32,
    /// Number of UIDs to map.
    pub uid_range: u32,
    /// Number of GIDs to map.
    pub gid_range: u32,
    /// Isolate the network namespace.
    pub isolate_network: bool,
    /// Isolate the mount namespace.
    pub isolate_mount: bool,
    /// Isolate the PID namespace.
    pub isolate_pid: bool,
}

impl NamespaceTemplate {
    /// Create a new template with the given name and default isolation settings.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            host_uid: 1000,
            host_gid: 1000,
            uid_range: 65536,
            gid_range: 65536,
            isolate_network: true,
            isolate_mount: true,
            isolate_pid: true,
        }
    }
}

/// Runtime statistics for a [`NamespaceCache`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheStats {
    /// Number of cache hits (returned an existing template).
    pub hits: usize,
    /// Number of cache misses (had to create a new template).
    pub misses: usize,
    /// Current number of cached templates.
    pub cached_count: usize,
}

/// Cache for previously created [`NamespaceTemplate`]s.
///
/// `NamespaceCache` wraps a `HashMap` keyed by template name.  The
/// [`get_or_create`](Self::get_or_create) method accepts a closure that
/// builds a template on a cache miss, keeping the creation logic with the
/// caller while the cache owns storage and statistics.
///
/// # Performance Pattern: Template Reuse
/// ```rust,no_run
/// # use enviro_core::engine::namespace_cache::{NamespaceCache, NamespaceTemplate};
/// let mut cache = NamespaceCache::new();
/// let tpl = cache.get_or_create("default", || NamespaceTemplate::new("default"));
/// ```
pub struct NamespaceCache {
    entries: HashMap<String, NamespaceTemplate>,
    hits: usize,
    misses: usize,
}

impl NamespaceCache {
    /// Create a new, empty namespace cache.
    pub fn new() -> Self {
        info!("Creating NamespaceCache");
        Self {
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Return a cached template for `name`, or create one using `init` on a miss.
    ///
    /// The returned reference is a *clone* of the cached template so the caller
    /// may mutate it without affecting the cache.
    pub fn get_or_create<F>(&mut self, name: &str, init: F) -> NamespaceTemplate
    where
        F: FnOnce() -> NamespaceTemplate,
    {
        if let Some(tpl) = self.entries.get(name) {
            self.hits += 1;
            debug!(name, "Namespace template cache hit");
            return tpl.clone();
        }

        self.misses += 1;
        debug!(name, "Namespace template cache miss — creating");
        let tpl = init();
        self.entries.insert(name.to_owned(), tpl.clone());
        tpl
    }

    /// Remove a cached template, forcing re-creation on the next access.
    ///
    /// Returns `true` if an entry was actually removed.
    pub fn invalidate(&mut self, name: &str) -> bool {
        let removed = self.entries.remove(name).is_some();
        if removed {
            debug!(name, "Namespace template invalidated");
        }
        removed
    }

    /// Snapshot the cache's runtime statistics.
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits,
            misses: self.misses,
            cached_count: self.entries.len(),
        }
    }
}

impl Default for NamespaceCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_new() {
        let tpl = NamespaceTemplate::new("test");
        assert_eq!(tpl.name, "test");
        assert_eq!(tpl.uid_range, 65536);
        assert!(tpl.isolate_network);
        assert!(tpl.isolate_mount);
        assert!(tpl.isolate_pid);
    }

    #[test]
    fn test_cache_miss_then_hit() {
        let mut cache = NamespaceCache::new();

        // First call — miss
        let tpl = cache.get_or_create("default", || NamespaceTemplate::new("default"));
        assert_eq!(tpl.name, "default");
        assert_eq!(cache.cache_stats().misses, 1);
        assert_eq!(cache.cache_stats().hits, 0);

        // Second call — hit
        let tpl2 = cache.get_or_create("default", || panic!("should not be called"));
        assert_eq!(tpl2.name, "default");
        assert_eq!(cache.cache_stats().misses, 1);
        assert_eq!(cache.cache_stats().hits, 1);
    }

    #[test]
    fn test_cache_invalidate() {
        let mut cache = NamespaceCache::new();

        cache.get_or_create("net-only", || {
            let mut tpl = NamespaceTemplate::new("net-only");
            tpl.isolate_mount = false;
            tpl.isolate_pid = false;
            tpl
        });

        assert!(cache.invalidate("net-only"));
        assert!(!cache.invalidate("net-only")); // already removed

        assert_eq!(cache.cache_stats().cached_count, 0);
    }

    #[test]
    fn test_cache_invalidate_nonexistent() {
        let mut cache = NamespaceCache::new();
        assert!(!cache.invalidate("does-not-exist"));
    }

    #[test]
    fn test_cache_stats() {
        let mut cache = NamespaceCache::new();

        cache.get_or_create("a", || NamespaceTemplate::new("a"));
        cache.get_or_create("b", || NamespaceTemplate::new("b"));
        cache.get_or_create("a", || panic!("should reuse"));

        let stats = cache.cache_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.cached_count, 2);
    }

    #[test]
    fn test_cache_recreate_after_invalidate() {
        let mut cache = NamespaceCache::new();

        cache.get_or_create("x", || {
            let mut tpl = NamespaceTemplate::new("x");
            tpl.host_uid = 1000;
            tpl
        });

        cache.invalidate("x");

        let tpl = cache.get_or_create("x", || {
            let mut tpl = NamespaceTemplate::new("x");
            tpl.host_uid = 2000;
            tpl
        });

        assert_eq!(tpl.host_uid, 2000);
        // miss count should be 2 (original + after invalidate)
        assert_eq!(cache.cache_stats().misses, 2);
    }

    #[test]
    fn test_default_impl() {
        let cache = NamespaceCache::default();
        assert_eq!(cache.cache_stats().cached_count, 0);
    }
}

//! Lazy Initialization Patterns for Container Startup Optimization
//!
//! This module provides generic lazy initialization primitives that defer
//! expensive resource creation until first use.  In a container runtime,
//! many subsystems (network stacks, filesystem drivers, cgroup controllers)
//! are only needed under specific workloads.  Eagerly initializing all of
//! them at startup adds hundreds of milliseconds of latency that this
//! module eliminates.
//!
//! # Performance-First Design:
//! - `OnceLock`-backed lazy init with zero overhead after first access
//! - Thread-safe without runtime locking on the hot path
//! - `LazyResourcePool` manages collections of lazily-initialized resources

use std::collections::HashMap;
use std::sync::OnceLock;
use tracing::{debug, info};

/// A wrapper that defers creation of an expensive resource until first access.
///
/// `LazyResource<T>` uses [`OnceLock`] internally, so the initialization
/// closure runs at most once, and subsequent calls to [`get_or_init`] return
/// a reference with no synchronization overhead.
///
/// # Performance Pattern: Deferred Initialization
/// ```rust,no_run
/// # use enviro_core::engine::lazy_init::LazyResource;
/// let res = LazyResource::<String>::new("db-conn");
/// // No work done yet…
/// let val = res.get_or_init(|| "connected".to_string());
/// ```
pub struct LazyResource<T> {
    /// Human-readable label for logging.
    name: String,
    /// Thread-safe, initialize-once cell.
    inner: OnceLock<T>,
}

impl<T> LazyResource<T> {
    /// Create a new, uninitialized lazy resource with the given label.
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        debug!(name = %name, "Creating LazyResource (deferred)");
        Self {
            name,
            inner: OnceLock::new(),
        }
    }

    /// Return the contained value, initializing it with `init` on first call.
    ///
    /// The closure is invoked at most once.  All subsequent calls return the
    /// cached value without any synchronization cost.
    pub fn get_or_init<F>(&self, init: F) -> &T
    where
        F: FnOnce() -> T,
    {
        self.inner.get_or_init(|| {
            debug!(name = %self.name, "Initializing LazyResource");
            init()
        })
    }

    /// Returns `true` if the resource has already been initialized.
    pub fn is_initialized(&self) -> bool {
        self.inner.get().is_some()
    }

    /// Consume the wrapper and return the inner value, if initialized.
    pub fn into_inner(self) -> Option<T> {
        self.inner.into_inner()
    }
}

/// A pool of named, lazily-initialized resources.
///
/// `LazyResourcePool` maintains a registry of [`LazyResource`] instances
/// keyed by name.  Resources are only created when first requested via
/// [`get_or_init`](Self::get_or_init), keeping container startup fast
/// by skipping subsystems that the workload never touches.
///
/// # Performance Pattern: On-Demand Subsystem Activation
/// ```rust,no_run
/// # use enviro_core::engine::lazy_init::LazyResourcePool;
/// let mut pool = LazyResourcePool::<String>::new();
/// pool.register("network");
/// pool.register("cgroup");
/// // Only "network" is initialized — "cgroup" stays dormant
/// let net = pool.get_or_init("network", || "net-ready".to_string());
/// ```
pub struct LazyResourcePool<T> {
    resources: HashMap<String, LazyResource<T>>,
}

impl<T> LazyResourcePool<T> {
    /// Create a new, empty resource pool.
    pub fn new() -> Self {
        info!("Creating LazyResourcePool");
        Self {
            resources: HashMap::new(),
        }
    }

    /// Register a named slot in the pool without initializing it.
    pub fn register(&mut self, name: impl Into<String>) {
        let name = name.into();
        debug!(name = %name, "Registering lazy resource slot");
        self.resources
            .entry(name.clone())
            .or_insert_with(|| LazyResource::new(name));
    }

    /// Return the value for `name`, initializing it with `init` on first call.
    ///
    /// If the name has not been registered yet, it is registered automatically.
    pub fn get_or_init<F>(&mut self, name: &str, init: F) -> &T
    where
        F: FnOnce() -> T,
    {
        if !self.resources.contains_key(name) {
            self.register(name.to_owned());
        }
        self.resources
            .get(name)
            .expect("resource was just registered")
            .get_or_init(init)
    }

    /// Returns `true` if the named resource has been initialized.
    ///
    /// Returns `false` both when the name is unknown and when the slot
    /// exists but has not been accessed yet.
    pub fn is_initialized(&self, name: &str) -> bool {
        self.resources
            .get(name)
            .map_or(false, |r| r.is_initialized())
    }

    /// Remove a named resource from the pool, dropping any cached value.
    ///
    /// This forces re-initialization on the next access if the name is
    /// re-registered.
    pub fn reset(&mut self, name: &str) -> bool {
        let removed = self.resources.remove(name).is_some();
        if removed {
            debug!(name, "LazyResource reset (removed from pool)");
        }
        removed
    }

    /// Return the number of registered resource slots.
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Returns `true` when the pool contains no registered slots.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

impl<T> Default for LazyResourcePool<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_resource_deferred() {
        let res = LazyResource::<String>::new("test");
        assert!(!res.is_initialized());
    }

    #[test]
    fn test_lazy_resource_init_once() {
        let res = LazyResource::<u32>::new("counter");
        let val = res.get_or_init(|| 42);
        assert_eq!(*val, 42);
        assert!(res.is_initialized());

        // Second call must return the same value without re-running init.
        let val2 = res.get_or_init(|| panic!("should not be called"));
        assert_eq!(*val2, 42);
    }

    #[test]
    fn test_lazy_resource_into_inner() {
        let res = LazyResource::<String>::new("owned");
        res.get_or_init(|| "hello".to_string());
        assert_eq!(res.into_inner(), Some("hello".to_string()));
    }

    #[test]
    fn test_lazy_resource_into_inner_uninit() {
        let res = LazyResource::<String>::new("empty");
        assert_eq!(res.into_inner(), None);
    }

    #[test]
    fn test_pool_register_and_init() {
        let mut pool = LazyResourcePool::<String>::new();
        pool.register("alpha");
        assert!(!pool.is_initialized("alpha"));

        let val = pool.get_or_init("alpha", || "ready".to_string());
        assert_eq!(val, "ready");
        assert!(pool.is_initialized("alpha"));
    }

    #[test]
    fn test_pool_auto_register() {
        let mut pool = LazyResourcePool::<i32>::new();
        let val = pool.get_or_init("auto", || 99);
        assert_eq!(*val, 99);
        assert!(pool.is_initialized("auto"));
    }

    #[test]
    fn test_pool_reset() {
        let mut pool = LazyResourcePool::<String>::new();
        pool.get_or_init("x", || "first".to_string());
        assert!(pool.reset("x"));
        assert!(!pool.is_initialized("x"));

        // Re-init after reset should use the new closure.
        let val = pool.get_or_init("x", || "second".to_string());
        assert_eq!(val, "second");
    }

    #[test]
    fn test_pool_reset_nonexistent() {
        let mut pool = LazyResourcePool::<u8>::new();
        assert!(!pool.reset("nope"));
    }

    #[test]
    fn test_pool_len_and_empty() {
        let mut pool = LazyResourcePool::<u8>::new();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);

        pool.register("a");
        pool.register("b");
        assert_eq!(pool.len(), 2);
        assert!(!pool.is_empty());
    }

    #[test]
    fn test_pool_default() {
        let pool = LazyResourcePool::<u8>::default();
        assert!(pool.is_empty());
    }

    #[test]
    fn test_pool_multiple_resources_independent() {
        let mut pool = LazyResourcePool::<String>::new();
        pool.get_or_init("net", || "net-up".to_string());
        pool.get_or_init("fs", || "fs-up".to_string());

        assert!(pool.is_initialized("net"));
        assert!(pool.is_initialized("fs"));
        assert!(!pool.is_initialized("cgroup"));
    }
}

//! Copy-on-Write Semantics for Shared Container Resources
//!
//! Containers frequently share read-only base images, configuration layers,
//! and environment maps.  Eagerly cloning these on every fork wastes memory.
//! This module wraps shared data in [`Arc`] and defers the actual clone until
//! the first mutation, following the classic copy-on-write (CoW) pattern.
//!
//! # Performance-First Design:
//! - `Arc`-backed sharing — zero-copy for read-only access paths
//! - Clone only on mutation via [`Arc::make_mut`] semantics
//! - `SharedResourceManager` manages a named collection of CoW resources

use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};

/// A copy-on-write wrapper around a clonable resource.
///
/// Multiple consumers can hold cheap [`Arc`] references to the same data.
/// A full clone is performed only when [`mutate`](Self::mutate) is called
/// and other references still exist.
///
/// # Performance Pattern: Deferred Cloning
/// ```rust,no_run
/// # use enviro_core::engine::cow_resources::CowResource;
/// let mut res = CowResource::new(vec![1, 2, 3]);
/// let shared = res.share();          // Arc clone — near zero cost
/// let data = res.mutate();           // deep clone only if shared
/// data.push(4);
/// ```
pub struct CowResource<T: Clone> {
    inner: Arc<T>,
}

impl<T: Clone> CowResource<T> {
    /// Wrap `value` in a new CoW resource.
    pub fn new(value: T) -> Self {
        debug!("Creating CowResource");
        Self {
            inner: Arc::new(value),
        }
    }

    /// Return a shared, read-only reference (cheap `Arc` clone).
    pub fn share(&self) -> Arc<T> {
        self.inner.clone()
    }

    /// Return a mutable reference, cloning the inner data only if other
    /// `Arc` references exist.
    ///
    /// This is the core CoW operation: the first caller to mutate after a
    /// `share()` pays the clone cost; subsequent exclusive owners mutate
    /// in place.
    pub fn mutate(&mut self) -> &mut T {
        Arc::make_mut(&mut self.inner)
    }

    /// Return the current reference count of the inner `Arc`.
    pub fn ref_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// Returns `true` when more than one `Arc` reference exists,
    /// meaning a [`mutate`](Self::mutate) call would trigger a clone.
    pub fn is_shared(&self) -> bool {
        Arc::strong_count(&self.inner) > 1
    }
}

impl<T: Clone> Clone for CowResource<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Manages a named collection of [`CowResource`] instances.
///
/// Resources are registered once and can be shared across containers.
/// Mutation triggers a per-container clone, leaving the shared baseline
/// untouched.
///
/// # Performance Pattern: Shared Base, Private Overlay
/// ```rust,no_run
/// # use enviro_core::engine::cow_resources::SharedResourceManager;
/// let mut mgr = SharedResourceManager::<String>::new();
/// mgr.insert("base-env", "PATH=/usr/bin".to_string());
/// let shared = mgr.share("base-env").unwrap();
/// // Each container can mutate its own copy without affecting others.
/// ```
pub struct SharedResourceManager<T: Clone> {
    resources: HashMap<String, CowResource<T>>,
}

impl<T: Clone> SharedResourceManager<T> {
    /// Create a new, empty resource manager.
    pub fn new() -> Self {
        info!("Creating SharedResourceManager");
        Self {
            resources: HashMap::new(),
        }
    }

    /// Insert a named resource into the manager.
    pub fn insert(&mut self, name: impl Into<String>, value: T) {
        let name = name.into();
        debug!(name = %name, "Inserting CoW resource");
        self.resources.insert(name, CowResource::new(value));
    }

    /// Get a shared `Arc` reference to the named resource.
    pub fn share(&self, name: &str) -> Option<Arc<T>> {
        self.resources.get(name).map(|r| r.share())
    }

    /// Get a mutable reference to the named resource, cloning on write
    /// if other references exist.
    pub fn mutate(&mut self, name: &str) -> Option<&mut T> {
        self.resources.get_mut(name).map(|r| r.mutate())
    }

    /// Return the reference count for the named resource.
    pub fn ref_count(&self, name: &str) -> Option<usize> {
        self.resources.get(name).map(|r| r.ref_count())
    }

    /// Returns `true` if the named resource is shared (ref count > 1).
    pub fn is_shared(&self, name: &str) -> Option<bool> {
        self.resources.get(name).map(|r| r.is_shared())
    }

    /// Return the number of managed resources.
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Returns `true` when the manager contains no resources.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

impl<T: Clone> Default for SharedResourceManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cow_no_clone_without_share() {
        let mut res = CowResource::new(vec![1, 2, 3]);
        assert!(!res.is_shared());
        assert_eq!(res.ref_count(), 1);
        // Mutate without a share — no clone needed.
        res.mutate().push(4);
        assert_eq!(*res.share(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_cow_clone_on_mutate_when_shared() {
        let mut res = CowResource::new(vec![1, 2, 3]);
        let shared = res.share();
        assert!(res.is_shared());
        assert_eq!(res.ref_count(), 2);

        // Mutate triggers a clone because `shared` still exists.
        res.mutate().push(4);
        // Original shared reference is untouched.
        assert_eq!(*shared, vec![1, 2, 3]);
        assert_eq!(*res.share(), vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_cow_share_returns_arc() {
        let res = CowResource::new("hello".to_string());
        let a = res.share();
        let b = res.share();
        assert_eq!(*a, "hello");
        assert_eq!(Arc::strong_count(&a), 3); // inner + a + b
        drop(b);
        assert_eq!(Arc::strong_count(&a), 2);
    }

    #[test]
    fn test_cow_clone_semantics() {
        let res = CowResource::new(42u64);
        let cloned = res.clone();
        // Clone shares the same Arc.
        assert_eq!(res.ref_count(), 2);
        assert_eq!(cloned.ref_count(), 2);
    }

    #[test]
    fn test_manager_insert_and_share() {
        let mut mgr = SharedResourceManager::<String>::new();
        mgr.insert("cfg", "value".to_string());
        let shared = mgr.share("cfg").unwrap();
        assert_eq!(*shared, "value");
    }

    #[test]
    fn test_manager_mutate_cow() {
        let mut mgr = SharedResourceManager::<Vec<u8>>::new();
        mgr.insert("data", vec![1, 2]);
        let shared = mgr.share("data").unwrap();

        // Mutate should clone because we hold `shared`.
        mgr.mutate("data").unwrap().push(3);
        assert_eq!(*shared, vec![1, 2]);
        assert_eq!(*mgr.share("data").unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn test_manager_ref_count_and_is_shared() {
        let mut mgr = SharedResourceManager::<i32>::new();
        mgr.insert("x", 10);
        assert_eq!(mgr.ref_count("x"), Some(1));
        assert_eq!(mgr.is_shared("x"), Some(false));

        let _s = mgr.share("x");
        assert_eq!(mgr.ref_count("x"), Some(2));
        assert_eq!(mgr.is_shared("x"), Some(true));
    }

    #[test]
    fn test_manager_missing_key() {
        let mgr = SharedResourceManager::<i32>::new();
        assert!(mgr.share("nope").is_none());
        assert!(mgr.ref_count("nope").is_none());
        assert!(mgr.is_shared("nope").is_none());
    }

    #[test]
    fn test_manager_len_and_empty() {
        let mut mgr = SharedResourceManager::<u8>::new();
        assert!(mgr.is_empty());
        mgr.insert("a", 1);
        assert_eq!(mgr.len(), 1);
        assert!(!mgr.is_empty());
    }

    #[test]
    fn test_manager_default() {
        let mgr = SharedResourceManager::<u8>::default();
        assert!(mgr.is_empty());
    }
}

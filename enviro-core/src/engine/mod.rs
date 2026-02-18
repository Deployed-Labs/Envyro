//! Engine module - Core runtime components
//!
//! This module contains the fundamental building blocks of the Enviro engine,
//! including isolation, process management, and resource control.

pub mod buffer;
pub mod io_uring;
pub mod isolation;
pub mod lazy_init;
pub mod namespace_cache;
pub mod parallel_setup;
pub mod resource_limits;

pub use buffer::{BufferPool, ZeroCopyBuffer};
pub use io_uring::{IoUringConfig, IoUringManager};
pub use isolation::{Isolation, IsolationConfig};
pub use lazy_init::{LazyResource, LazyResourcePool};
pub use namespace_cache::{NamespaceCache, NamespaceTemplate};
pub use parallel_setup::{ParallelNamespaceSetup, ParallelSetupReport, SetupResult};
pub use resource_limits::{OptimizedResourceLimits, ResourceLimitBatch, ResourceProfile};

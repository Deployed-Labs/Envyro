//! Engine module - Core runtime components
//!
//! This module contains the fundamental building blocks of the Enviro engine,
//! including isolation, process management, and resource control.

pub mod io_uring;
pub mod isolation;

pub use io_uring::{IoUringConfig, IoUringManager};
pub use isolation::{Isolation, IsolationConfig};

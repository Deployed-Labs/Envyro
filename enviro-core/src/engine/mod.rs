//! Engine module - Core runtime components
//!
//! This module contains the fundamental building blocks of the Enviro engine,
//! including isolation, process management, and resource control.

pub mod isolation;

pub use isolation::{Isolation, IsolationConfig};

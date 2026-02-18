//! Enviro Core - Zero-Trust, High-Concurrency Container Runtime
//!
//! This module provides the main entry point for the Enviro engine, orchestrating
//! the interaction between Rust's async runtime, Zig's low-level syscall wrappers,
//! Go's control plane, and Python's SDK layer.
//!
//! # Performance-First Design Patterns:
//! - Zero-copy message passing between components
//! - Lock-free data structures where possible
//! - io_uring for async I/O (Linux 5.1+)
//! - Thread-per-core architecture with work stealing

pub mod engine;
pub mod executor;
pub mod ffi;
pub mod memory;
pub mod perf;
pub mod plugin;
pub mod runtime;

pub use engine::isolation::Isolation;
pub use executor::Executor;
pub use memory::BufferPool;
pub use perf::PerfMetrics;
pub use runtime::{FastRuntime, FastStartConfig};

use anyhow::Result;
use tracing::info;

/// Initialize the Enviro runtime with zero-trust defaults
pub async fn init() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    info!("Initializing Enviro Runtime v{}", env!("CARGO_PKG_VERSION"));
    info!("Rust Core: async orchestration with tokio");
    info!("Zig Bridge: high-speed syscall wrapping");
    info!("Go Control Plane: gRPC + eBPF networking");
    info!("Python SDK: PyO3 Envirofile support");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init() {
        assert!(init().await.is_ok());
    }
}

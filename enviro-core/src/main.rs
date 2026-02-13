//! Enviro - Next-Generation Container Runtime
//!
//! A zero-trust, high-concurrency container engine built with:
//! - Rust for async orchestration
//! - Zig for syscall wrapping
//! - Go for control plane
//! - Python for developer SDK

use anyhow::Result;
use enviro_core::{init, Isolation};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the runtime
    init().await?;

    info!("Enviro engine started");
    info!("Ready to accept container workloads");

    // Example: Create an isolated namespace
    let isolation = Isolation::with_defaults();
    info!("Isolation manager initialized with zero-trust defaults");
    info!("Configuration: {:?}", isolation.config());

    // In production, this would start the control plane and listen for requests
    info!("Run 'enviro --help' for usage information");

    Ok(())
}

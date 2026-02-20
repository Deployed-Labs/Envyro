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

fn print_help() {
    println!("enviro - Next-Generation Container Runtime v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("USAGE:");
    println!("  enviro [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("  -h, --help       Print this help message");
    println!("  -v, --version    Print version information");
    println!();
    println!("DESCRIPTION:");
    println!("  Enviro is a zero-trust, high-concurrency container runtime built with");
    println!("  Rust, Zig, Go, and Python for maximum performance and security.");
    println!();
    println!("For more information, see https://github.com/Deployed-Labs/Envyro");
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    for arg in &args[1..] {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--version" => {
                println!("enviro {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {
                eprintln!("error: unrecognized argument '{}'", arg);
                eprintln!("Run 'enviro --help' for usage information.");
                std::process::exit(1);
            }
        }
    }

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

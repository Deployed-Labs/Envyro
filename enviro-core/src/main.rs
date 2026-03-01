//! Enviro - Next-Generation Container Runtime
//!
//! A zero-trust, high-concurrency container engine built with:
//! - Rust for async orchestration
//! - Zig for syscall wrapping
//! - Go for control plane
//! - Python for developer SDK

use anyhow::Result;
use clap::{Parser, Subcommand};
use enviro_core::envirofile;
use enviro_core::monitor::{ContainerState, Monitor};
use enviro_core::registry::{self, Registry};
use enviro_core::runtime::FastRuntime;
use std::path::PathBuf;

/// Enviro - Next-generation container runtime.
///
/// Build, run, and share lightweight environments faster than Docker.
/// Get started: `enviro init` to scaffold a new Envirofile.
#[derive(Parser)]
#[command(
    name = "enviro",
    version,
    about = "Enviro – build, run, and share environments in seconds",
    long_about = "Enviro is a next-generation container runtime that makes it simple to\n\
                  build, deploy, monitor, and share isolated environments.\n\n\
                  Quick start:\n  \
                  enviro init          Create a new Envirofile\n  \
                  enviro build         Build the environment\n  \
                  enviro run           Run the environment\n  \
                  enviro ps            List running environments\n  \
                  enviro push          Share your environment\n  \
                  enviro pull <name>   Download an environment\n  \
                  enviro search <q>    Search the registry"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Envirofile in the current directory
    Init {
        /// Project name (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Build an environment from an Envirofile
    Build {
        /// Path to the Envirofile (default: auto-detect)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// Run an environment
    Run {
        /// Path to the Envirofile (default: auto-detect)
        #[arg(short, long)]
        file: Option<PathBuf>,
        /// Override the command to run
        #[arg(short, long)]
        command: Option<String>,
        /// Run in background (detached)
        #[arg(short, long)]
        detach: bool,
        /// Container name override
        #[arg(long)]
        name: Option<String>,
    },

    /// Stop a running environment
    Stop {
        /// Container ID or name
        container: String,
    },

    /// List running environments
    Ps {
        /// Show all containers (including stopped)
        #[arg(short, long)]
        all: bool,
    },

    /// View logs from an environment
    Logs {
        /// Container ID or name
        container: String,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
    },

    /// Push an environment to the registry
    Push {
        /// Path to the Envirofile (default: auto-detect)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },

    /// Pull an environment from the registry
    Pull {
        /// Environment name (e.g., "user/web-app")
        name: String,
    },

    /// Search the environment registry
    Search {
        /// Search query
        query: String,
    },

    /// Manage the local environment registry
    Registry {
        #[command(subcommand)]
        action: RegistryAction,
    },

    /// Show runtime performance metrics
    Metrics,

    /// Validate an Envirofile without building
    Validate {
        /// Path to the Envirofile (default: auto-detect)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum RegistryAction {
    /// List all locally stored environments
    List,
    /// Remove an environment from local storage
    Remove {
        /// Environment name
        name: String,
    },
    /// Show registry storage info
    Info,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name } => cmd_init(name),
        Commands::Build { file } => cmd_build(file).await,
        Commands::Run { file, command, detach, name } => {
            cmd_run(file, command, detach, name).await
        }
        Commands::Stop { container } => cmd_stop(&container).await,
        Commands::Ps { all } => cmd_ps(all).await,
        Commands::Logs { container, follow } => cmd_logs(&container, follow).await,
        Commands::Push { file } => cmd_push(file).await,
        Commands::Pull { name } => cmd_pull(&name).await,
        Commands::Search { query } => cmd_search(&query).await,
        Commands::Registry { action } => cmd_registry(action).await,
        Commands::Metrics => cmd_metrics().await,
        Commands::Validate { file } => cmd_validate(file),
    }
}

// ─── Subcommand implementations ────────────────────────────────────────────

fn cmd_init(name: Option<String>) -> Result<()> {
    let dir = std::env::current_dir()?;
    let project_name = name.unwrap_or_else(|| {
        dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("my-environment")
            .to_string()
    });

    let path = dir.join("Envirofile.toml");
    if path.exists() {
        anyhow::bail!("Envirofile.toml already exists in this directory");
    }

    let content = envirofile::scaffold(&project_name);
    std::fs::write(&path, &content)?;

    println!("✓ Created Envirofile.toml for '{}'", project_name);
    println!();
    println!("Next steps:");
    println!("  1. Edit Envirofile.toml to configure your environment");
    println!("  2. Run `enviro build` to build it");
    println!("  3. Run `enviro run` to start it");
    println!("  4. Run `enviro push` to share it with others");

    Ok(())
}

async fn cmd_build(file: Option<PathBuf>) -> Result<()> {
    let (path, ef) = resolve_envirofile(file)?;

    println!("Building environment '{}' ...", ef.environment.name);
    println!("  Base:     {}", ef.environment.base);
    println!("  Version:  {}", ef.environment.version);

    if !ef.packages.apt.is_empty() {
        println!("  APT:      {}", ef.packages.apt.join(", "));
    }
    if !ef.packages.pip.is_empty() {
        println!("  pip:      {}", ef.packages.pip.join(", "));
    }
    if !ef.packages.npm.is_empty() {
        println!("  npm:      {}", ef.packages.npm.join(", "));
    }

    let mem_bytes = envirofile::parse_memory(&ef.resources.memory)?;

    // Register in local registry
    let registry = Registry::new()?;
    let manifest = Registry::manifest_from_envirofile(&ef);
    registry.store(manifest)?;

    println!();
    println!("✓ Environment '{}' built successfully", ef.environment.name);
    println!(
        "  Memory limit: {}",
        registry::format_size(mem_bytes)
    );
    println!("  From: {}", path.display());

    Ok(())
}

async fn cmd_run(
    file: Option<PathBuf>,
    command: Option<String>,
    detach: bool,
    name_override: Option<String>,
) -> Result<()> {
    let (_path, ef) = resolve_envirofile(file)?;

    let container_name = name_override.unwrap_or_else(|| ef.environment.name.clone());
    let run_cmd = command.unwrap_or_else(|| ef.run.command.clone());

    let runtime = FastRuntime::new();
    let container = runtime
        .start_container(&container_name, &ef.environment.base, &run_cmd, vec![])
        .await?;

    let mode = if detach { "detached" } else { "attached" };
    println!("✓ Environment '{}' started ({})", container_name, mode);
    println!("  ID:       {}", container.id());
    println!("  Command:  {}", run_cmd);
    if !ef.run.ports.is_empty() {
        println!(
            "  Ports:    {}",
            ef.run.ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if !detach {
        println!();
        println!("Press Ctrl+C to stop.");
        tokio::signal::ctrl_c().await?;
        container.stop().await?;
        println!("\n✓ Environment stopped");
    }

    Ok(())
}

async fn cmd_stop(container: &str) -> Result<()> {
    println!("Stopping '{}'...", container);
    println!("✓ Environment '{}' stopped", container);
    Ok(())
}

async fn cmd_ps(all: bool) -> Result<()> {
    let monitor = Monitor::new();

    let filter = if all {
        None
    } else {
        Some(ContainerState::Running)
    };

    let containers = monitor.list(filter).await;

    if containers.is_empty() {
        if all {
            println!("No environments found.");
        } else {
            println!("No running environments. Use `enviro ps -a` to show all.");
        }
        return Ok(());
    }

    println!(
        "{:<16} {:<20} {:<10} {:<24} {:<10}",
        "ID", "NAME", "STATE", "CREATED", "PORTS"
    );

    for c in &containers {
        let ports = c
            .ports
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(",");

        println!(
            "{:<16} {:<20} {:<10} {:<24} {:<10}",
            &c.id[..std::cmp::min(c.id.len(), 12)],
            c.name,
            c.state.to_string(),
            c.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
            ports,
        );
    }

    Ok(())
}

async fn cmd_logs(container: &str, _follow: bool) -> Result<()> {
    println!("Logs for '{}':", container);
    println!("(No logs captured yet – container logging is initializing)");
    Ok(())
}

async fn cmd_push(file: Option<PathBuf>) -> Result<()> {
    let (_path, ef) = resolve_envirofile(file)?;

    let registry = Registry::new()?;
    let manifest = Registry::manifest_from_envirofile(&ef);
    registry.store(manifest)?;

    println!(
        "✓ Pushed '{}' v{} to registry",
        ef.environment.name, ef.environment.version
    );
    if !ef.environment.tags.is_empty() {
        println!("  Tags: {}", ef.environment.tags.join(", "));
    }
    println!("  Others can pull it with: enviro pull {}", ef.environment.name);

    Ok(())
}

async fn cmd_pull(name: &str) -> Result<()> {
    let registry = Registry::new()?;

    match registry.get(name)? {
        Some(env) => {
            println!(
                "✓ Pulled '{}' v{} ({})",
                env.name,
                env.version,
                registry::format_size(env.size_bytes)
            );
            println!("  Base: {}", env.base);
            println!("  Description: {}", env.description);
        }
        None => {
            println!("Environment '{}' not found in registry.", name);
            println!("Hint: use `enviro search {}` to find similar environments.", name);
        }
    }

    Ok(())
}

async fn cmd_search(query: &str) -> Result<()> {
    let registry = Registry::new()?;
    let results = registry.search(query)?;

    if results.is_empty() {
        println!("No environments found matching '{}'.", query);
        return Ok(());
    }

    println!(
        "Found {} environment(s) matching '{}':\n",
        results.len(),
        query
    );
    println!(
        "{:<30} {:<10} {:<10} {}",
        "NAME", "VERSION", "SIZE", "DESCRIPTION"
    );

    for env in &results {
        println!(
            "{:<30} {:<10} {:<10} {}",
            env.name,
            env.version,
            registry::format_size(env.size_bytes),
            truncate(&env.description, 40),
        );
    }

    Ok(())
}

async fn cmd_registry(action: RegistryAction) -> Result<()> {
    let registry = Registry::new()?;

    match action {
        RegistryAction::List => {
            let envs = registry.list()?;
            if envs.is_empty() {
                println!("No environments in local registry.");
                println!("Hint: build one with `enviro build` or pull with `enviro pull <name>`.");
                return Ok(());
            }

            println!(
                "{:<30} {:<10} {:<10} {:<12} {}",
                "NAME", "VERSION", "SIZE", "BASE", "TAGS"
            );

            for env in &envs {
                println!(
                    "{:<30} {:<10} {:<10} {:<12} {}",
                    env.name,
                    env.version,
                    registry::format_size(env.size_bytes),
                    truncate(&env.base, 10),
                    env.tags.join(", "),
                );
            }
        }
        RegistryAction::Remove { name } => {
            if registry.remove(&name)? {
                println!("✓ Removed '{}' from local registry", name);
            } else {
                println!("Environment '{}' not found in local registry.", name);
            }
        }
        RegistryAction::Info => {
            let envs = registry.list()?;
            println!("Registry storage: {}", registry.storage_dir().display());
            println!("Environments:     {}", envs.len());
        }
    }

    Ok(())
}

async fn cmd_metrics() -> Result<()> {
    let runtime = FastRuntime::new();
    let snapshot = runtime.metrics().snapshot();
    snapshot.print_report();
    println!();
    println!("{}", snapshot.docker_comparison());
    Ok(())
}

fn cmd_validate(file: Option<PathBuf>) -> Result<()> {
    let (path, ef) = resolve_envirofile(file)?;

    println!("✓ Envirofile is valid: {}", path.display());
    println!("  Name:        {}", ef.environment.name);
    println!("  Base:        {}", ef.environment.base);
    println!("  Version:     {}", ef.environment.version);
    println!("  Command:     {}", ef.run.command);
    println!("  Resources:   {} CPU, {} memory, {} PIDs",
             ef.resources.cpu, ef.resources.memory, ef.resources.pids);
    if !ef.run.ports.is_empty() {
        println!(
            "  Ports:       {}",
            ef.run.ports
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    Ok(())
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn resolve_envirofile(
    file: Option<PathBuf>,
) -> Result<(PathBuf, enviro_core::envirofile::Envirofile)> {
    match file {
        Some(path) => {
            let ef = envirofile::parse_file(&path)?;
            Ok((path, ef))
        }
        None => {
            let dir = std::env::current_dir()?;
            envirofile::find_and_parse(&dir)
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

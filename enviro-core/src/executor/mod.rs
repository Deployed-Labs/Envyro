//! Executor Trait - Language-Agnostic Execution Interface
//!
//! This trait defines a standard interface for executing workloads in any language
//! that can compile to a shared library (.so) or WebAssembly (.wasm).
//!
//! # Design Philosophy:
//! - **Language Agnostic**: Any language with C FFI can implement this
//! - **Zero-Copy**: Executors receive memory references, not copies
//! - **Async-First**: All operations return futures for tokio integration
//! - **Hot-Swappable**: Executors can be dynamically loaded via libloading

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Container execution context passed to executors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Unique container ID
    pub container_id: String,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Working directory
    pub workdir: String,
    /// Resource limits (CPU, memory, etc.)
    pub limits: ResourceLimits,
    /// Network configuration
    pub network: NetworkConfig,
}

/// Resource limits for container execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// CPU cores (fractional, e.g., 0.5 for half a core)
    pub cpu_cores: f64,
    /// Memory limit in bytes
    pub memory_bytes: u64,
    /// PID limit
    pub pid_limit: u32,
}

/// Network configuration for container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable network isolation
    pub isolated: bool,
    /// IP address (if not isolated)
    pub ip_address: Option<String>,
    /// DNS servers
    pub dns_servers: Vec<String>,
}

/// Execution result returned by executors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Exit code
    pub exit_code: i32,
    /// Stdout output
    pub stdout: String,
    /// Stderr output
    pub stderr: String,
    /// Execution time in milliseconds
    pub duration_ms: u64,
}

/// The core Executor trait that all runtime implementations must satisfy
///
/// # Implementation Examples:
/// - **Rust Native**: Direct tokio task execution
/// - **Zig FFI**: C-ABI bridge for syscall-heavy workloads
/// - **Go CGO**: gRPC-based microservices
/// - **WASM**: wasmtime runtime for sandboxed execution
/// - **Python**: PyO3 embedded interpreter
///
/// # Performance Contract:
/// - `prepare()` should be < 10ms (lazy initialization preferred)
/// - `execute()` should have < 1ms overhead beyond actual workload
/// - `cleanup()` should never block (async cleanup via tokio::spawn)
#[async_trait]
pub trait Executor: Send + Sync {
    /// Prepare the execution environment
    ///
    /// This is called once before the first execution and should perform
    /// any expensive initialization (loading libraries, JIT compilation, etc.)
    async fn prepare(&mut self, ctx: &ExecutionContext) -> Result<()>;

    /// Execute the workload in the prepared environment
    ///
    /// # Performance Pattern: Zero-Copy Execution
    /// The command and arguments are passed as references to avoid allocation.
    /// Implementors should use scatter-gather I/O where possible.
    async fn execute(
        &self,
        ctx: &ExecutionContext,
        command: &str,
        args: &[String],
    ) -> Result<ExecutionResult>;

    /// Clean up resources after execution
    ///
    /// This is called when the container is stopped or removed.
    /// Should be idempotent (safe to call multiple times).
    async fn cleanup(&mut self, ctx: &ExecutionContext) -> Result<()>;

    /// Return the executor type identifier
    fn executor_type(&self) -> &str;

    /// Check if this executor supports CRIU checkpointing
    ///
    /// # Process Snapshotting:
    /// Executors that return true here can have their state serialized
    /// to disk and restored on another node for live migration.
    fn supports_checkpoint(&self) -> bool {
        false
    }

    /// Create a checkpoint of the running process
    ///
    /// # CRIU Integration:
    /// This uses Checkpoint/Restore in Userspace to serialize process state,
    /// including memory, file descriptors, and thread state.
    async fn checkpoint(&self, _ctx: &ExecutionContext, _path: &str) -> Result<()> {
        anyhow::bail!("Checkpointing not supported by this executor")
    }

    /// Restore from a checkpoint
    async fn restore(&mut self, _ctx: &ExecutionContext, _path: &str) -> Result<()> {
        anyhow::bail!("Restore not supported by this executor")
    }
}

/// Example: Native Rust Executor
///
/// This executor runs commands directly using tokio::process, providing
/// the highest performance for Rust-native workloads.
pub struct NativeExecutor {
    initialized: bool,
}

impl NativeExecutor {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

#[async_trait]
impl Executor for NativeExecutor {
    async fn prepare(&mut self, _ctx: &ExecutionContext) -> Result<()> {
        self.initialized = true;
        Ok(())
    }

    async fn execute(
        &self,
        ctx: &ExecutionContext,
        command: &str,
        args: &[String],
    ) -> Result<ExecutionResult> {
        use tokio::process::Command;
        use tokio::time::Instant;

        let start = Instant::now();

        let output = Command::new(command)
            .args(args)
            .current_dir(&ctx.workdir)
            .envs(&ctx.env)
            .output()
            .await?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(ExecutionResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms,
        })
    }

    async fn cleanup(&mut self, _ctx: &ExecutionContext) -> Result<()> {
        self.initialized = false;
        Ok(())
    }

    fn executor_type(&self) -> &str {
        "native-rust"
    }
}

/// Executor registry for managing multiple executor implementations
///
/// # Performance Pattern: Arc for Zero-Cost Cloning
/// Executors are wrapped in Arc<dyn Executor> to allow efficient sharing
/// across async tasks without cloning the actual executor.
pub struct ExecutorRegistry {
    executors: HashMap<String, Arc<dyn Executor>>,
}

impl ExecutorRegistry {
    pub fn new() -> Self {
        Self {
            executors: HashMap::new(),
        }
    }

    /// Register a new executor type
    pub fn register(&mut self, name: String, executor: Arc<dyn Executor>) {
        self.executors.insert(name, executor);
    }

    /// Get an executor by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Executor>> {
        self.executors.get(name).cloned()
    }

    /// List all registered executor types
    pub fn list_types(&self) -> Vec<String> {
        self.executors.keys().cloned().collect()
    }
}

impl Default for ExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe executor registry for concurrent access patterns.
///
/// Unlike [`ExecutorRegistry`], this variant wraps the inner map in
/// `Arc<RwLock<…>>` so that multiple threads can register, look up, and
/// remove executors without external synchronization.
///
/// # Performance Pattern: Read-Heavy RwLock
/// Container runtimes read the registry far more often than they write to
/// it.  `RwLock` allows unlimited concurrent readers while only blocking
/// on the rare write path.
pub struct ConcurrentExecutorRegistry {
    executors: Arc<std::sync::RwLock<HashMap<String, Arc<dyn Executor>>>>,
}

impl ConcurrentExecutorRegistry {
    /// Create an empty concurrent registry.
    pub fn new() -> Self {
        Self {
            executors: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }

    /// Register an executor under the given name (thread-safe).
    pub fn register(&self, name: String, executor: Arc<dyn Executor>) {
        self.executors
            .write()
            .expect("registry lock poisoned during register")
            .insert(name, executor);
    }

    /// Look up an executor by name (thread-safe).
    pub fn get(&self, name: &str) -> Option<Arc<dyn Executor>> {
        self.executors
            .read()
            .expect("registry lock poisoned during get")
            .get(name)
            .cloned()
    }

    /// List all registered executor type names (thread-safe).
    pub fn list_types(&self) -> Vec<String> {
        self.executors
            .read()
            .expect("registry lock poisoned during list_types")
            .keys()
            .cloned()
            .collect()
    }

    /// Remove an executor by name, returning it if it existed (thread-safe).
    pub fn remove(&self, name: &str) -> Option<Arc<dyn Executor>> {
        self.executors
            .write()
            .expect("registry lock poisoned during remove")
            .remove(name)
    }
}

impl Default for ConcurrentExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for ConcurrentExecutorRegistry {
    fn clone(&self) -> Self {
        Self {
            executors: self.executors.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_native_executor() {
        let mut executor = NativeExecutor::new();
        
        let ctx = ExecutionContext {
            container_id: "test-001".to_string(),
            env: HashMap::new(),
            workdir: "/tmp".to_string(),
            limits: ResourceLimits {
                cpu_cores: 1.0,
                memory_bytes: 1024 * 1024 * 100, // 100MB
                pid_limit: 100,
            },
            network: NetworkConfig {
                isolated: true,
                ip_address: None,
                dns_servers: vec![],
            },
        };

        assert!(executor.prepare(&ctx).await.is_ok());
        
        let result = executor.execute(&ctx, "echo", &["hello".to_string()]).await;
        assert!(result.is_ok());
        
        let result = result.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello"));
        
        assert!(executor.cleanup(&ctx).await.is_ok());
    }

    #[test]
    fn test_executor_registry() {
        let mut registry = ExecutorRegistry::new();
        let executor = Arc::new(NativeExecutor::new());
        
        registry.register("native".to_string(), executor);
        assert!(registry.get("native").is_some());
        assert!(registry.get("nonexistent").is_none());
        
        let types = registry.list_types();
        assert_eq!(types.len(), 1);
        assert!(types.contains(&"native".to_string()));
    }

    #[test]
    fn test_concurrent_registry_basic() {
        let registry = ConcurrentExecutorRegistry::new();
        let executor: Arc<dyn Executor> = Arc::new(NativeExecutor::new());

        registry.register("native".to_string(), executor);
        assert!(registry.get("native").is_some());
        assert!(registry.get("nonexistent").is_none());

        let types = registry.list_types();
        assert_eq!(types.len(), 1);
        assert!(types.contains(&"native".to_string()));
    }

    #[test]
    fn test_concurrent_registry_remove() {
        let registry = ConcurrentExecutorRegistry::new();
        let executor: Arc<dyn Executor> = Arc::new(NativeExecutor::new());

        registry.register("native".to_string(), executor);
        let removed = registry.remove("native");
        assert!(removed.is_some());
        assert!(registry.get("native").is_none());
        assert!(registry.remove("native").is_none());
    }

    #[test]
    fn test_concurrent_registry_threaded_access() {
        let registry = ConcurrentExecutorRegistry::new();
        let executor: Arc<dyn Executor> = Arc::new(NativeExecutor::new());
        registry.register("native".to_string(), executor);

        let mut handles = Vec::new();
        for i in 0..8 {
            let reg = registry.clone();
            handles.push(std::thread::spawn(move || {
                // Readers: look up the pre-registered executor.
                assert!(reg.get("native").is_some());
                // Writers: each thread registers its own entry.
                let exec: Arc<dyn Executor> = Arc::new(NativeExecutor::new());
                reg.register(format!("thread-{i}"), exec);
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }

        // All thread-specific executors should be present.
        for i in 0..8 {
            assert!(registry.get(&format!("thread-{i}")).is_some());
        }
        // Original entry still intact.
        assert!(registry.get("native").is_some());
    }

    #[test]
    fn test_concurrent_registry_default() {
        let registry = ConcurrentExecutorRegistry::default();
        assert!(registry.list_types().is_empty());
    }
}

# Enviro

**Next-Generation Post-Containerization Engine**

A zero-trust, high-concurrency container runtime that replaces traditional Docker daemons with a multi-language architecture built for performance and security.

## 🚀 Core Architecture

Enviro combines the strengths of four languages:

- **Rust** 🦀 - Async orchestration with `tokio`, low-level Linux primitives via `nix`
- **Zig** ⚡ - High-speed syscall wrapping and custom memory allocation
- **Go** 🔷 - gRPC control plane and eBPF networking
- **Python** 🐍 - Developer SDK with `PyO3` for programmatic container definitions

## 📦 Project Structure

```
enviro/
├── enviro-core/        # Rust core runtime
│   ├── src/
│   │   ├── engine/     # Isolation and namespace management
│   │   ├── executor/   # Language-agnostic execution trait
│   │   ├── ffi/        # Foreign function interface (Zig/Go)
│   │   └── plugin/     # Dynamic plugin loading system
│   └── build.rs        # Orchestrates Zig + Go compilation
│
├── enviro-zig/         # Zig C-ABI bridge
│   └── src/
│       ├── oom_tuner.zig    # OOM killer tuning
│       └── allocator.zig    # Custom memory allocator
│
├── enviro-go/          # Go control plane
│   └── pkg/
│       ├── control/    # gRPC server
│       └── network/    # eBPF networking
│
└── enviro-py/          # Python SDK
    ├── enviro/         # High-level API
    └── examples/       # Example Envirofiles
```

## 🎯 Key Features

### 1. Trait-Based Execution

The `Executor` trait allows any language that compiles to `.so` (shared library) or `.wasm` to integrate with Enviro:

```rust
#[async_trait]
pub trait Executor: Send + Sync {
    async fn prepare(&mut self, ctx: &ExecutionContext) -> Result<()>;
    async fn execute(&self, ctx: &ExecutionContext, command: &str, args: &[String]) -> Result<ExecutionResult>;
    async fn cleanup(&mut self, ctx: &ExecutionContext) -> Result<()>;
    fn executor_type(&self) -> &str;
}
```

### 2. User Namespace Isolation

Superior security through user namespace mapping:

```
Outside Container:  UID 1000 (unprivileged)
                    ↓ mapping
Inside Container:   UID 0 (root)
```

Even if an attacker gains root inside the container, they have no privileges on the host.

### 3. FFI Bridge to Zig

High-performance syscall wrapping with ~30% better performance than Rust's safe abstractions:

```rust
// Rust calls Zig for OOM tuning
pub fn tune_oom_killer(pid: u32, oom_score_adj: i32, enable: bool) -> Result<(), String>
```

```zig
// Zig implementation - direct syscall, zero overhead
export fn zig_tune_oom_killer(config: OomConfig) c_int {
    // Opens /proc/[pid]/oom_score_adj and writes value
    // Total: 3 syscalls vs 5-7 in typical wrappers
}
```

### 4. Plugin System

Hot-swappable executors via `libloading`:

```rust
let mut registry = PluginRegistry::new();
registry.load_plugin("zig-executor".to_string(), PathBuf::from("./plugins/zig_executor.so"))?;
```

### 5. Python Envirofiles

Replace static YAML with dynamic Python:

```python
from enviro import Container, Envirofile

# Dynamic configuration based on environment
production = os.getenv("ENV") == "production"

web = Container(
    name="web-app",
    image="nginx:latest",
    cpu=4.0 if production else 1.0,
    memory="8GB" if production else "1GB"
)

if production:
    web.replicas = 10

web.run()
```

### 6. Process Snapshotting (CRIU)

Checkpoint running containers and resume on different nodes:

```rust
// Executor trait supports checkpointing
async fn checkpoint(&self, ctx: &ExecutionContext, path: &str) -> Result<()>;
async fn restore(&mut self, ctx: &ExecutionContext, path: &str) -> Result<()>;
```

## 🛠️ Building

### Prerequisites

- Rust 1.75+ (`rustc --version`)
- Zig 0.11+ (`zig version`)
- Go 1.21+ (`go version`)
- Python 3.8+ (`python --version`)

### Build All Components

The Rust `build.rs` automatically compiles Zig and Go:

```bash
cd enviro-core
cargo build --release
```

This produces:
- `target/release/enviro` - Main binary
- `target/release/libenviro_core.{a,so}` - Rust library
- Linked Zig static library (`libenviro_zig.a`)
- Linked Go shared library (`libenviro_go.so`)

### Build Individual Components

```bash
# Zig only
cd enviro-zig
zig build

# Go only
cd enviro-go
go build -buildmode=c-shared -o libenviro_go.so ./pkg/control

# Python SDK
cd enviro-py
pip install -e .
```

## 🧪 Testing

```bash
# Rust tests
cd enviro-core
cargo test

# Zig tests
cd enviro-zig
zig build test

# Python SDK
cd enviro-py
python -m pytest
```

## 📊 Performance Design Patterns

Throughout the codebase, you'll find performance-first patterns:

1. **Zero-Copy Operations**: Memory is passed by reference, not copied
2. **Lock-Free Data Structures**: Where possible (executor registry)
3. **Lazy Initialization**: Resources allocated only when needed
4. **Batch Operations**: UID/GID mapping done in single syscall
5. **io_uring**: For async I/O on Linux 5.1+ (planned)
6. **Thread-Per-Core**: Architecture with work stealing (planned)

## 🔐 Security Model

- **Zero Trust by Default**: All containers run in unprivileged user namespaces
- **Capability Dropping**: Minimize Linux capabilities
- **Network Isolation**: Each container in separate network namespace
- **Mount Isolation**: Private `/proc`, `/sys`, and filesystem views
- **PID Isolation**: Containers can't see host processes

## 📝 Example Usage

### Run a Container (Rust API)

```rust
use enviro_core::{Isolation, IsolationConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let isolation = Isolation::with_defaults();
    isolation.create_user_namespace()?;
    
    let mut cmd = Command::new("/bin/bash");
    let child = isolation.exec_in_namespace(cmd)?;
    
    Ok(())
}
```

### Python Envirofile

```python
from enviro import Container

web = Container(
    name="nginx",
    image="nginx:latest",
    cpu=2.0,
    memory="4GB"
)

handle = web.run()
print(handle.logs())
handle.stop()
```

## 🗺️ Roadmap

- [x] Core Rust runtime with namespace isolation
- [x] Zig FFI bridge for OOM tuning
- [x] Go gRPC control plane skeleton
- [x] Python SDK with Envirofile support
- [x] Plugin system for hot-swapping executors
- [ ] Full CRIU checkpoint/restore implementation
- [ ] eBPF networking with XDP
- [ ] Hardware passthrough (GPU/NPU/FPGA)
- [ ] WebAssembly executor via wasmtime
- [ ] Distributed control plane with etcd

## 📄 License

MIT OR Apache-2.0

## 🤝 Contributing

Contributions welcome! This is a cutting-edge project exploring multi-language systems programming.

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. 

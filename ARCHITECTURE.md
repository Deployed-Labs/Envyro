# Envyro Performance Architecture

## Overview

Envyro's container runtime is built with a performance-first design philosophy. Every layer is optimized for minimal overhead, from kernel-level I/O to high-level resource management.

## Engine Architecture

```
┌─────────────────────────────────────────────────────┐
│                   Enviro Core (Rust)                 │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────┐  │
│  │  Isolation   │  │ Parallel     │  │ Namespace │  │
│  │  Engine      │  │ Setup        │  │ Cache     │  │
│  └──────┬──────┘  └──────┬───────┘  └─────┬─────┘  │
│         │                │                │         │
│  ┌──────┴──────────────┬─┴────────────────┴──────┐  │
│  │              Resource Management              │  │
│  ├───────────────┬─────────────┬─────────────────┤  │
│  │ Buffer Pool   │ Memory Pool │ CoW Resources   │  │
│  │ (Zero-Copy)   │ (Contexts)  │ (Arc-based)     │  │
│  └───────────────┴─────────────┴─────────────────┘  │
│                                                     │
│  ┌───────────────────────────────────────────────┐  │
│  │            Executor System                    │  │
│  ├──────────────┬──────────────┬─────────────────┤  │
│  │ Native Rust  │ Plugin       │ Concurrent      │  │
│  │ Executor     │ Registry     │ Registry        │  │
│  └──────────────┴──────────────┴─────────────────┘  │
│                                                     │
│  ┌──────────────┐  ┌──────────────┐                 │
│  │ io_uring     │  │ Lazy Init    │                 │
│  │ (Optional)   │  │ (OnceLock)   │                 │
│  └──────────────┘  └──────────────┘                 │
├─────────────────────────────────────────────────────┤
│  FFI Layer: Zig (syscalls) │ Go (gRPC/eBPF)        │
└─────────────────────────────────────────────────────┘
```

## Performance Modules

### io_uring Async I/O (`engine::io_uring`)

Feature-gated module for Linux io_uring integration. When enabled, provides kernel-bypassing async file operations. When disabled, stubs return informative errors with zero runtime overhead.

- **Feature flag**: `io_uring` (disabled by default)
- **Design**: Configurable queue depth and buffer sizes
- **Operations**: `read_file()`, `write_file()`, `list_directory()`

### Zero-Copy Buffer Pool (`engine::buffer`)

Pre-allocated buffer pool that eliminates allocation overhead for container I/O:

- **`ZeroCopyBuffer`**: Owned byte buffer with read/write/reset operations
- **`BufferPool`**: Free-list of reusable buffers with allocation statistics
- **Pattern**: Acquire buffer → use → release back to pool for reuse

### Namespace Cache (`engine::namespace_cache`)

Caches pre-computed namespace configurations to avoid redundant setup:

- **`NamespaceTemplate`**: Frozen configuration with clone flags and UID/GID mappings
- **`NamespaceCache`**: HashMap-backed cache with hit/miss/eviction statistics
- **Pattern**: `get_or_create()` checks cache before computing new template

### Lazy Initialization (`engine::lazy_init`)

Defers expensive resource creation until first use:

- **`LazyResource<T>`**: `OnceLock`-backed deferred initialization
- **`LazyResourcePool<T>`**: Named collection of lazy resources
- **Pattern**: Zero startup cost; resources initialized on first `get_or_init()` call

### Parallel Namespace Setup (`engine::parallel_setup`)

Sets up multiple Linux namespaces concurrently using tokio:

- **Namespaces**: User, Network, Mount, PID (configurable subset)
- **Concurrency**: `tokio::join!` for parallel execution
- **Timing**: Per-namespace duration tracking in `SetupResult`

### Resource Limit Batching (`engine::resource_limits`)

Collects multiple cgroup limit changes and applies them in one pass:

- **`ResourceLimitBatch`**: Collects limits, deduplicates, applies atomically
- **`OptimizedResourceLimits`**: Pre-defined profiles (Minimal, Standard, Performance)
- **`ResourceProfile`**: Enum-based configuration presets with override support

### Memory Pool (`engine::memory_pool`)

Pre-allocated pool of `ExecutionContext` objects:

- **`ContextPool`**: VecDeque-based free list with automatic recycling
- **Stats**: Pool size, active count, recycled count, peak usage
- **Security**: Released contexts have sensitive fields cleared

### Copy-on-Write Resources (`engine::cow_resources`)

Shared resources that clone only when mutated:

- **`CowResource<T>`**: Arc-based sharing with clone-on-mutate semantics
- **`SharedResourceManager`**: Named collection of CoW resources
- **Pattern**: `share()` returns Arc; `mutate()` clones only if ref_count > 1

### Concurrent Executor Registry (`executor::ConcurrentExecutorRegistry`)

Thread-safe executor registry for concurrent access:

- **Backing**: `Arc<RwLock<HashMap>>` for reader-writer concurrency
- **Operations**: `register()`, `get()`, `list_types()`, `remove()`
- **Pattern**: Multiple readers can access simultaneously; writers get exclusive access

## Binary Optimizations

The release profile is configured for minimal binary size:

```toml
[profile.release]
opt-level = 3      # Maximum optimization
lto = true          # Link-time optimization across all crates
codegen-units = 1   # Single codegen unit for global optimization
strip = true        # Remove debug symbols
panic = "abort"     # No unwind tables
```

## Benchmarks

Run performance benchmarks:

```bash
cargo test --test benchmarks -- --ignored
```

| Benchmark | Metric | Target |
|-----------|--------|--------|
| Container startup | Context creation + executor prepare | < 10ms |
| Buffer pool | 1000 allocate/release cycles | Tracks alloc/reuse ratio |
| Namespace cache | Cache hit vs miss timing | Hit < 1µs |
| Resource limits | Full batch apply | < 100µs |
| Context pool | Acquire/release cycles | Tracks recycling rate |

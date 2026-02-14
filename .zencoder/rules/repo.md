---
description: Repository Information Overview
alwaysApply: true
---

# Envyro Repository Information

## Repository Summary
Envyro is a sophisticated multi-component project featuring two primary systems: **Envyro-Core**, a custom Transformer-based AI neural engine with vectorized Long-Term Memory (LTM), and **Enviro**, a high-concurrency, zero-trust containerization engine built using a multi-language architecture (Rust, Go, Zig, and Python).

## Repository Structure
- **`envyro_core/`**: Python package containing the core AI logic, Transformer models, and vector memory management.
- **`enviro-core/`**: Rust-based core runtime for the containerization engine.
- **`enviro-go/`**: Go-based gRPC control plane and networking for the container runtime.
- **`enviro-py/`**: Python SDK for the Enviro runtime, utilizing Maturin/PyO3 for Rust integration.
- **`enviro-zig/`**: Low-level Zig syscall wrappers and memory allocators.
- **`uploads/`**: Directory for file processing via the AI engine.

## Projects

### Envyro-Core (AI Neural Engine)
**Main Package**: `envyro_core/`  
**Configuration File**: `requirements.txt`, `.env.example`, `docker-compose.yml`

#### Language & Runtime
**Language**: Python  
**Version**: >=3.8 (Docker uses 3.10-slim)  
**Package Manager**: `pip`  
**Database**: PostgreSQL 12+ with `pgvector` extension

#### Dependencies
**Main Dependencies**:
- `torch>=2.0.0` (Neural engine)
- `numpy>=1.24.0` (Numerical operations)
- `psycopg2-binary`, `pgvector` (Memory management)
- `bcrypt`, `cryptography`, `PyJWT` (Security)
- `flask`, `flask-cors` (Web Interface)

#### Build & Installation
```bash
pip install -r requirements.txt
python setup_admiral.py  # Create admin account
python launch.py         # Start web launcher
```

#### Docker
**Dockerfile**: `./Dockerfile`  
**Configuration**: Root `docker-compose.yml` orchestrates a `postgres` service (with pgvector) and the `envyro-core` neural engine.

#### Testing
**Test Location**: Root directory  
**Run Command**:
```bash
python comprehensive_test.py
python test_structure.py
```

---

### Enviro-Core (Rust Runtime)
**Configuration File**: `enviro-core/Cargo.toml`

#### Language & Runtime
**Language**: Rust  
**Edition**: 2021  
**Build System**: Cargo

#### Dependencies
- `tokio`: Async runtime
- `nix`: Linux primitives
- `tonic`/`prost`: gRPC for Go integration
- `libloading`: Dynamic plugin loading
- `serde`: Serialization

#### Build & Installation
```bash
cd enviro-core
cargo build
```

---

### Enviro-Go (Control Plane)
**Configuration File**: `enviro-go/go.mod`

#### Language & Runtime
**Language**: Go  
**Version**: 1.21  
**Build System**: `go build`

#### Dependencies
- `google.golang.org/grpc`: Control plane communication
- `google.golang.org/protobuf`: Protocol buffers

---

### Enviro-Py (Python SDK)
**Configuration File**: `enviro-py/pyproject.toml`

#### Language & Runtime
**Language**: Python / Rust  
**Build System**: `maturin`  
**Version**: Python >=3.8

---

### Enviro-Zig (Syscall Wrapper)
**Configuration File**: `enviro-zig/build.zig.zon`

#### Language & Runtime
**Language**: Zig  
**Build System**: `zig build`

#### Key Resources
- `src/allocator.zig`: Custom memory allocation
- `src/oom_tuner.zig`: OOM management

#### Usage & Operations
```bash
cd enviro-zig
zig build
```

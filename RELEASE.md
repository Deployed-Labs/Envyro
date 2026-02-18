# Release Process

This document describes how to create a new release of Envyro.

## Automated Release Workflow

Envyro uses GitHub Actions to automatically build and publish releases. The workflow builds the `enviro` binary for multiple platforms and attaches them to GitHub releases.

### Supported Platforms

The release workflow builds executables for the following platforms:

- **Linux (glibc)**: `enviro-linux-x86_64`
- **Linux (musl)**: `enviro-linux-x86_64-musl` (static binary, works on any Linux)
- **macOS (Intel)**: `enviro-macos-x86_64`
- **macOS (Apple Silicon)**: `enviro-macos-aarch64`
- **Windows**: `enviro-windows-x86_64.exe`

Each binary includes a SHA256 checksum file for verification.

## Creating a Release

### Method 1: Git Tag (Recommended)

1. Ensure all changes are committed and pushed to the main branch
2. Create and push a version tag:

```bash
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

3. The GitHub Actions workflow will automatically:
   - Build executables for all platforms
   - Generate SHA256 checksums
   - Create a GitHub release
   - Upload all binaries and checksums
   - Generate release notes from commits

### Method 2: Manual Workflow Dispatch

You can also trigger the build manually from the GitHub Actions tab:

1. Go to the **Actions** tab in the GitHub repository
2. Select the **Build and Release** workflow
3. Click **Run workflow**
4. Choose the branch to build from
5. Click **Run workflow**

Note: Manual runs will build the binaries but won't create a release (only tag pushes create releases).

## Release Artifacts

Each release includes:

- **Binaries**: Pre-compiled executables for each platform
- **Checksums**: SHA256 checksum files (`.sha256`) for verification
- **Release Notes**: Auto-generated from commit messages

## Verifying Downloads

To verify a downloaded binary:

### Linux/macOS:
```bash
shasum -a 256 -c enviro-linux-x86_64.sha256
```

### Windows (PowerShell):
```powershell
Get-FileHash enviro-windows-x86_64.exe -Algorithm SHA256
# Compare output with content of .sha256 file
```

## Version Numbering

Envyro follows [Semantic Versioning](https://semver.org/):

- **Major version (v1.0.0)**: Breaking changes
- **Minor version (v0.1.0)**: New features, backwards compatible
- **Patch version (v0.0.1)**: Bug fixes, backwards compatible

## Build Configuration

The release binaries are built with optimizations enabled:

- LTO (Link-Time Optimization)
- Symbol stripping for smaller binaries
- Single codegen unit for maximum optimization
- Panic = abort for smaller binaries

See `Cargo.toml` `[profile.release]` section for full configuration.

## Troubleshooting

### Build Failures

If a build fails for a specific platform:

1. Check the GitHub Actions logs for that platform
2. The workflow continues building other platforms even if one fails
3. Fix the issue and create a new tag to retry

### Missing Go/Zig Support

The `enviro` binary can be built without Go and Zig FFI support. If these components are not available during the build, the workflow will:

- Display warnings about missing components
- Continue building without FFI support
- Produce a functional but limited binary

For full functionality, ensure Go and Zig are installed in the build environment.

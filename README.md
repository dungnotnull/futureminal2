# Futureminal

> The next-generation terminal: AI-native, Privacy-first, Blockchain-auditable.

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust](https://img.shields.io/badge/Rust-1.92%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)]()

---

## Overview

**Futureminal** is an open-source terminal emulator built on top of [Warp](https://warp.dev)'s proven core engine (`warp_terminal`), extended with cutting-edge features for the modern developer:

- **AI-Native**: Multi-provider AI abstraction (OpenAI, Anthropic, Gemini, Ollama, LM Studio) with privacy guardrails and local audit logging.
- **Blockchain-Auditable**: Immutable command audit logs with optional on-chain notarization (Ethereum, Solana, local chains).
- **Plugin Ecosystem**: Extensible WASM + Lua plugin host for custom terminal workflows.
- **Privacy-First**: All sensitive data is sanitized before leaving the terminal. Local processing is the default.

---

## Architecture

```
Futureminal
    |
    +-- warp_terminal (Warp's open-source terminal engine - Alacritty-derived)
    +-- futureminal-core (Terminal grid, VT parser, PTY, shell integration)
    +-- futureminal-renderer (GPU-accelerated text rendering - wgpu)
    +-- futureminal-ai (Multi-provider AI router with privacy sanitization)
    +-- futureminal-blockchain (Command audit logs + on-chain notarization)
    +-- futureminal-plugin (WASM/Lua plugin host)
    +-- futureminal-ipc (Daemon/UI inter-process communication)
```

---

## Project Structure

| Crate | Description | Status |
|-------|-------------|--------|
| `crates/futureminal` | Main binary entry point | Compiling |
| `crates/futureminal-core` | Terminal grid, VT100/xterm parser, PTY management | Compiling |
| `crates/futureminal-renderer` | GPU text rendering (wgpu stub - needs port to wgpu 29.x) | Stub |
| `crates/futureminal-ai` | AI provider abstraction + privacy sanitizer | Compiling |
| `crates/futureminal-blockchain` | Blockchain audit adapter + vault | Compiling |
| `crates/futureminal-plugin` | Plugin host (WASM/Lua stub - needs mlua build deps) | Stub |
| `crates/futureminal-ipc` | Cross-platform IPC (Unix sockets / Windows named pipes) | Compiling |
| `warp-fork/crates/warp_terminal` | Warp's open-source terminal engine (Apache 2.0) | Upstream |
| `warp-fork/crates/warp_core` | Warp's shared types and utilities | Upstream |

---

## Building

### Prerequisites

- **Rust 1.92+** (managed by `rust-toolchain.toml`)
- **Windows**: Visual Studio Build Tools or MSVC
- **macOS**: Xcode Command Line Tools
- **Linux**: `build-essential`, `pkg-config`
- **Optional**: Lua 5.4 + pkg-config (for full `futureminal-plugin` with mlua)
- **Optional**: libclang (for some upstream Warp crate dependencies)

### Quick Start

```bash
# Clone the repository
git clone https://github.com/dungnotnull/futureminal.git
cd futureminal

# Build the main binary
cargo build -p futureminal --release

# Run tests for all Futureminal crates
cargo test -p futureminal-core -p futureminal-ai -p futureminal-blockchain -p futureminal-ipc
```

### Running

```bash
# Interactive mode (default)
cargo run -p futureminal

# Daemon mode
cargo run -p futureminal -- --daemon

# With blockchain features enabled
cargo run -p futureminal --features blockchain
```

---

## Testing

```bash
# Run all Futureminal tests
cargo test -p futureminal-core -p futureminal-ai -p futureminal-blockchain -p futureminal-ipc -p futureminal-plugin -p futureminal-renderer -p futureminal --lib
```

**Current test status:**
- `futureminal-core`: 22 passed, 3 ignored (VT parser edge cases)
- `futureminal-ai`: 11 passed
- `futureminal-blockchain`: 10 passed, 2 ignored (batch flush logic)
- `futureminal-ipc`: 1 passed

---

## Roadmap

### Phase 0: Foundation (Done)
- [x] Workspace integration with Warp's open-source crates
- [x] Core terminal emulation (grid, VT parser, PTY)
- [x] AI provider abstraction layer
- [x] Blockchain audit adapter framework
- [x] IPC transport layer

### Phase 1: Integration (In Progress)
- [ ] Port `futureminal-renderer` from wgpu 0.20 to wgpu 29.x
- [ ] Port `futureminal-plugin` to build without system Lua dependency
- [ ] Integrate `warp_terminal` types into `futureminal-core`
- [ ] Cross-platform windowing (winit-based)

### Phase 2: Production Hardening
- [ ] Full test coverage for VT parser
- [ ] GPU renderer performance optimization
- [ ] Plugin sandbox security audit
- [ ] CI/CD pipelines

### Phase 3: Advanced Features
- [ ] AI agent mode (autonomous terminal tasks)
- [ ] Real-time collaborative sessions
- [ ] Custom themes and shaders
- [ ] Marketplace for plugins

---

## License

This project is licensed under the **AGPL-3.0-only** license.

The `warp-fork/crates/warp_terminal` code is derived from Alacritty and licensed under **Apache-2.0** (see `crates/warp_terminal/src/model/LICENSE-ALACRITTY`).

---

## Acknowledgements

- **Warp** ([warpdotdev/Warp](https://github.com/warpdotdev/Warp)) for the open-source terminal engine
- **Alacritty** for the foundational terminal emulation code
- The Rust async and terminal emulator communities

---

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

> **Note**: This is a real fork of Warp's repository. We are actively working to strip proprietary cloud features and build an independent, open-source terminal that anyone can run, modify, and extend.

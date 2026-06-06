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
- **Plugin Ecosystem**: Extensible JavaScript plugin host (QuickJS-powered) for custom terminal workflows.
- **Privacy-First**: All sensitive data is sanitized before leaving the terminal. Local processing is the default.
- **GPU-Accelerated**: Real wgpu 29.x renderer with a distinctive visual identity (deep slate blue theme).

---

## Architecture

```
Futureminal
    |
    +-- warp_terminal (Warp's open-source terminal engine - Alacritty-derived)
    +-- futureminal-core (Terminal grid, VT parser, PTY, shell integration, windowing)
    +-- futureminal-renderer (GPU-accelerated text rendering - wgpu 29.x)
    +-- futureminal-ai (Multi-provider AI router with privacy sanitization)
    +-- futureminal-blockchain (Command audit logs + on-chain notarization)
    +-- futureminal-plugin (JavaScript plugin host via QuickJS)
    +-- futureminal-ipc (Daemon/UI inter-process communication)
```

---

## Project Structure

| Crate | Description | Status | Tests |
|-------|-------------|--------|-------|
| `crates/futureminal` | Main binary entry point | Compiling | - |
| `crates/futureminal-core` | Terminal grid, VT100/xterm parser, PTY, windowing | Compiling | 30 passed |
| `crates/futureminal-renderer` | GPU text rendering (wgpu 29.x) | Compiling | 3 passed |
| `crates/futureminal-ai` | AI provider abstraction + privacy sanitizer | Compiling | 11 passed |
| `crates/futureminal-blockchain` | Blockchain audit adapter + vault | Compiling | 12 passed |
| `crates/futureminal-plugin` | JavaScript plugin host (QuickJS) | Compiling | 6 passed |
| `crates/futureminal-ipc` | Cross-platform IPC (Unix sockets / Windows named pipes) | Compiling | 1 passed |
| `warp-fork/crates/warp_terminal` | Warp's open-source terminal engine (Apache 2.0) | Upstream | - |
| `warp-fork/crates/warp_core` | Warp's shared types and utilities | Upstream | - |

**Total: 63 tests, 100% pass rate, 0 ignored.**

---

## Visual Identity

Futureminal uses a **distinctive deep slate blue theme** (`#0F1420` background) that is visually different from Warp's default appearance:

```rust
// Futureminal's default theme (in futureminal-renderer)
background: [0.06, 0.08, 0.12, 1.0], // Deep slate blue
foreground: [0.85, 0.87, 0.91, 1.0], // Soft white
```

The window chrome, tab styling, and UI panels are all custom-designed for Futureminal.

---

## Building

### Prerequisites

- **Rust 1.92+** (managed by `rust-toolchain.toml`)
- **Windows**: Visual Studio Build Tools or MSVC
- **macOS**: Xcode Command Line Tools
- **Linux**: `build-essential`, `pkg-config`

### Quick Start

```bash
# Clone the repository
git clone https://github.com/dungnotnull/futureminal.git
cd futureminal

# Build the main binary
cargo build -p futureminal --release

# Run all tests (must pass 100%)
cargo test -p futureminal-core -p futureminal-ai -p futureminal-blockchain -p futureminal-ipc -p futureminal-plugin -p futureminal-renderer -p futureminal --lib
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

All tests must pass before any code is considered production-ready:

```bash
cargo test -p futureminal-core -p futureminal-ai -p futureminal-blockchain -p futureminal-ipc -p futureminal-plugin -p futureminal-renderer -p futureminal --lib
```

**Current test status:**
| Crate | Passed | Failed | Ignored |
|-------|--------|--------|---------|
| futureminal-core | 30 | 0 | 0 |
| futureminal-ai | 11 | 0 | 0 |
| futureminal-blockchain | 12 | 0 | 0 |
| futureminal-ipc | 1 | 0 | 0 |
| futureminal-plugin | 6 | 0 | 0 |
| futureminal-renderer | 3 | 0 | 0 |

---

## Roadmap

### Phase 0: Foundation (Complete)
- [x] Workspace integration with Warp's open-source crates
- [x] Core terminal emulation (grid, VT parser, PTY)
- [x] AI provider abstraction layer
- [x] Blockchain audit adapter framework
- [x] IPC transport layer
- [x] wgpu 29.x GPU renderer
- [x] JavaScript plugin host
- [x] Cross-platform windowing abstraction
- [x] 100% test pass rate (63 tests)

### Phase 1: Production Hardening
- [ ] Full VT sequence coverage tests
- [ ] GPU renderer text atlas + glyph rendering
- [ ] Plugin sandbox security audit
- [ ] CI/CD pipelines

### Phase 2: Advanced Features
- [ ] AI agent mode (autonomous terminal tasks)
- [ ] Real-time collaborative sessions
- [ ] Custom shaders and animations
- [ ] Plugin marketplace

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

We welcome contributions! Please ensure all tests pass before submitting:

```bash
cargo test -p futureminal-core -p futureminal-ai -p futureminal-blockchain -p futureminal-ipc -p futureminal-plugin -p futureminal-renderer -p futureminal --lib
```

> **Note**: This is a real fork of Warp's repository. We have stripped proprietary cloud features and built an independent, open-source terminal that anyone can run, modify, and extend. Futureminal is a **distinct project** with its own visual identity, architecture, and feature set.

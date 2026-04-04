<p align="center">
  <img src="resources/logo.svg" width="120" alt="Oryxis logo">
</p>

<h1 align="center">Oryxis</h1>

<p align="center">
  A modern SSH client built entirely in Rust — fast, encrypted, peer-to-peer.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-1.90%2B-orange?logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/platform-linux-blue" alt="Platform">
  <img src="https://img.shields.io/badge/status-early%20development-yellow" alt="Status">
</p>

---

## What is Oryxis?

Oryxis is an open-source alternative to [Termius](https://termius.com/) — a desktop SSH client with a modern UI, an encrypted vault for credentials, and decentralized sync between devices. No Electron, no webview, no cloud servers. Just a single native binary.

### Why?

Most SSH clients are either powerful but ugly (PuTTY), pretty but Electron-heavy (Termius, Tabby), or terminal-only (OpenSSH). Oryxis aims to be all three: **beautiful, fast, and native**.

## Features

- **Native GPU-accelerated UI** — Built with [Iced](https://iced.rs) (wgpu backend). No webview, no JavaScript.
- **Embedded terminal emulator** — Powered by [alacritty_terminal](https://github.com/alacritty/alacritty) for accurate VT100/VT220/xterm emulation with 256-color and truecolor support.
- **Encrypted vault** — Credentials and SSH keys stored locally with Argon2id key derivation + ChaCha20Poly1305 encryption.
- **P2P sync** — Share connection folders between team members using [iroh](https://iroh.computer/) (QUIC-based, NAT-traversing, no central server).
- **Pure Rust SSH** — Connections handled by [russh](https://github.com/warp-tech/russh), async and with no C dependencies.
- **Single binary** — `cargo build --release` and you're done.

## Architecture

```
┌─ Iced Application (wgpu) ────────────────────┐
│                                               │
│  Sidebar ─ Connection tree, search, groups    │
│  Terminal ─ Canvas rendering + tab bar        │
│  Views ─ Dashboard, Keys, Snippets, Settings  │
│                                               │
├───────────────────────────────────────────────┤
│  oryxis-ssh     │  oryxis-vault               │
│  (russh)        │  (SQLite + age + Argon2id)  │
├───────────────────────────────────────────────┤
│  oryxis-sync (iroh / QUIC)                    │
└───────────────────────────────────────────────┘
```

The project is organized as a Cargo workspace with focused crates:

| Crate | Purpose |
|-------|---------|
| `oryxis-app` | Main application, UI, window management |
| `oryxis-core` | Shared types — Connection, SshKey, Group, Snippet |
| `oryxis-terminal` | Terminal widget (alacritty_terminal + Iced canvas + PTY) |
| `oryxis-ssh` | SSH connection engine (russh wrapper) |
| `oryxis-vault` | Encrypted credential and key storage |
| `oryxis-sync` | Peer-to-peer sync engine (iroh wrapper) |

## Tech Stack

| Layer | Technology | Why |
|-------|-----------|-----|
| UI | Iced 0.14 | Pure Rust, GPU-accelerated, retained-mode |
| Terminal parsing | alacritty_terminal | Battle-tested, accurate emulation |
| Font rendering | cosmic-text | Advanced shaping, Unicode/emoji |
| SSH | russh | Async, pure Rust, no OpenSSL |
| Encryption | age + Argon2id + ChaCha20Poly1305 | Modern, audited cryptographic primitives |
| P2P | iroh | QUIC-based sync with NAT traversal |
| Storage | SQLite (rusqlite) | Embedded, zero-config |
| Async | Tokio | Industry standard |

## Building

### Prerequisites

- Rust 1.90+ (install via [rustup](https://rustup.rs/))
- Linux with X11 or Wayland
- GPU with OpenGL/Vulkan support (for wgpu)

### Build & Run

```bash
git clone https://github.com/wilsonglasser/oryxis.git
cd oryxis

# Debug
cargo run

# Release (optimized)
cargo build --release
./target/release/oryxis

# With logging
RUST_LOG=oryxis=debug cargo run
```

## Current Status

Oryxis is in **early development** (pre-v0.1). Here's where things stand:

### Working
- Application shell with dark theme (Termius-inspired)
- Sidebar with connection tree and groups
- Terminal widget with full VT emulation (256-color, truecolor)
- PTY spawning and I/O
- Keyboard handling (all keys, ctrl+key, function keys)
- Tab bar for multiple sessions
- Dashboard, Keys, Snippets, and Settings views

### In Progress
- SSH connection pipeline (direct, jump hosts, proxies)
- Vault encryption and database
- P2P sync implementation
- SFTP / file transfer
- Split panes
- Session recording

## Roadmap

| Version | Scope |
|---------|-------|
| **v0.1** | Local SSH connections, vault with master password, basic key management |
| **v0.2** | Jump hosts, SOCKS/HTTP proxy, port forwarding, snippets |
| **v0.3** | P2P folder sharing, CRDT-based sync, peer roles (owner/editor/viewer) |
| **v0.4** | SFTP panel, split panes, session recording |

## Contributing

The project is early-stage — contributions, ideas, and feedback are welcome. Open an issue to discuss before submitting large PRs.

## License

TBD

---

<p align="center">
  Built with Rust, for people who live in the terminal.
</p>
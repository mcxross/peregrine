<p align="center">
   <a href="https://mcxross.xyz/">
     <img src="https://raw.githubusercontent.com/mcxross/peregrine/main/public/peregrine-logo.png" alt="Peregrine logo" width="200" height="200">
   </a>
</p>

<h3 align="center">Peregrine</h3>

<p align="center">
  Peregrine is what you need when code is secondary and understanding behavior is everything
   <br>
</p>

> [!WARNING]
> **This project is under active development**
>
> Thing are changing rapidly, and the current state of the project may not be stable. Use with caution and expect breaking changes.

# Development

## Prerequisites

- [Rust](https://www.rust-lang.org/) (latest stable)
- Node.js 22
- [bun](https://bun.sh/)

## Getting Started

```bash
# Install dependencies
bun install

# Build the isolated sidecars and run the Tauri desktop app
bun run dev:desktop

# Build the isolated sidecars and run the TUI
bun run dev:tui
```

## Cargo

```bash
# Compile the full workspace
cargo build

# Build a runnable release TUI with sibling sidecars
bun run build:tui

# Run tests
cargo test --workspace

# Lint and check
cargo fmt
cargo clippy --workspace --all-targets
```

## Architecture

See [docs/analysis-architecture.md](docs/analysis-architecture.md) for the
chain-neutral analysis engine, Sui plugin composition, and MCP server layout.

## Validation

```bash
# Tauri desktop bundle with packaged-sidecar verification
bun run build:desktop

# Rust
cargo fmt
cargo clippy --workspace --all-targets
cargo test --workspace
```

## Sidecar Packaging

The desktop and TUI binaries do not embed the Sui MCP servers or the Peregrine
helper. A runnable Peregrine installation must place these executables beside
the frontend executable:

- `peregrine-helper`
- `peregrine-sui-mcp-server`
- `peregrine-sui-move-analyzer-mcp-server`

`bun run dev:desktop`, `bun run build:desktop`, `bun run dev:tui`, and
`bun run build:tui` build and verify this complete process set. Directly
building only `peregrine-tui` is useful for compilation checks, but does not
produce a complete installation. Release bundles must be built natively on
each target platform so the packaging preflight can execute every sidecar.

## License

    Copyright 2026 McXross

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.

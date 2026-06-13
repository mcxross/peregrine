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

# Build and run the Tauri desktop app
bun run tauri dev
```

## Cargo

```bash
# Build the full workspace
cargo build

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
# Tauri desktop build
bun run tauri build

# Rust
cargo fmt
cargo clippy --workspace --all-targets
cargo test --workspace
```

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

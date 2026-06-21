# Toolchains and MCP Integration

Peregrine is built from the ground up to be a blockchain-agnostic security agent. Instead of hardcoding logic for specific blockchains or smart contract languages into the core agent, Peregrine relies entirely on the **Model Context Protocol (MCP)** to execute specialized tasks.

Peregrine utilizes MCP servers to construct its auditing toolchains, allowing you to configure these tools to fit your specific security needs.

## Blockchain Agnosticism via MCP

Smart contract ecosystems evolve rapidly. New languages, testing frameworks, and vulnerability classes emerge constantly. If Peregrine's core logic was tightly coupled to a specific chain (like Ethereum or Sui), it would quickly become outdated.

To solve this, Peregrine's core audit lifecycle operates on generic concepts (e.g., *Target*, *Evidence*, *Trace*, *Report*). Whenever Peregrine needs to interact with a specific blockchain environment—whether to compile code, run a static analyzer, or fuzz a function—it delegates the work to an external **MCP Server**.

Because of this architecture, adding support for a new blockchain simply means connecting a new MCP server that provides tools for that chain's ecosystem.

## Out-of-the-Box Components

Peregrine ships with several powerful MCP servers right out of the box. While they are bundled for convenience, they act as entirely independent, self-contained components:

- **Eidetic:** Handles agent memory and project context.
- **Chain Adapters (e.g., Sui Security Adapter):** Provides blockchain-specific capabilities like source code normalization, dependency resolution, and exploit replay for the Sui ecosystem.
- **Analysis Engines:** Connects to specialized fuzzers, symbolic executors, and graph analyzers.

Because these are modular MCP components rather than monolithic core features, they can be individually updated, swapped, or removed without impacting the core Peregrine orchestration engine.

## User-Controlled Configuration

While Peregrine provides a default toolchain, **you are in complete control of what tools the agent can use.**

You decide which MCP servers are active and which specific tools are exposed to the agent during an audit. This flexibility is managed through your `config.toml` file.

### Adding or Disabling MCP Servers

You can register custom MCP servers or disable out-of-the-box servers using the `[mcp_servers]` configuration block:

```toml
[mcp_servers.my_custom_fuzzer]
command = "npx"
args = ["-y", "my-fuzzer-mcp", "serve"]

[mcp_servers.sui_adapter]
# You can disable an out-of-the-box toolchain component entirely
enabled = false 
```

### Capability Discovery (The ToolRouter)

During an audit, Peregrine does not blindly assume that a specific tool (like a Move fuzzer) exists. 

Instead, it uses its internal **ToolRouter** to dynamically discover capabilities. Before executing a stage of the audit plan, the coordinator will:
1. Search the currently connected MCP servers for tools matching the required capabilities (e.g., "dynamic.fuzzing").
2. Execute the tool if it exists to generate verified evidence.
3. If no tool is configured for that capability, Peregrine explicitly records a "coverage gap" in the final report. It will not silently pass or pretend the analysis was completed.

This means you can easily customize your toolchain by plugging in your preferred proprietary or open-source analyzers, and Peregrine will automatically discover and integrate them into the audit workflow.

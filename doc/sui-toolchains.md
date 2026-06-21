# Sui Toolchain Ecosystem

While Peregrine’s orchestration engine is blockchain-agnostic (as detailed in [Toolchains and MCP Integration](toolchains.md)), the platform ships with a comprehensive, out-of-the-box security toolchain specifically designed for the **Sui Network** and the **Move** programming language.

This ecosystem is composed of specialized components that provide discrete capabilities to the core audit lifecycle. Instead of being tightly coupled to the main Peregrine agent, these tools are exposed via the Model Context Protocol (MCP) or managed through chain adapters.

## The Adapter Layer

The adapters serve as the bridge between Peregrine's generic audit lifecycle and the realities of compiling, testing, and interacting with Sui.

- **Sui Security Adapter (`peregrine-sui-security-adapter`)**: The high-level implementation of the `AuditChainAdapter` interface. It orchestrates preflight checks, target acquisition, and chain-specific workflows during an audit.
- **Sui Base Adapter (`peregrine-sui-adapter`)**: The foundational adapter for interacting with the Sui network RPCs and compiling local Move code.

## Analysis Engines

The core of the Sui auditing capability is broken down into highly specialized analysis engines. These engines provide the tools and evidence that Peregrine's agents (Researcher, Exploiter, etc.) rely on.

### Static Analysis & Scanning
- **`peregrine-sui-scanner`**: Performs fast, pattern-based static scanning of Move source code to quickly flag common vulnerability classes.
- **`peregrine-sui-static-analysis`**: Executes deeper static analysis passes over Move ASTs, looking at data-flow and access control issues.
- **`peregrine-sui-move-insights`**: Extracts high-level security insights from raw code metrics and structural analysis.

### Dynamic Analysis & Fuzzing
- **`peregrine-sui-dynamic-analysis`**: Powers dynamic execution, fuzzing, and state manipulation against compiled Move bytecode. This component integrates advanced toolchains like **Movy** to conduct deep dynamic testing and invariant checking. The Exploiter agent heavily relies on these dynamic execution environments to confirm theoretical vulnerabilities through actual exploit replays.

### Formal Verification
- **Sui Prover Integration**: Peregrine includes built-in support for the **Sui Prover**, allowing agents to write and execute formal specifications against Move smart contracts. This mathematical proof engine rigorously verifies that protocol invariants hold true across all possible execution paths, providing the highest level of assurance beyond standard fuzzing. This capability is deeply integrated through `peregrine-sui-dynamic-analysis` and `peregrine-sui-adapter`.

### Graph & Semantic Analysis
- **`peregrine-sui-move-graph`**: Generates control flow and semantic graphs from Move modules. This helps agents visually or programmatically trace asset flows and object ownership transfers.
- **`peregrine-sui-move-model`**: Extracts a formal mathematical or semantic representation of the Move codebase.
- **`peregrine-sui-move-analyzer`**: Language server and semantic analyzer for understanding deep Move typing and function relationships.

### Bytecode Analysis
- **`peregrine-sui-bytecode`**: Inspects compiled Sui Move bytecode directly. This is critical for verifying on-chain deployment artifacts against provided source code, or finding compiler-level edge cases.

## Package Management & Data Retrieval

Auditing a modern protocol often means auditing its dependencies. These components manage the complex graph of on-chain and local dependencies.

- **`peregrine-sui-package-resolution`** & **`peregrine-sui-import-engine`**: Handlers for resolving, downloading, and managing complex on-chain or GitHub-based Sui package dependencies.
- **`peregrine-sui-project-loader`**: Loads and normalizes local Sui projects into the isolated audit workspace.
- **`peregrine-sui-indexer`**: Emits structure, evidence, spans, and diagnostics from the codebase for efficient retrieval by the agent. *(Note: By design, the indexer only emits structural evidence; it does not decide vulnerability risk).*

## MCP Protocol Integration

To maintain strict architectural boundaries, the Sui toolchain features are wrapped and exposed as MCP tools.

- **`peregrine-sui-mcp-protocol`** & **`peregrine-sui-move-analyzer-mcp-protocol`**: These protocol bridges expose the underlying analysis engines as standard MCP tools. This ensures that the core Peregrine agent interacts with the Sui tools exclusively via the generic `ToolRouter`, rather than directly importing Sui logic into the coordinator loop.

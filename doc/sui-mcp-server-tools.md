# Peregrine SUI MCP Server Tools

The `peregrine-sui-mcp-server` exposes a rich set of tools for the LLM to inspect, analyze, and build Sui Move packages. These tools leverage the Eager Caching architecture to deliver instantaneous insights without stalling the agent.

## Available Tools

The server registers the following MCP tools for the model to use:

### 1. Package Resolution and Management
*   **`package_resolve`**: Validates and resolves a Sui Move package within the MCP workspace, verifying its existence and `Move.toml`.
*   **`create_package`**: Creates a new, boilerplate local Sui Move package on the filesystem.
*   **`import_package`**: Downloads and imports an on-chain Sui package (and its dependencies) into a localized, bounded artifact workspace.

### 2. Module & Source Exploration
*   **`modules`**: Lists the Move modules present in a package using bounded cursor pagination.
*   **`signatures`**: Lists the public and entry function signatures within a package using bounded cursor pagination.

### 3. Analysis & Security Scanners
*   **`scanner_report`**: Executes Peregrine's Sui Move pattern scanners and returns structural, source-backed evidence.
*   **`package_insights`**: Inspects a package for high-level security-relevant signals and potential vulnerability indicators.
*   **`static_analyze_package`**: Runs the full suite of Peregrine static analysis rules against the package.
*   **`static_rule_catalog`**: Lists all bundled and user-configured static analysis rules.

### 4. Graph Construction
*   **`graphs`**: Retrieves package-level graphs for function calls, type dependencies, and state-access. 
*   **`function_state_graph`**: Builds a focused, granular state-access graph for a specific Sui Move function.

### 5. Bytecode & Execution Tools
*   **`bytecode_view`**: Loads and formats compiled Sui Move bytecode, providing disassembly and control-flow blocks.
*   **`bytecode_decompile`**: Attempts to decompile root-package bytecode modules back into high-level Move source.
*   **`command`**: Runs an analysis-safe Sui CLI command safely sandboxed (e.g., `build`, `test`, `coverage`, `publishDryRun`, `moveFuzz`).
*   **`movy_fuzz`**: Executes the Movy local fuzzing engine against public target functions to find edge cases.
*   **`formal_verify`**: Runs the bundled Sui Prover to formally verify mathematically specified constraints on a specific module.
*   **`test_scanner_report`**: Inspects the package's testing health, listing unit, fuzz, invariant, and formal verification tests.

### 6. The Core Engine
*   **`analyze`**: An advanced tool that directly invokes the shared Sui analysis engine over a local or on-chain package with complete control over stages, capabilities, and options.

---

## Graph Filtering Mechanics

When a model requests structural context via the `graphs` or `function_state_graph` tools, the server processes the pre-computed eager cache and applies filtering. This allows the model to constrain the context window and focus on relevant logic. 

Filtering is performed locally within the server against the cached `EngineAnalysisReport` using several parameters:

1. **Module Name Filtering (`modules`)**:
   The model can pass an array of string identifiers (e.g., `["amm", "pool"]`). The server strips out any graph nodes that do not originate from or target these specific modules, severely reducing the visual noise and token count.
2. **External Dependency Filtering (`include_external`)**:
   By default, the server aggressively prunes edges that point to external dependencies (like `0x2::sui` or `0x1::option`) unless explicitly requested. This keeps the focus entirely on the local package logic.
3. **Depth Limiting (`depth`)**:
   For deeply nested call graphs or complex type dependencies, the server implements depth-first pruning. If the model specifies a `depth` of `2`, the server traverses outward from the target module nodes and strictly cuts off any edges traversing further than 2 hops away.

This multi-axis filtering system ensures the graph data returned via the MCP protocol is directly relevant to the user's immediate question without blowing out the model's maximum token context limit.

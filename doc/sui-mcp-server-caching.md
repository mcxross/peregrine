# Peregrine SUI MCP Server Caching Architecture

## Overview

The `peregrine-sui-mcp-server` serves as the primary bridge between the Peregrine TUI/model and the underlying Move package analysis engine. To ensure that the Large Language Model (LLM) receives fast, accurate, and up-to-date context, the server implements an **Eager Caching** architecture.

Instead of lazily rebuilding the analysis graph every time the model requests package resolution or module graphs, the server proactively monitors the file system for changes and maintains a constantly updated, in-memory representation of the graph.

## Why Eager Caching?

1. **Low Latency for the Model**:
   When an LLM agent needs context, it needs it immediately. Lazy loading would introduce significant latency (often multiple seconds) on every context request, severely degrading the UX and the agent's reasoning speed.
2. **Reduced Redundant Work**:
   Agents often make multiple rapid requests (e.g., getting the package structure, then asking for several module graphs). Eager caching ensures the heavy lifting (scanning, AST parsing, graph building) is done exactly once per user edit.
3. **Remote Server Compatibility**:
   While file-based hashing could work for local setups, it limits the server's ability to be deployed remotely where the TUI and the MCP Server do not share a filesystem. An eager watcher model handles state internally.

## How it Works

The implementation relies on several key components located primarily in `crates/peregrine-mcp-server/sui/src/cache.rs` and integrated via `server.rs`.

### 1. File System Watcher (`notify`)
The `EagerCache` utilizes the `notify` crate to establish a background watcher on the user's `package_root`.
- **Targeted Monitoring**: It filters events to only react to modifications of `.move` source files and `Move.toml` configuration files. This prevents irrelevant file changes (like READMEs or build artifacts) from triggering expensive rebuilds.

### 2. Debouncing (2 Seconds)
To prevent build thrashing when a user saves multiple files in quick succession (or during git operations), the watcher events are piped through a debouncer.
- The rebuild task waits for 2 seconds of silence after the last relevant file change before it kicks off the `AnalysisEngine`.

### 3. State Management (`PackageState`)
The cache maintains the status of each monitored package using a thread-safe structure (`RwLock<HashMap<PathBuf, PackageState>>`). The states are:
- `Analyzing(tokio::sync::watch::Receiver<()>)`: Indicates a build is currently in progress. The attached `watch` receiver allows incoming requests to efficiently await completion.
- `Ready(Arc<EngineAnalysisReport>)`: The graph has been built successfully and is ready for instant retrieval.
- `Failed(String)`: The build failed (e.g., due to a severe syntax error).

### 4. Explicit Blocking & Client Notification
When the MCP server receives a tool request (like `package/resolve` or `graph/module`) while the cache is in the `Analyzing` state, it does **not** fail or return stale data.
- **Blocking**: The request gracefully blocks by awaiting the `watch` receiver channel (`rx.changed().await`).
- **Transparency**: While blocked, the server explicitly informs the client (the Peregrine TUI) by dispatching a standard MCP `notifications/message` payload:
  `"Background analysis in progress. Blocking until graph is ready..."`
  This ensures the user and the TUI are aware that the agent is waiting on a compilation step, rather than hanging silently.

## Handling Graphs

Graph computation is one of the heaviest operations performed by the server, which is why eager caching is critical. Here is how graphs are generated, stored, and served:

1. **Pre-computed Analysis Stages**:
   When the background rebuild is triggered, the `AnalysisEngine` is requested to execute multiple specific stages: `AnalysisStage::Scan`, `AnalysisStage::Graph`, and sometimes `AnalysisStage::Static` or `AnalysisStage::Dynamic`. 
2. **Graph Kinds**:
   During the `Graph` stage, the engine specifically computes relational data for multiple dimensions simultaneously, including:
   - `GraphKind::CALL` (Function call graphs)
   - `GraphKind::TYPE` (Type dependency graphs)
   - `GraphKind::STATE_ACCESS` (Sui state read/write access graphs)
3. **Storage in the Report**:
   The output of these stages is aggregated into a single `EngineAnalysisReport`. This report acts as the single source of truth for the entire package and is what gets wrapped in the `PackageState::Ready` state. By storing the completed report, the server avoids regenerating ASTs or relations for individual modules.
4. **Retrieval**:
   When the model calls a tool like `graph/module`, the server retrieves the cached `EngineAnalysisReport`, locates the requested module, and slices out only the relevant sub-graphs (e.g., call depth or module filtering). 
5. **Flexible Output Formatting**:
   The engine provides raw relational data. Because the data is structured and eagerly pre-computed, it can be instantaneously mapped into whatever visualization format works best for the model or the user (e.g., DOT format, Mermaid, or raw JSON). The LLM is free to choose the representation that fits its reasoning context without incurring any backend computational delay.

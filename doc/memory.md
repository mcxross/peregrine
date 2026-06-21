# Memory in Peregrine

Peregrine implements a sophisticated, multi-tiered memory system designed to retain long-term context across smart contract audits, security research sessions, and protocol reviews. Memory ensures that the agent doesn't start from scratch, maintaining a continuous understanding of complex protocol invariants, vulnerabilities, and architectural patterns.

Peregrine's memory architecture relies on two main components:
1. **The internal Memories subsystem**
2. **Eidetic**: An external Agent-Agnostic Memory MCP Server

---

## The Internal Memories Subsystem

Peregrine automatically generates and organizes memories in the background as you audit codebases and research vulnerabilities. As you complete tasks and end sessions, Peregrine extracts key security insights, identified risks, and structural understanding from your conversations. Periodically, it consolidates, deduplicates, and structures this knowledge to ensure a clear record of protocol mechanics and security context without flooding your context window with redundant information.

This subsystem is configured in `config.toml` under the `[memories]` table.

### Configuration (`config.toml`)

You can govern how Peregrine handles and retains internal memory via these settings:

```toml
[memories]
# Enable or disable memory generation for new threads
generate_memories = true

# Enable or disable injecting historical memories into the agent's context
use_memories = true

# Expose dedicated memory management tools to the agent
dedicated_tools = true

# Maximum number of recent raw memories retained for global consolidation
max_raw_memories_for_consolidation = 100

# Maximum days since a memory was last used before it's ignored for consolidation
max_unused_days = 30

# Model used for extracting summaries and records from threads
extract_model = "gpt-5.4"

# Model used for consolidating raw memories into a structured knowledge base
consolidation_model = "gpt-5.4"
```

---

## Eidetic: Agent-Agnostic Memory (MCP)

While Peregrine maintains its own internal records, it also integrates with **Eidetic**, an Agent-Agnostic Memory server built on the Model Context Protocol (MCP).

*Note: Eidetic was originally built for Peregrine's auditing workflows, but for portability and reuse across the ecosystem, it is maintained as a separate component.*

Eidetic provides a structured, queryable long-term memory system that any MCP-compliant agent can use. It establishes a standard format for tracking audit observations, allowing vulnerability context or protocol invariants established by one agent to be instantly and reliably recalled by another.

**(For a full guide on configuring and operating Eidetic, see the [Eidetic Documentation](#) - link coming soon).**

### Storage Backends and Walrus (`memwal`)

Eidetic supports multiple storage layers, but the recommended backend for a robust security knowledge base is **Memwal (Walrus)**. 

Walrus provides decentralized, highly available storage that makes your agent's audit memories globally accessible and persistent. This ensures a resilient record of vulnerability findings and security reviews without relying on a centralized database provider.

When using the `memwal` backend, Eidetic stores both the memory payloads (the "artifacts") and the searchable **index** directly on the decentralized Walrus network. This approach completely eliminates the need for a local database file, providing true serverless, decentralized memory indexing and retrieval tailored for distributed security research teams.

# Sui/Move Knowledge Base

This directory is the Peregrine-owned Sui/Move security knowledge base used to
prime audit agents before they reason about Sui Move contracts. The selected
documentation content is vendored locally under `source/`; agents should not
need to follow external links during normal security work.

## Runtime Use

When an agent task is security-related and the project chain is Sui/Move,
Peregrine attaches the compact Sui/Move security context to the model
instructions and exposes it as a `relevantGuides` packet entry.

Agents can also call local deterministic knowledge tools:

- `rust.knowledge.sui_move.search`: search the vendored docs.
- `rust.knowledge.sui_move.read`: read a bounded local doc excerpt.

This knowledge is advisory. Current project source, compiler output, bytecode,
graph evidence, test results, and transaction traces remain canonical.

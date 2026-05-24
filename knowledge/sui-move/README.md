# Sui/Move Knowledge Base

This directory is the Peregrine-owned Sui/Move security knowledge base used to
prime audit agents before they reason about Sui Move contracts. The selected
documentation content is vendored locally under `source/`; agents should not
need to follow external links during normal security work.

Source snapshot:

- Repository: https://github.com/contract-hero/sui-pilot
- Commit: `b636affe685a3a8221e6f209d18c444226efc9f7`
- Local source root: `knowledge/sui-move/source`
- Vendored file count: 727
- Vendored size: about 6.7 MB
- Scope used here: documentation files, Move Book references, Sui docs, Sui
  security docs, Sui Prover docs, and Move review/quality skill guidance.

The harness does not edit the source snapshot in `/tmp/sui-pilot`. It copies the
selected documentation into `source/`, distills a compact prompt pack into
`packages/agent-runtime/src/sui-move-knowledge.ts`, and keeps the larger local
corpus here for retrieval, refreshes, and review.

## Runtime Use

When an agent task is security-related and the project chain is Sui/Move,
Peregrine attaches the compact Sui/Move security context to the model
instructions and exposes it as a `relevantGuides` packet entry.

Agents can also call local deterministic knowledge tools:

- `rust.knowledge.sui_move.search`: search the vendored docs.
- `rust.knowledge.sui_move.read`: read a bounded local doc excerpt.

This knowledge is advisory. Current project source, compiler output, bytecode,
graph evidence, test results, and transaction traces remain canonical.

## Refresh Checklist

1. Clone or update `contract-hero/sui-pilot` outside this directory.
2. Review documentation changes under `.move-book-docs`, `.sui-docs`,
   `.sui-prover-docs`, and `skills/*`.
3. Re-copy the selected documentation into `source/`.
4. Update `manifest.json`, `doc-index.json`, `move-security-rules.json`, and the
   compact runtime prompt in `packages/agent-runtime/src/sui-move-knowledge.ts`.
5. Keep the prompt compact enough for local models. Put detailed rules here,
   not in every prompt.
6. Run `bun test packages/agent-runtime/test` and `npm run build`.

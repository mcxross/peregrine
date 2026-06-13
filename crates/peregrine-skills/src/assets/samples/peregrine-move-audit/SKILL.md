---
name: peregrine-move-audit
description: Security audit workflow for Sui Move packages and Peregrine Move analysis output. Use when reviewing Move contracts, package upgrades, object ownership, capabilities, coin/accounting logic, dynamic fields, transfer policies, or Sui transaction security.
metadata:
  short-description: Audit Sui Move package security
---

# Peregrine Move Audit

Use this skill for Sui Move security reviews. Do not assume generated analysis is correct; use it as a starting point and verify against source.

## Scope

If the user names files or modules, audit only those files and their direct dependencies. Otherwise, discover Move sources with `rg --files -g '*.move'` and skip build artifacts, tests, examples, and generated directories unless the user asks for them.

## Review Checklist

- Object ownership and transfer: shared objects, owned objects, receiving, transfer policy, freeze/share/transfer flows, and object wrapping/unwrapping.
- Authority and capabilities: admin capabilities, witness types, one-time witnesses, package upgrade authority, and privileged entry functions.
- Asset accounting: coin conservation, balance splitting/joining, supply changes, fee paths, rounding, overflow/underflow, and zero-value edge cases.
- State invariants: dynamic fields, tables/bags, event consistency, versioning, paused states, and migration paths.
- Entry points: public/package/entry visibility, signer usage, object mutability, PTB composition, and reentrancy-like callback patterns through shared objects.
- Oracle and external data: freshness checks, decimal handling, stale values, and trust assumptions.
- Tests and proofs: missing negative tests, invariant tests, property tests, and Sui Prover specs where appropriate.

## Process

1. Read `Move.toml`, source modules, and relevant tests.
2. Map roles, capabilities, shared objects, assets, and entry functions.
3. Trace each critical asset/state transition from inputs to persistent state.
4. Run available local checks when practical, such as `sui move test`, existing Peregrine analyzers, or Peregrine's bundled Sui Prover path (`formal_verify` in the harness or `peregrine verify` in the CLI) for configured specs.
5. Report only actionable findings. Avoid speculative claims without a plausible exploit path.

## Output

For each finding include severity, affected module/function, exploit path, impact, recommended fix, and a regression test or proof obligation.

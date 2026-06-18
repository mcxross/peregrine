# Sui/Move Audit Context

This is the human-readable form of the compact context injected into Sui/Move
security agents.

## Source

Distilled from the bundled Peregrine Sui/Move knowledge corpus.

## Ground Rules

- Treat remembered Sui/Move knowledge as potentially stale.
- Use bundled docs as guidance, but current source, compiler output, bytecode,
  graph evidence, and test traces are canonical.
- Separate exposed attack surface from confirmed vulnerability.
- Confirm serious findings only with a source-to-evidence chain and validation
  evidence showing reachability and impact.

## Move 2024 Surface Semantics

- `public fun` is programmable-transaction-block composable when its signature
  can be satisfied.
- Absence of `entry` is not by itself a security boundary.
- `entry fun` is endpoint-only.
- `public entry` is generally redundant in modern Sui Move style.
- Reachability must consider visibility, object ownership, shared objects,
  ability constraints, private fields, generic constraints, and package/module
  boundaries.

## Authorization Patterns

- Owned objects and capability possession are authorization boundaries.
- A required `&AdminCap` or `&mut AdminCap` parameter proves the caller supplied
  that capability object, assuming no public path can mint or leak it.
- External modules cannot directly borrow private fields such as private `UID`
  fields.
- Helpers that require `&mut UID` may be unreachable from outside the defining
  module unless another public path exposes that reference.
- Check whether capabilities, receipts, witnesses, or owner-only objects can be
  transferred, reused, forged, leaked, or bypassed.

## Object And Asset Semantics

- `key` objects own a `UID`.
- `store` permits public transfer/share/freeze variants.
- Shared mutable objects can become contention points and broad mutation
  surfaces.
- Dynamic fields are keyed under an object UID and require cleanup and key
  uniqueness review.
- `Coin<T>` is an object-level asset; `Balance<T>` is internal value storage.
- Track `split`, `join`, `zero`, `destroy_zero`, `into_balance`,
  `from_balance`, mint, and burn flows.
- Asset findings need accounting evidence: supply, user claims, vault balances,
  receipts, events, and before/after state movement.

## Arithmetic, Oracles, And Upgrades

- Division requires denominator proof.
- Narrowing casts require explicit upper bounds.
- Multiplication/division ordering can introduce premature flooring.
- Oracle-dependent code should validate feed identity, asset binding,
  freshness, confidence, decimals/exponent, and stale or reused inputs.
- Upgradeable shared state should have version checks and migration strategy.

## Finding Discipline

- Hypotheses should state impact if true, required actor, target function,
  evidence path, missing evidence, and validation plan.
- Confirmed findings need a proof path plus test, trace, or state-diff evidence.
- Mitigations should name the exact precondition enforced before mutation:
  capability, owner, phase, amount bound, oracle freshness, version, receipt
  consumption, or dependency validation.

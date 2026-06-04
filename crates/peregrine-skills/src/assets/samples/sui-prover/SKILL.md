---
name: sui-prover
description: Help with bundled Sui Prover formal verification for Move smart contracts in Peregrine. Use when the user wants to run formal verification, write or debug Move specs, target a module/function, interpret prover failures, or translate audit findings into proof obligations.
metadata:
  short-description: Work with bundled Sui Prover specs
---

# Sui Prover

Use this skill to add, run, and debug Sui Prover specifications for Move packages. Peregrine bundles the prover path; do not install `sui-prover` with Homebrew or ask the user to install it as part of this workflow.

For detailed syntax and examples, read [references/spec-reference.md](references/spec-reference.md) when working on complex specs, quantifiers, ghost state, mathematical types, loop invariants, or advanced attributes.

## First Pass

1. Locate the workspace and package root containing `Move.toml`.
2. Check whether specs live in the main package or in a separate specs package.
3. Identify the exact property to prove: abort safety, accounting, authorization, monotonicity, object lifecycle, event emission, or another invariant.
4. Start with one module/function. Avoid broad verification until the focused proof is stable.
5. Run the bundled Peregrine prover path when verification is requested and the package is available.

### Move.toml Setup

The Sui Prover relies on implicit dependencies. Remove any direct dependencies to `Sui` and `MoveStdlib` from `Move.toml`:

```toml
# DELETE this line if present:
Sui = { git = "https://github.com/MystenLabs/sui.git", subdir = "crates/sui-framework/packages/sui-framework", rev = "framework/testnet", override = true }
```

If you need to reference Sui directly, put the specs in a separate package.

## Running Verification

Prefer the Rust harness tool when available:

```text
security_sui_formal_verify({
  "package_path": ".",
  "file_path": "sources/module.move",
  "module_name": "module",
  "timeout_seconds": 60
})
```

For the Peregrine CLI/TUI workflow, run from the workspace or package root:

```bash
peregrine verify --module module --file sources/module.move --timeout-seconds 60
peregrine --project /workspace --package packages/app verify --module module
peregrine check-all --module module --file sources/module.move
```

If a standalone `sui-prover` binary is not on PATH, do not install it. Use the bundled Peregrine verification path above, or draft the spec and tell the user which Peregrine command or harness tool invocation should verify it.

## Package Setup

The prover has its own Sui framework/prover assumptions. If verification fails because framework dependencies conflict, inspect `Move.toml` for direct `Sui` or `MoveStdlib` entries and decide whether to move specs into a separate package or adjust dependencies. Do not casually break normal `sui move build`; preserve the project’s regular build/test path.

Use a separate specs package when prover-only attributes, imports, or dependency changes would make the main package fail normal Move compilation.

## Writing Specs

Use a spec function with the same signature as the target when practical:

```move
#[spec(prove)]
fun transfer_spec(account: &mut Account, amount: u64): bool {
    requires(amount > 0);
    asserts(account.balance.to_int().sub(amount.to_int()).gte(0u64.to_int()));

    let old_account = clone!(account);
    let result = transfer(account, amount);

    ensures(account.balance.to_int().add(amount.to_int()) == old_account.balance.to_int());
    result
}
```

Core functions:

- `requires(condition)`: constrain valid inputs.
- `ensures(condition)`: state postconditions after the call.
- `asserts(condition)`: describe conditions required to avoid aborts.
- `clone!(ref)`: snapshot mutable state before the call.
- `forall!`, `exists!`, `implies`: express quantified or logical obligations.
- `.to_int()` and `.to_real()`: reason in unbounded numeric domains.
- `fresh<T>()`: create an unconstrained spec value.

Spec composition:

- Name specs as `<function>_spec` so the prover can use them as opaque summaries for callers.
- Use `#[spec(prove, focus)]` while debugging, then remove `focus` before final verification.
- Use `#[spec(prove, no_opaque)]` when proving through called implementations is more useful than relying on summaries.
- Use `target = module::function` or `target = 0x2::transfer::public_transfer` for cross-module specs.
- Add `#[spec_only]` helpers or getters when specs need private fields or prover-only imports.

## Abort Obligations

Most failed proofs start as missing abort conditions. Add `asserts` before the function call that can abort.

Common obligations:

- Arithmetic overflow/underflow: compare `.to_int()` calculations against primitive bounds.
- Division: assert the divisor is non-zero.
- Tables and dynamic fields: assert key existence before borrowing.
- Bags: prefer `contains_with_type<K, V>` before `bag::borrow<K, V>`.
- Nested calls: copy required `asserts` from specs of called functions.
- Early returns: guard assertions for code that only runs after the early-return branch.

Prefer `asserts` for implementation abort behavior. Use `requires` only for true caller assumptions.

## Security Proof Targets

Prioritize specs for:

- Asset conservation, supply deltas, and share-price/exchange-rate monotonicity.
- Authorization, signer checks, capabilities, witness types, and upgrade gates.
- Object lifecycle transitions, dynamic field/table consistency, and migration invariants.
- Rounding, overflow, zero-value, max-value, and boundary behavior.
- Event emission and transfer side effects via ghost variables.
- Public/entry functions that mutate shared objects or critical balances.

## Debugging Failures

1. Add `focus` to the spec under development.
2. Run bundled verification for the one module/function.
3. If the output says code aborts, add missing `asserts` for the target and nested calls.
4. If an assertion does not hold, fix the condition or add intermediate `ensures` to expose the missing relationship.
5. If it times out, simplify the spec, increase timeout, split paths, or add `boogie_opt=b"vcsSplitOnEveryAssert"` to complex specs.
6. Remove `focus` and re-run the relevant suite before finishing.

Useful Peregrine flags:

```bash
peregrine verify --module module --file sources/module.move --timeout-seconds 120
peregrine verify --module module --trace --keep-temp
```

## Output

When changing specs, report:

- The property being proved.
- The module/function and package path.
- Any `requires` assumptions and why they are valid.
- The abort obligations covered by `asserts`.
- The exact Peregrine command or harness tool invocation used.
- Whether verification passed, failed, timed out, or could not be run locally.

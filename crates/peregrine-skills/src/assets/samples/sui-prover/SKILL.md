---
name: sui-prover
description: Help with Sui Prover formal verification for Move smart contracts. Use when the user wants to run the prover, write specs, debug verification failures, translate audit findings into invariants, or decide what proof obligations to add.
metadata:
  short-description: Work with Sui Prover specs
---

# Sui Prover

Use this skill to help add or debug Sui Prover specifications for Move packages.

## First Pass

1. Locate the package root containing `Move.toml`.
2. Check whether prover specs already exist and whether the project keeps specs in the main package or a separate specs package.
3. Identify the invariant the user wants to prove, or derive one from the security review.
4. Prefer a narrow proof target before broad verification.

## Common Commands

Run from the package root unless the user gives another path:

```bash
sui-prover
sui-prover --path ./path/to/package
sui-prover --verbose --timeout 60
```

If `sui-prover` is missing, explain the missing tool and continue by drafting specs and expected proof obligations.

## Spec Patterns

- Use `#[spec(prove)]` for specs that should be verified.
- Use `requires(...)` for preconditions the caller must satisfy.
- Use `ensures(...)` for postconditions that must hold after the function returns.
- Use `asserts(...)` to document abort behavior.
- Use wider integer reasoning or `.to_int()` for arithmetic obligations where overflow or truncation matters.
- Add focused specs while debugging, then remove focus before finishing.

## Security-Oriented Proofs

Prioritize proofs for:

- Asset conservation and supply changes
- Authorization and capability checks
- Share-price or exchange-rate monotonicity
- Object lifecycle transitions
- No accidental privilege escalation during migrations or upgrades
- Rounding and boundary behavior

## Output

When changing specs, explain the invariant, the function or module under proof, any assumptions added through `requires`, and what command was run to verify it. If verification cannot run locally, state that clearly and provide the exact command to run.

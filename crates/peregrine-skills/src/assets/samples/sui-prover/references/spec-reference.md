# Sui Prover Reference

Use this reference when the main skill is not enough. It summarizes the Sui Prover patterns Peregrine agents should apply while using the bundled prover path.

## Attributes

`#[spec(...)]` marks a specification function.

| Attribute | Use |
| --- | --- |
| `prove` | Verify this spec. |
| `focus` | Verify only focused specs while debugging. Remove before final verification. |
| `skip` | Skip this spec. Avoid using it to hide real failures. |
| `target = path::to::function` | Attach a spec to a function in another module. |
| `include = path::to::spec` | Include another spec contract. |
| `ignore_abort` | Stop checking abort behavior. Use sparingly. |
| `no_opaque` | Inline called implementations instead of only using their specs. |
| `extra_bpl = b"file.bpl"` | Load extra Boogie definitions from a local file. |
| `boogie_opt = b"vcsSplitOnEveryAssert"` | Split complex verification conditions. |

`#[ext(...)]` marks helper behavior:

| Attribute | Use |
| --- | --- |
| `pure` | Deterministic and side-effect-free; usable in specs and quantifiers. |
| `no_abort` | Function is assumed not to abort. |
| `axiom` | Treat the function as axiomatically defined. |

`#[spec_only]` makes imports, helpers, structs, modules, and getters available only to the prover. Use `#[spec_only(inv_target = Type)]` for datatype invariants and `#[spec_only(loop_inv(target = function_spec))]` for external loop invariants.

## Cross-Module Specs

Use `target` when the spec is outside the implementation module:

```move
module specs::vault_spec {
    #[spec(prove, target = vault::withdraw)]
    public fun withdraw_spec(account: &mut Vault, amount: u64): Balance {
        asserts(amount <= account.balance);
        let result = vault::withdraw(account, amount);
        ensures(result.value() == amount);
        result
    }
}
```

When private state is needed, add `#[spec_only]` getters in the implementation module instead of making production APIs public.

## Numeric Reasoning

Use unbounded spec-only numbers for proof conditions.

```move
let total = amount.to_int().mul(price.to_int());
asserts(total.lte(std::u64::max_value!().to_int()));
```

Useful domains:

- `std::integer::Integer`: arbitrary-precision integer via `.to_int()`.
- `std::real::Real`: arbitrary-precision real via `.to_real()`.
- `std::q32`, `std::q64`, `std::q128`: signed fixed-point spec types.

Prefer `.to_int()` for overflow, underflow, multiplication, division, and rounding obligations.

## Quantifiers

Use `forall!` and `exists!` for properties over all values of a type. The lambda argument is a reference, and the body should call a named `#[ext(pure)]` function.

```move
#[ext(pure)]
fun is_authorized(user: &address): bool {
    *user != @0x0
}

#[spec(prove)]
fun authorization_spec() {
    ensures(forall!<address>(|user| is_authorized(user)));
}
```

Do not put inline arithmetic or aborting code inside quantifier lambdas.

## Vector Iterators

Import with `use prover::vector_iter::*`.

| Function | Use |
| --- | --- |
| `all!<T>(&v, |x| pred(x))` | Every element satisfies a predicate. |
| `any!<T>(&v, |x| pred(x))` | At least one element satisfies a predicate. |
| `count!<T>(&v, |x| pred(x))` | Count matching elements. |
| `map!`, `filter!`, `find!` | Transform or locate elements. |
| `sum(&v)` | Sum numeric vector elements. |

Most macros also have `_range!` variants for subranges. `sum` is a function, not a macro.

## Loop Invariants

Use inline invariants directly before loops:

```move
invariant!(|| {
    ensures(i <= n);
    ensures(total == i.to_int().mul(step.to_int()));
});
while (i < n) {
    i = i + 1;
    total = total + step;
};
```

Use external invariants for repeated or bulky invariants:

```move
#[spec_only(loop_inv(target = sum_spec))]
#[ext(no_abort)]
fun sum_loop_inv(i: u64, n: u64, total: u128): bool {
    i <= n && total == (i as u128) * 10
}
```

For multiple loops, add `label = N` with zero-based loop order.

## Ghost State

Use ghost variables to propagate prover-only state such as events or transfer targets.

```move
#[spec_only]
public struct EventSeen {}

#[spec(prove)]
fun emit_spec() {
    ghost::declare_global<EventSeen, bool>();
    emit_event();
    ensures(*ghost::global<EventSeen, bool>());
}
```

For specs involving `transfer::public_transfer`, declare the transfer-address ghost variables expected by the transfer specs before asserting recipient behavior.

## Known Failure Patterns

| Symptom | Likely cause | Response |
| --- | --- | --- |
| Code aborts | Missing abort assertions | Add `asserts` before the call that can abort. |
| Assert does not hold | Assertion does not match implementation | Recheck branch conditions and intermediate state. |
| Timeout | Spec is too broad or solver path is hard | Focus one spec, increase timeout, split paths, or add Boogie split option. |
| Table borrow still aborts | Existence condition is missing or too weak | Add `contains`/typed contains before borrow. |
| Bag borrow not connected | Used `bag::contains` only | Use `bag::contains_with_type<K, V>`. |
| Pure function unavailable | Missing `#[ext(pure)]` | Mark the getter/helper and its called helpers as pure. |
| Missing native pure Boogie function | Native helper lacks a Boogie definition | Add an `extra_bpl` prelude with the missing declaration. |
| UID type lost after destructuring | Known prover limitation | Avoid proving that function directly; prove callers or surrounding invariants. |

## Finalization Checklist

- Remove `focus`.
- Avoid `ignore_abort` unless intentionally justified.
- Keep normal `sui move build` and tests working.
- Run bundled Peregrine verification for the narrow target.
- State any remaining unproved assumptions in the final response.

import type { AgentRole, GuideRef } from "./types";

export const SUI_MOVE_SECURITY_CONTEXT = [
  "Sui/Move security knowledge pack",
  "Use this as a compact guide, not as proof. Current source, compiler output, bytecode, graph evidence, and tests override this guide.",
  "",
  "Doc-first discipline:",
  "- Treat remembered Sui/Move knowledge as potentially stale; verify language and framework claims against bundled docs when evidence is needed.",
  "- When available, use `rust_knowledge_sui_move_search` and `rust_knowledge_sui_move_read` to retrieve exact local docs instead of relying on memory or web links.",
  "- Separate exposed attack surface from confirmed vulnerability. Confirm only with source + graph/tool evidence + test/trace/state-diff impact.",
  "- Do not invent missing authorization or state preconditions. Say which check is present, missing, bypassable, or unresolved.",
  "",
  "Move 2024 visibility and transaction surface:",
  "- `public fun` is programmable-transaction-block composable when its signature can be satisfied; absence of `entry` is not by itself a safety boundary.",
  "- `entry fun` is endpoint-only; `public entry` is generally redundant in modern Sui Move guidance.",
  "- Validate external reachability from visibility, argument types, object ownership, shared object mode, abilities, private fields, and package/module boundaries.",
  "",
  "Objects, capabilities, and ownership:",
  "- Owned objects and capability possession are authorization boundaries; a parameter like `&AdminCap` proves the caller supplied that object.",
  "- External modules cannot directly borrow private fields such as a private `UID`; a helper that needs `&mut UID` may be unreachable unless another public path exposes it.",
  "- `key` objects have `UID`; `store` enables public transfer/share/freeze variants. Track whether capabilities or receipts can be publicly transferred, reused, or leaked.",
  "- Shared objects with `&mut` access can become contention and global-state choke points. Check version fields and phase/state guards on upgradeable shared state.",
  "- Dynamic fields are keyed under an object UID. Check key uniqueness, cleanup on delete, and whether user-controlled keys can overwrite or strand state.",
  "",
  "Assets and accounting:",
  "- `Coin<T>` is an object-level asset; `Balance<T>` is internal value storage. Track `split`, `join`, `zero`, `destroy_zero`, `into_balance`, `from_balance`, mint, and burn flows.",
  "- Returning coins or values preserves PTB composability; internal transfer can hide asset movement and reduce caller control.",
  "- Asset findings need accounting evidence: supply, user claims, vault balances, receipts, events, and before/after state movement.",
  "",
  "Arithmetic and oracle-dependent code:",
  "- Division needs denominator proof. Narrowing casts need explicit upper bounds. Multiplication/division order can create premature flooring or precision loss.",
  "- Validate price feed identity, asset binding, timestamp/freshness, confidence, decimals/exponent, and stale or reused oracle inputs before asset movement.",
  "",
  "Finding patterns to check:",
  "- Access control: unprotected public mutation, ignored boolean auth, missing cap/witness/owner check, checks after mutation.",
  "- Lifecycle: created object not transferred/shared/frozen, delete while referenced, shared init mistakes, leaked capability, reusable receipt.",
  "- State machine: missing phase/version/expiry/unlock/finalization checks and repeated-call accounting breaks.",
  "- External calls: dependency reached from critical paths, unchecked external return values, package/version drift, upgradeable dependency assumptions.",
  "",
  "Model behavior:",
  "- Prefer precise Sui/Move terminology. Do not call `public fun` safe merely because it is not `entry`.",
  "- Mark hypotheses as impact-if-true until validation proves reachability and impact.",
  "- Mitigation must name the exact precondition to enforce before mutation: capability, owner, phase, amount bound, oracle freshness, version, or receipt consumption.",
].join("\n");

const SUI_MOVE_SECURITY_ROLES: readonly AgentRole[] = [
  "securityReview",
  "testGeneration",
  "fuzzCampaign",
  "formalSpec",
  "patch",
  "report",
  "triage",
  "ci",
];

export const SUI_MOVE_SECURITY_GUIDE: GuideRef = {
  id: "sui-move-security-knowledge",
  title: "Sui/Move Security Knowledge",
  summary:
    "Compact Sui Move security context distilled from the bundled Peregrine knowledge corpus and Move review guidance.",
  content: SUI_MOVE_SECURITY_CONTEXT,
};

export function shouldAttachSuiMoveSecurityKnowledge(role: AgentRole, chain?: string) {
  const chainText = (chain ?? "").toLowerCase();

  return (
    SUI_MOVE_SECURITY_ROLES.includes(role)
    && (chainText.includes("sui") || chainText.includes("move"))
  );
}

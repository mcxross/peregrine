# Peregrine Audit Workflow

The Peregrine audit workflow enables you to initiate smart contract audits, approve audit strategies, and leverage specialized agents to verify vulnerabilities and produce final, evidence-backed reports.

## Design Goals

Peregrine’s audit system is not a simplistic "audit this code" prompt. It is a persisted, tool-routed lifecycle designed to rigorously verify smart contract invariants and maintain a verifiable trail of security context. 

Core properties:
- **User-Approved Strategies:** You must approve a model-authored audit plan before execution begins.
- **Persisted State:** The audit run is persisted. You can pause, resume, or review the audit at any time without losing context.
- **Evidence-Backed Findings:** Confirmed vulnerabilities require hard evidence, independent verification, adapter replay, and formal Judge approval. Missing tools are treated as explicit coverage gaps, not silent passes.

## User-Facing Workflow

Initiating an audit is a two-step process to ensure you maintain control over the execution strategy.

### 1. Planning Phase
Start by requesting a plan for your target smart contract:

```text
/audit --plan <target>
```

Peregrine will inspect the target read-only, discover available analysis tools, and formulate an audit strategy. This strategy is tailored to the specific contract and includes planned stages (e.g., attack surface analysis, invariant mapping, dynamic fuzzing).

The planner returns an immutable plan fingerprint and a start command.

### 2. Execution Phase
Review the proposed audit strategy. If it meets your requirements, explicitly start the run using the provided fingerprint:

```text
/audit start <fingerprint>
```

Once running, you can interact with the audit lifecycle using standard commands:
- `/audit list`
- `/audit pause <auditId>`
- `/audit resume <auditId>`
- `/audit report <auditId>`

## The Audit Workspace

Each audit run receives a dedicated, isolated workspace under `$PEREGRINE_HOME/audits/<auditId>/`.

This workspace serves as a resilient, verifiable record of the audit, containing:
- **`input/`**: An immutable snapshot of the target codebase.
- **`artifacts/`**: Agent conclusions and intermediate stage outputs.
- **`evidence/`**: Normalized, JSON-formatted evidence of vulnerabilities.
- **`traces/`**: Execution traces and adapter replay logs.
- **`reports/`**: The final `report.md` and machine-readable `report.json`.

Your original target repository is never modified during the audit.

## Agent Orchestration

Peregrine doesn't rely on a single perspective. It deploys specialized child agents to critically evaluate the protocol from multiple adversarial angles:

- **Researcher:** Maps the attack surface, identifies entry points, and formulates exploit hypotheses.
- **Exploiter:** Conducts dynamic analysis, targeted fuzzing, and attempts to confirm exploit hypotheses via proof-of-concepts.
- **Skeptic:** Actively reviews the findings of the Researcher and Exploiter, aggressively looking for false positives or flawed logic.
- **Judge:** Receives all normalized evidence, execution traces, and public agent conclusions to make the final determination on vulnerability severity and validity.

This division of labor ensures that findings are rigorously challenged before making it into your final report.

## Evidence and Report Validation

Peregrine is built on the principle that *model-submitted assertions are not sufficient to confirm a vulnerability*. 

To transition an identified risk into a confirmed finding in the final report, it must pass a strict validation pipeline:

1. **Evidence Exists:** The finding must reference concrete evidence files stored in the audit workspace.
2. **Independent Verification:** The vulnerability must be verified by at least two independent, non-model methods (e.g., static analysis plus execution replay).
3. **Execution Trace:** There must be a reproducible execution trace or adapter replay confirming the exploit.
4. **Judge Approval:** The Judge agent must publicly conclude that the evidence supports the finding.

If an analysis capability or tool is missing during the audit, Peregrine does not silently fail. It explicitly records a "coverage gap" in the final report, ensuring you always know exactly what was—and wasn't—tested.

---
*Note: Peregrine's core audit lifecycle is blockchain-neutral. Blockchain-specific operations, such as network replay or bytecode analysis, are delegated to chain adapters (like the Sui adapter).*

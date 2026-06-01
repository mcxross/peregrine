---
name: peregrine-security-audit
description: Security review workflow for Peregrine and adjacent Rust, Tauri, MCP, plugin, skill, and Sui tooling code. Use when the user asks for a security audit, threat model, vulnerability review, supply-chain check, secret scan, telemetry/exfiltration review, sandbox review, or auth/config boundary review.
metadata:
  short-description: Audit Peregrine security boundaries
---

# Peregrine Security Audit

Use this skill for code and configuration security reviews. Stay evidence-driven: inspect the repo, cite exact files/lines, and label uncertainty clearly.

## Workflow

1. Establish scope from the user's request. If unspecified, audit changed files first, then broaden to relevant shared crates.
2. Build a quick map of entry points: CLIs, app server APIs, Tauri commands, MCP servers, plugins, skill installers, model providers, config loading, auth, filesystem access, and network clients.
3. Check secrets and credentials:
   - Search for private keys, tokens, API keys, seed phrases, `.env` values, and committed credential material.
   - Check logs, telemetry payloads, panic messages, and error paths for accidental secret disclosure.
4. Check filesystem and sandbox boundaries:
   - Confirm writes target Peregrine-owned directories unless the user explicitly selected another destination.
   - Validate path joins, archive extraction, symlink handling, and user-controlled paths.
   - Confirm destructive operations have explicit scope and do not cross trust boundaries.
5. Check network and telemetry behavior:
   - Identify all outbound calls and what data they send.
   - Flag unexpected OpenAI/Codex endpoints, telemetry submission, and implicit downloads.
   - Prefer paused or opt-in behavior for telemetry and remote installs.
6. Check supply chain:
   - Inspect new dependencies, git sources, patches, build scripts, plugin bundles, and embedded skills.
   - Verify default installed skills are limited to the intended Peregrine set.
7. Check auth and authorization:
   - Review provider auth, ChatGPT/OpenAI assumptions, app-server authentication, extension boundaries, and escalation prompts.
   - Confirm OpenAI-specific auth is guarded by the provider that actually requires it.

## Output

Lead with findings ordered by severity. For each finding include:

- Severity and short title
- File and line reference
- What can go wrong
- Why the current code allows it
- Concrete fix
- Test or verification to add

If no issue is found, say that directly and list any meaningful residual risk or untested surface.

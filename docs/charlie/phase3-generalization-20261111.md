# Charlie Phase 1–3 generalization hardening

This change is specific to the **ItisPhoenix/rakazo** fork. It is not an upstream Rakazo release.

## Proactive work

- Offered built-in tools are filtered against the shared read-only capability catalog and an explicit, bounded local-effect policy; the previous 25-tool copy is removed.
- Connected read-only tools are discovered using their advertised read-only metadata **and** conservative read-action naming checks. Unknown or mutating connector tools fail closed.
- The same check runs at dispatch before a side effect can be claimed, including catalog-resolved and batch tools.
- Agent tools remain subject to owner/Space authorization, execution controls, sandbox boundaries where configured, and existing effect auditing.
- A proactive run may carry out several related, bounded steps, record the remaining work, and resume on a later cycle. The platform's run/tool budgets still apply.

## Environment

Docker instruction prompts do not assume a specific Linux distribution, home directory or installed packages. Bots inspect available commands and privileges at runtime before trying an operation. A model cannot use a shell it was not offered.

## Memory

Honcho now supports an optional per-Space provider setting `recallStrategy`:

- `auto` (default): keep the prior local + low/minimal preference for direct stored-conclusion retrieval; use reasoned chat otherwise.
- `conclusions`: use stored conclusions directly when available, including with a cloud provider. Does not bypass Space isolation.
- `reasoned`: always use Honcho peer-chat reasoning.

When a direct auto search finds no stored facts, reasoned recall is tried. An explicitly configured external user peer always uses reasoned recall. The Honcho API returns stored conclusion identifiers but does not expose numeric relevance confidence; the existing score field is not a measured probability.

## Delegated messaging

- Terminal failures are reported using the structured `result` intent. The gate no longer parses a specific English error prefix.
- On worker restarts, a previously-sent delegation is recovered from persisted message history before user-visible progress decisions.
- The final-message idempotency key and fallback wording are bot-name agnostic.
- Ambiguous recipient names and mismatched ID/name pairs continue to fail closed, with regression coverage.

## Current boundary

The direct-delegation final gate remains **one origin run with its directly requested specialists**. Nested fan-out, changed requirements, multi-wave plans, cancellation, and dependency-aware durable goal graphs require an explicit workflow-ledger design, not an unsafe text prompt or implicit assumption. Do not claim full arbitrary-depth goal orchestration from this patch.

## Release verification

Perform Biome check, TypeScript package checks, focused regression tests, Docker build, and verified rollout with rollback. A clean deployment is not proof of behavior. Before closing Phase 3, use real different natural-language user tasks to verify correct specialist selection, no unintended cross-Space delivery, a single consolidated final response, and live failure/restart recovery. Do not deploy or restart unrelated services.

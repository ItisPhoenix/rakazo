# Autonomous learning and capability evolution

This implementation builds on the existing run, tool, memory and skill systems. It does not introduce a separate mission framework.

## Intended behavior

1. Agents execute the owner's real requests using the capabilities available at runtime. When blocked, they investigate, discover alternatives, check existing skills and verify results rather than repeating a failed action.
2. A completed run that used tools and produced a result can create a durable, silent internal review. Its idempotency key incorporates the completed source run. A review never schedules another review.
3. Internal reviews can examine local skill and memory evidence, capture verified reusable procedures using the existing skill tools, and keep stable long-term knowledge. They cannot execute shells, mutate external accounts, send messages or delete skills.
4. Skill reads, creations, updates and deletions emit source-run-linked scoped evidence events without copying the skill contents into event logs.
5. A review aggregates recent consultations alongside the original runs' terminal statuses, treating the correlation as a hint rather than proof of cause.
6. Every skill update through the application skill API or built-in skill tool archives the prior content as an accessible `history/N.md` reference file. Updates use optimistic revision checks, preserving prior working versions and rejecting conflicts.

## Multi-user behavior

Records retain their original Space, user, bot and run identifiers. Generated skills are created through existing user-owned, space-scoped skill mechanisms. Existing authentication, tenant isolation, approval rules, effect auditing, replay protection and computer boundaries remain enforced. New code does not reference specific agents, hostnames, device paths or model providers.

## Limits still to validate

- A completed run is not evidence of task-level correctness. Review text explicitly avoids treating success labels as proof.
- The candidate has not passed a full build, all regression tests or a live end-to-end evaluation.
- Internal learning consumes model resources and may need owner-configurable budgets and visibility controls.
- Evidence-based skill promotion and automated retirement require broader evaluation. The present version can revise working skill content and retain prior versions but does not silently delete or archive skills.
- Internal reviews may be missed after a crash between source finalization and review scheduling; their run IDs are stable once recorded, and a reconciliation path must be validated before claiming full crash recovery.
- No new executable code is auto-installed or granted permissions by a review.

## Acceptance

Validate with real tasks from more than one agent and Space: obstacle investigation and retry; verified reusable skill creation; skill reuse; correction and rollback; no duplicate or user-visible review messages; restart behavior; no cross-Space access; no unrelated host effects. Build, typecheck and run existing plus new regression tests before deployment.

## Parent PRD

[private-yamux-tunnel-relay](../../prd/private-yamux-tunnel-relay.md)

## What to build

Optional hardening: **stress** or **high-concurrency** scenario—many simultaneous connections or Yamux streams across one or more tunnels—to expose **leaks**, **backpressure**, or **task** issues. Add an automated test or benchmark-gated test per **Testing Decisions** v1+. Fix issues found; keep tests bounded (timeouts) so CI stays reliable.

See **Testing Decisions** (v1+) and **User story** 11 in the parent PRD.

## Acceptance criteria

- [x] Automated test (or gated benchmark) exercises **many concurrent** streams or accepts without hanging CI.
- [x] Documented threshold or loop count; explicit timeouts on all waits.
- [x] Issues found (leaks, errors under load) are fixed or explicitly deferred with rationale in the issue/PR.

## Blocked by

- [Multi-tunnel config, routing, and relay validation](./02-multi-tunnel-config-routing-validation.md)

## User stories addressed

- 11 (concurrency / robustness angle)

## Parent PRD

[private-yamux-tunnel-relay](../../prd/private-yamux-tunnel-relay.md)

## What to build

Wrap or drive the client with a **Ratatui** + **crossterm** full-screen UI: a **four-column** table **Name | Local Addr | Public Domain | Status**, plus **per-tunnel throughput** (bytes in/out) updated from the copy loops. Refresh the UI about **every 100 ms**. Per parent **Testing Decisions**, **no mandatory** Ratatui snapshot tests in v0; optional **unit tests** only if pure **view-model / stats aggregation** is extracted.

See **Solution** and **Implementation Decisions** (TUI) in the parent PRD.

## Acceptance criteria

- [ ] TUI shows all configured tunnels in the four columns with sensible status (e.g. up / error / degraded—definitions may be refined in implementation).
- [ ] Local addr and public domain columns reflect YAML (domain remains informational unless relay validates it).
- [ ] Per-tunnel byte counters move under load and are safe to read from the UI thread/task (atomics or async-safe aggregation).
- [ ] ~100 ms redraw tick; terminal experience acceptable on Mac terminal per user story 24.
- [ ] No requirement for automated full-terminal UI tests unless view logic is extracted and tested in isolation.

## Blocked by

- [Multi-tunnel config, routing, and relay validation](./02-multi-tunnel-config-routing-validation.md)

## User stories addressed

- 14, 15, 16, 17, 18, 24

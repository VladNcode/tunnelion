## Parent PRD

[private-yamux-tunnel-relay](../../prd/private-yamux-tunnel-relay.md)

## What to build

Extend the system to **multiple named tunnels**: client `tunnels.yaml` lists several tunnels; relay configuration maps each **tunnel name** to a **distinct listen port**. **Relay startup must fail** if definitions are inconsistent (e.g. **duplicate listen ports**, missing mappings, or other validation rules you document). Prove **correct routing**: for at least two tunnels, bytes entering relay port A reach local service A and port B reach B, concurrently if applicable. Add **v0.5 integration-style tests** on **127.0.0.1** with one control connection and **two tunnels**, demonstrating **multiplexing** and **correct tunnel association** (Caddy still not in automated tests).

See **Implementation Decisions** (configuration, relay bridging, stream association) and **Testing Decisions** (v0.5) in the parent PRD.

## Acceptance criteria

- [ ] Client supports **multiple** tunnels from a single YAML file; each has `addr` and `domain`.
- [ ] Relay loads explicit **name → listen port** (or equivalent) for every tunnel; **duplicate ports** cause startup failure with a clear error.
- [ ] Concurrent or sequential use: streams for different tunnels reach the correct local `addr` (verified by tests).
- [ ] Integration tests run relay + client on localhost; **no Caddy** in CI; tests use relay tunnel listen ports as the entry point.
- [ ] Invalid client YAML cases remain fail-fast with readable errors (extends story 19).

## Blocked by

- [Single-tunnel Yamux pipe](./01-single-tunnel-yamux-pipe.md)

## User stories addressed

- 1, 2, 11, 19, 20, 21, 22, 29

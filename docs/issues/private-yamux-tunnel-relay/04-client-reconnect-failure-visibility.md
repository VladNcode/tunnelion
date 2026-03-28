## Parent PRD

[private-yamux-tunnel-relay](../../prd/private-yamux-tunnel-relay.md)

## What to build

When the **control TCP connection** drops, the **client** reconnects automatically using a **simple backoff** policy (exact parameters implementation-defined but logged). **Handshake failures** must surface clearly on **stderr** (and TUI status if slice 3 is merged, without duplicating logic unnecessarily). The **relay** continues to emit useful **logs** for handshake and accept errors so VPS operators are not blind. Document behavior briefly for operators.

See **User stories** 9, 13, 25 and **Further Notes** (reconnect policy) in the parent PRD.

## Acceptance criteria

- [x] Client detects control disconnect and retries with backoff; does not tight-loop or spam without limit.
- [x] Failed PSK handshake produces a clear, actionable stderr message on the client; connection is closed cleanly on the relay for bad attempts.
- [x] Relay logs (or stderr) include enough context to debug handshake and accept failures on the VPS.
- [x] Behavior is described in operator-facing notes (README section or comment block—minimal, not a new doc framework unless you already have one).

## Blocked by

- [Single-tunnel Yamux pipe](./01-single-tunnel-yamux-pipe.md)

## User stories addressed

- 9, 13, 25

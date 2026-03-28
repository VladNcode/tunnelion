## Parent PRD

[private-yamux-tunnel-relay](../../prd/private-yamux-tunnel-relay.md)

## What to build

Establish the full vertical path for **one** named tunnel: Cargo workspace with a **protocol** package (PSK handshake + per-stream tunnel identity header), **relay** binary (control listen + Yamux server role + **one** tunnel listen port), and **client** binary that reads **`tunnels.yaml` containing a single tunnel**, connects with **one TCP session**, and bridges **bidirectional** bytes between each new Yamux stream and the local `addr`. No Ratatui yet—use **stderr logging** for status and errors. Include **v0 protocol unit tests** (handshake success/failure, malformed input, header encode/decode) and **serde tests** for valid/invalid YAML samples, plus a **localhost integration test** that connects to the relay’s tunnel port and reaches a local echo or test server **without Caddy**.

See **Solution**, **Implementation Decisions** (workspace, transport stack, stream association, PSK, relay/client bridging, configuration), and **Testing Decisions** (v0 and start of v0.5) in the parent PRD.

## Acceptance criteria

- [ ] Cargo workspace with protocol library + relay + client binaries (layout per parent PRD).
- [ ] PSK handshake completes before Yamux; wrong or malformed handshake fails fast and closes the connection.
- [ ] After Yamux is up, new streams carry agreed **tunnel identity** before application payload (per parent PRD).
- [ ] Relay accepts **one** client control connection and listens on **one** tunnel TCP port; accepted public-side connections become Yamux streams and copy bytes to/from the client.
- [ ] Client parses `tunnels.yaml` with **at least one** tunnel (`addr`, `domain`); invalid YAML fails before networking with a clear error.
- [ ] End-to-end on localhost: traffic sent to relay tunnel port appears at the configured local service and returns (bidirectional).
- [ ] Automated tests cover protocol (v0 scope) and include integration test **without** Caddy edge.

## Blocked by

None — can start immediately.

## User stories addressed

- 3, 4, 5, 7, 8, 9, 10, 11, 12, 19, 21, 22, 23, 25, 27, 28, 29 (local simulation)

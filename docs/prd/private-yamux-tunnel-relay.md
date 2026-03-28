## Problem Statement

Developers who run web services on a Mac (or other local machines) often need those services reachable on stable public HTTPS URLs—for demos, webhooks, or mobile clients—without exposing their home network broadly or paying for a hosted tunnel product they do not control. Existing tools solve this with a hosted relay and a local agent, but the user wants a **self-hosted**, **private** system: a VPS fronts TLS with **Caddy**, while a **Rust relay** and **Rust client** move bytes over a **single long-lived multiplexed connection**, with **named tunnels** driven by **YAML** and visibility via a **local TUI**. Today the `tunnelion` repository is an empty starter crate; there is no protocol, relay, client, or UI yet.

## Solution

Ship a small Rust ecosystem: a **shared protocol** (PSK handshake + logical tunnel identity for multiplexed streams), a **relay binary** for the VPS that accepts one **control** TCP connection from the client (Yamux over TCP), listens on **per-tunnel local ports** that Caddy reverse-proxies to, and bridges each accepted public-side TCP connection to a **virtual stream** on the Yamux session. A **client binary** for the Mac reads **`tunnels.yaml`**, connects once, and for each incoming stream **routes** to the configured **local TCP address** using **`tokio::io::copy`** (or equivalent bidirectional forwarding). A **Ratatui** TUI (via **crossterm**) shows a four-column table—**Name**, **Local Addr**, **Public Domain**, **Status**—plus basic **bytes in/out** per tunnel, refreshed about every **100 ms**. **Caddy** remains the TLS edge; it is configured out-of-band to map each public hostname to the matching relay listen port on the VPS.

## User Stories

1. As a developer, I want to define several named tunnels in one YAML file, so that I can run multiple local services behind one client process.
2. As a developer, I want each tunnel to specify a local port (or address) and a public domain, so that I know how traffic maps end-to-end.
3. As a developer, I want the client to open a single TCP connection to my VPS and multiplex all tunnels over it, so that I avoid many parallel connections and simplify firewall and NAT behavior.
4. As a relay operator, I want the relay to listen on a dedicated control port for the client, so that only the tunnel agent connects there and public traffic never hits the Yamux session directly.
5. As a relay operator, I want each tunnel to correspond to a distinct TCP listen port on the VPS (e.g. 8001, 8002), so that Caddy can reverse-proxy each subdomain to the right upstream.
6. As a relay operator, I want Caddy to terminate HTTPS for my domains, so that browsers get valid TLS without the Rust stack handling certificates for the edge.
7. As a developer, I want a pre-shared key handshake before Yamux starts, so that random scanners cannot open Yamux sessions on my control port.
8. As a security-conscious operator, I want the PSK stored in config or environment on both sides, so that I can rotate it without code changes.
9. As a developer, I want failed handshake to close the connection quickly, so that misuse is obvious in logs and the client can retry with correct credentials.
10. As an end user hitting the public URL, I want HTTP(S) requests to reach my local service transparently, so that the tunnel behaves like a normal reverse proxy path.
11. As a developer, I want new connections from the internet to a tunnel port to create new Yamux streams, so that concurrent requests work like direct TCP to the local service.
12. As a developer, I want bidirectional byte forwarding on each stream, so that WebSockets or long-polling work as on localhost.
13. As a developer, I want the client to reconnect if the control connection drops, so that transient network issues do not require a manual restart (within reason).
14. As a developer, I want clear status in the TUI for each tunnel (e.g. up, degraded, error), so that I can see which public names are live.
15. As a developer, I want the TUI to show local bind targets and public domains side by side, so that I do not confuse port numbers across machines.
16. As a developer, I want per-tunnel throughput counters (bytes in and out), so that I can spot traffic spikes or stuck connections.
17. As a developer, I want the TUI to refresh frequently (about 100 ms), so that status and counters feel responsive during demos.
18. As a developer, I want the TUI to use a simple table layout (four columns), so that information density stays high without nested navigation.
19. As a developer, I want YAML parsing errors to fail fast with readable messages, so that misconfiguration is fixed before any network activity.
20. As a relay operator, I want relay startup to fail if tunnel definitions are inconsistent (e.g. duplicate listen ports or unknown tunnel ids), so that partial misconfiguration does not silently drop traffic.
21. As a developer, I want tunnel **names** in YAML to be the stable identifier on the wire, so that reordering file lines does not break routing.
22. As a developer, I want the relay and client to agree on how a new stream is associated with a tunnel, so that there is no ambiguity when multiple tunnels are active.
23. As a developer, I want async I/O throughout (Tokio), so that many concurrent streams do not block each other.
24. As a Mac user, I want the client to run comfortably in a terminal, so that I can leave it open next to my editor.
25. As an operator, I want logs or stderr output for handshake failures and accept errors, so that VPS debugging does not depend on the Mac TUI alone.
26. As a developer, I want to run the relay as a simple systemd (or similar) service, so that it survives reboots on the VPS.
27. As a developer, I want the system to be “private ngrok-like” without claiming compatibility with ngrok’s protocol, so that expectations stay accurate.
28. As a future maintainer, I want the handshake and framing isolated in a small shared module or crate, so that relay and client stay in sync and tests can target the protocol directly.
29. As a tester, I want to simulate a client and relay locally without Caddy, so that protocol and bridging logic can be validated in CI or dev loops.
30. As a developer, I want out-of-scope features explicitly documented, so that the first release stays shippable and focused.

## Implementation Decisions

- **Workspace layout**: Use a Cargo workspace with separable artifacts: a **protocol** library (handshake + tunnel/stream metadata), a **relay** binary, and a **client** binary (TUI can live in the client crate or a thin `main` that wires Ratatui to a library core). This keeps the protocol stable and testable.
- **Transport stack**: One **TCP** connection from client to relay on the **control port**. After a successful **PSK handshake**, both sides upgrade the socket to a **Yamux** session (roles: server on relay, client on agent). All tunnel traffic uses **Yamux substreams**, not additional TCP connections from client to relay.
- **Associating streams with tunnels**: Define a **small application header** on each new Yamux stream (after open) carrying the **tunnel name** (or fixed-length id derived from name) and version, **before** application bytes flow—so the relay knows which local VPS port family to bridge and the client knows validation succeeded. Alternative of pre-open stream negotiation is acceptable if documented; the PRD assumes **first-frame metadata** on each stream for clarity and testability.
- **PSK handshake**: Use a compact binary handshake (magic, version, nonce/challenge, **HMAC-SHA256** or similar over agreed material with PSK) so that neither side sends the raw PSK on the wire. Exact message layout lives in the protocol module; both sides share **constant-time comparison** for MAC verification.
- **Relay bridging**: For each configured tunnel, the relay **listens on a TCP port** on loopback (or configurable bind address) matching what Caddy will target. On **accept**, open a new Yamux stream toward the client, write tunnel identity, then **split copy** between the accepted socket and the stream (Tokio tasks).
- **Client bridging**: On **incoming Yamux stream**, read tunnel identity, **connect to local `addr`** from YAML (default IPv4 localhost if only a port is given), then bidirectional copy. If local connect fails, reset or close the stream with a defined error path.
- **Configuration**: Client uses **serde** + **serde_yaml** for `tunnels.yaml` with top-level `tunnels` map; each entry has **`addr`** (port or host:port) and **`domain`** (informational for TUI and ops; Caddy mapping remains external). Relay configuration (control listen address, PSK, map of tunnel name → listen port) can be YAML or env-first; exact format is an implementation choice but must be **explicitly documented** in code or operator docs.
- **TUI**: **ratatui** + **crossterm**, full-screen terminal UI with a **four-column** table: Name | Local Addr | Public Domain | Status. Throughput: maintain **atomic or async-safe counters** updated by copy loops, read by the TUI tick (~100 ms).
- **Edge (Caddy)**: **Out of Rust scope** as code, but **in scope** as deployment contract: one site block (or equivalent) per public hostname pointing to `127.0.0.1:<relay-tunnel-port>` on the VPS. TLS issuance and renewal are Caddy’s responsibility.
- **Edition / toolchain**: Stay on the repo’s chosen Rust edition; add dependencies (**tokio**, **yamux**, **serde**, **serde_yaml**, **ratatui**, **crossterm**, crypto for PSK) as needed without pinning exact versions in this PRD.

## Testing Decisions

- **What makes a good test**: Assert **observable behavior**—handshake acceptance/rejection, correct tunnel association on a stream, and **byte-for-byte round-trip** through the stack—without asserting Ratatui layouts, internal task counts, or private struct fields. Prefer **localhost TCP** with **ephemeral ports**, bounded reads, and explicit **timeouts** so failures do not hang CI.

- **v0 priority (ship first)**: Invest most test effort in the **protocol library** only: **PSK handshake** success and failure (wrong key, truncated messages, garbage after magic), **tunnel/stream header** parse errors and valid round-trips, and any **constant-time** or encoding helpers. These tests should run **without** spinning the full relay or client binaries. **Client YAML**: add **serde** tests for valid and invalid `tunnels.yaml` samples (missing fields, wrong types, empty map).

- **v0.5 (immediately after first end-to-end path)**: Add **integration-style** tests that run **relay + client** (or relay + minimal harness peer) on **127.0.0.1**: one control connection, one or two tunnels, **echo or HTTP-shaped** payloads to prove **multiplexing** and **correct tunnel routing**. Caddy stays **out of tests**; tests terminate at relay **listen ports**.

- **v1+ (optional hardening)**: **Stress-ish** tests (many concurrent streams), **reconnect** behavior, and **relay config validation** (duplicate ports). Still avoid snapshot-testing the full TUI.

- **TUI testing**: **No** mandatory automated tests for Ratatui in v0. If **view-model** or **stats aggregation** is extracted into pure functions, unit-test **that layer only** (e.g. counter rollups, status derivation). Defer terminal snapshot and input simulation tests unless regressions justify the cost.

- **Prior art in codebase**: None yet; establish **`#[tokio::test]`** for async I/O and keep **protocol tests** in a dedicated workspace package so CI can run **only that package’s tests** first and gate merges on handshake correctness before integration tests exist.

## Out of Scope

- **Official ngrok protocol** compatibility, dashboards, or hosted coordination.
- **Automatic Caddyfile generation** or ACME integration inside Rust (operators configure Caddy separately).
- **End-to-end TLS inside the tunnel** (trust is PSK + VPS network boundary; optional future work).
- **UDP**, **HTTP/2**-aware proxying, or **QUIC**—TCP byte streams only unless explicitly added later.
- **User authentication** for public URLs (beyond what the local app does); **rate limiting** and **DDoS** protection at the edge.
- **Windows/Linux GUI client** parity; first-class target is **Mac terminal** as stated, though portable Rust is welcome if it does not expand scope.
- **Persistent tunnel registry** or multi-tenant billing.

## Further Notes

- **Operational clarity**: Document for operators the full path: DNS → Caddy (443) → relay port → Yamux → client → local `addr`. Mismatched ports between Caddy and relay are a common failure mode; the TUI’s **Public Domain** column is informational unless paired with relay-side validation.
- **Reconnect policy**: Backoff and max retry behavior should be simple and logged; exact policy can be tuned after the first working end-to-end path.
- **Security**: PSK rotation requires updating relay and client configs together. Consider future **per-device keys** or **mTLS** if the threat model grows.
- **Naming**: The repository name `tunnelion` can remain the product name for binaries and docs unless renamed later.

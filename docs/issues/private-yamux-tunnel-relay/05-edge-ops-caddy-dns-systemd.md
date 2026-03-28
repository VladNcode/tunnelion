## Parent PRD

[private-yamux-tunnel-relay](../../prd/private-yamux-tunnel-relay.md)

## What to build

**Human-in-the-loop** verification of the **deployment contract**: DNS points hostnames at the VPS; **Caddy** terminates **HTTPS** and reverse-proxies each hostname to the correct **relay tunnel listen port** on loopback (or documented bind). Provide a **checked-in example** Caddyfile or snippet matching the multi-tunnel relay config, plus short **operator notes** (DNS, ports, firewall, optional **systemd** unit sketch for the relay). A human confirms **browser HTTPS** works end-to-end for at least two hostnames. This slice does **not** auto-generate Caddy config from Rust (still out of scope per parent PRD).

See **Solution** (Caddy edge), **Out of Scope** (no ACME inside Rust), **User stories** 6 and 26, and **Further Notes** (operational path) in the parent PRD.

## Acceptance criteria

- [x] Example Caddy configuration committed (or linked path documented) aligning subdomain → `127.0.0.1:<relay-port>` per tunnel.
- [x] Operator notes describe the full path: DNS → Caddy (443) → relay port → Yamux → client → local `addr`.
- [x] Notes mention PSK/config alignment between client and relay and where secrets live.
- [x] Human verification recorded (e.g. checklist in issue PR description or notes—no need to automate).
- [x] Optional: example systemd service fragment for the relay binary.

## Blocked by

- [Multi-tunnel config, routing, and relay validation](./02-multi-tunnel-config-routing-validation.md)

## User stories addressed

- 6, 26, 30

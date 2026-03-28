# tunnelion

Private **ngrok-style** tunnel: TLS at the edge (e.g. Caddy); **relay** on a VPS and **client** next to your apps; PSK + **Yamux** on a control TCP connection; named tunnels map public listen ports to local addresses.

This README is the operator overview. Step-by-step setup: **[docs/deploy/guide.md](docs/deploy/guide.md)**.

---

## End-to-end path

A browser opens `https://app.example.com`.

1. **DNS** resolves the name to your VPS public address.
2. **Caddy** (or another reverse proxy) accepts TLS on **443** and forwards plain HTTP to a **loopback port** on the VPS — the same address you put in `relay.yaml` under `tunnels.<name>.listen` (for example `127.0.0.1:18001`).
3. **tunnelion-relay** accepts that TCP connection, opens a **Yamux** stream toward the client, and tags the stream with the tunnel **name** (`app`, `api`, …).
4. **tunnelion-client** receives the stream, looks up that name in `tunnels.yaml`, and pipes bytes to your process on **`addr`** (for example `127.0.0.1:9000`).

The **control** connection (PSK handshake + Yamux session) uses a **different** socket than the per-tunnel listeners. The client’s `relay.addr` must be that **control** host and port (for example `203.0.113.50:9780`), not the HTTP tunnel ports. Many teams keep the control port off the public internet (VPN, SSH tunnel, or firewall allowlist).

---

## Two kinds of port

| Kind                 | Typical use                                                   | In config                                           |
| -------------------- | ------------------------------------------------------------- | --------------------------------------------------- |
| **Control**          | Single long-lived TCP from client to relay; carries Yamux.    | `relay.yaml` → `control.addr`; client `relay.addr`. |
| **Tunnel listeners** | One per tunnel on the VPS; what Caddy (or the internet) hits. | `relay.yaml` → `tunnels.<name>.listen`.             |

Mixing these up (pointing the client at a tunnel listen port, or Caddy at the control port) is a common misconfiguration.

---

## Secrets and naming

- **`TUNNELION_PSK`** must be the **same** on relay and client. It is **not** stored in YAML; treat it like any other deploy secret.
- **Tunnel names** (the keys under `tunnels:`) must match **exactly** between `relay.yaml` and `tunnels.yaml`. A typo shows up as “unknown tunnel” on the client.
- **Public hostnames** (TLS, browser) must match what you configure in Caddy. Those names do not have to match tunnel keys, but your mental model is easier if you keep a table: hostname → tunnel name → listen port → local `addr`.

---

## Keeping relay, client, and Caddy aligned

Editing three files by hand is error-prone. Prefer a **workspace** YAML that lists each tunnel’s relay listen, client upstream, and public host, then generate artifacts with **`tunnelion-config`**:

```bash
tunnelion-config check config/tunnelion.workspace.yaml
tunnelion-config emit all -i config/tunnelion.workspace.yaml \
  --relay-addr VPS_PUBLIC_IP:9780 -o ./config
```

That writes **`relay.yaml`**, **`tunnels.yaml`**, and **`caddy-tunnelion.caddy`** into **`./config`**. Copy or deploy those files as needed. For stdout-only of a single artifact, use **`emit relay`**, **`emit client --relay-addr …`**, or **`emit caddy`** with the same `-i` workspace.

---

## What the processes do when things go wrong

**Client:** If the control session drops or setup fails, it retries with **exponential backoff** (starts at **500 ms**, doubles up to **30 s**). After a **clean** disconnect following a good session, the next wait is **500 ms** again and failure backoff resets. Handshake and connect errors print a short line on **stderr** when running **headless**; with the default TUI, tracing to stderr stays off so the screen does not fill with logs.

**Relay:** After a control session ends, it **accepts the next** control connection. A failed PSK handshake is **logged**, the TCP connection is **closed**, and the relay **keeps running** (it does not exit on a single bad client).

---

## Edge (TLS) and network

- Terminate **HTTPS** in Caddy (or equivalent). The tunnelion binaries do not implement ACME or public TLS for you.
- Open **443** (and any other ports Caddy binds) on the VPS firewall as needed.
- Ensure **Caddy’s `reverse_proxy` targets** are the **tunnel listen** addresses from `relay.yaml`, not the control address.
- Do not point Caddy at the control port unless you deliberately terminate TLS there and forward raw bytes — that is not the default design.

---

## Smoke checklist (production-shaped)

1. Relay running on the VPS with `relay.yaml` and `TUNNELION_PSK`.
2. Caddy (or your proxy) installed; DNS for each hostname points at the VPS.
3. Client running with matching `tunnels.yaml` and the same `TUNNELION_PSK`.
4. Browser: **https://** each hostname returns the app you expect.

For a **localhost** multi-tunnel test without DNS or Caddy, see **[docs/deploy/guide-local-test.md](docs/deploy/guide-local-test.md)**.

---

## More docs

| Doc                                              | Purpose                              |
| ------------------------------------------------ | ------------------------------------ |
| [Quick guide](docs/deploy/guide.md)              | Build, config, run relay and client. |
| [Local testing](docs/deploy/guide-local-test.md) | Two tunnels on one machine.          |
| [PRD](docs/prd/private-yamux-tunnel-relay.md)    | Product background and decisions.    |

Example config in this repo: **`config/`** (`relay.yaml`, `tunnels.yaml`, `tunnelion.workspace.yaml`, `caddy-tunnelion.caddy`).

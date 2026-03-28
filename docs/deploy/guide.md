# tunnelion — quick guide

**Relay** runs on the VPS (public IP). **Client** runs where your apps run (laptop, home server). Same **`TUNNELION_PSK`** everywhere. Tunnel **names** must match across relay config, client config, and Caddy.

**Local multi-tunnel test (no VPS):** [guide-local-test.md](guide-local-test.md)

---

## 1. Build

```bash
cargo build --release -p tunnelion-relay -p tunnelion-client -p tunnelion-config -p tunnelion-demo-server
```

| Binary                  | Role                                                                                 |
| ----------------------- | ------------------------------------------------------------------------------------ |
| `tunnelion-relay`       | VPS                                                                                  |
| `tunnelion-client`      | Your machine                                                                         |
| `tunnelion-config`      | Generate/check workspace → relay / client / Caddy                                    |
| `tunnelion-demo-server` | Optional local upstream: `PORT` + `APP_NAME` required; `/api/time`, `/api/slow` only |

---

## 2. Config (pick one)

### A. One workspace file (recommended)

Keep a single **workspace** YAML (this repo has **`config/tunnelion.workspace.yaml`** as a template).

```bash
cargo run -p tunnelion-config -- check config/tunnelion.workspace.yaml

cargo run -p tunnelion-config -- emit all \
  -i config/tunnelion.workspace.yaml \
  --relay-addr YOUR_VPS_IP:9780 \
  -o ./config
```

That writes **`relay.yaml`**, **`tunnels.yaml`**, and **`caddy-tunnelion.caddy`** into **`./config`** (directory is created if needed). Put **`relay.yaml`** on the VPS, **`tunnels.yaml`** on the client host, merge the Caddy file on the VPS. PSK is **never** in these files — only **`TUNNELION_PSK`**.

To print one file to stdout (scripts): `emit relay`, `emit client --relay-addr …`, or `emit caddy` (same `-i` workspace).

### B. By hand

- VPS: `deploy/relay.yaml.example` → `relay.yaml` (control + per-tunnel `listen`).
- Client: `tunnels.yaml` with `relay.addr` = **`VPS:control_port`** (not the HTTP tunnel ports) and the same tunnel **keys** as relay.

---

## 3. Relay (VPS)

```bash
export TUNNELION_PSK='your-secret'
# optional: export TUNNELION_RELAY_CONFIG=/path/to/relay.yaml
./tunnelion-relay
```

---

## 4. Client

```bash
export TUNNELION_PSK='your-secret'
./tunnelion-client tunnels.yaml
```

- **TUI** (default): quit `q` / Esc / Ctrl+C. Logs to stderr are **off** by default so the UI stays clean.
- **Headless + logs:** `TUNNELION_HEADLESS=1 ./tunnelion-client tunnels.yaml`

---

## 5. HTTPS on the VPS

1. DNS → VPS IP.
2. Caddy terminates TLS; each site `reverse_proxy`s to the **tunnel listen** from `relay.yaml` (see `deploy/Caddyfile.example` or generated Caddy snippet).
3. Do not terminate TLS on the tunnelion **control** port unless you know why.

More: [README](../../README.md) in the repo root.

---

## When something breaks

| Symptom                    | Check                                                                     |
| -------------------------- | ------------------------------------------------------------------------- |
| Handshake / reconnect loop | Same `TUNNELION_PSK`; client `relay.addr` is **control** port, not 1800x. |
| Can’t reach relay          | Firewall allows **control** TCP from client to VPS.                       |
| Wrong app / 404            | Tunnel **names** match; Caddy → correct **tunnel listen** port.           |

---

## systemd (relay on VPS)

See `deploy/tunnelion-relay.service` — adjust user, paths, and `relay.env` for `TUNNELION_PSK`.

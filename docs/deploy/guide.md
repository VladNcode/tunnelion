# tunnelion — quick guide

**Relay** runs on the VPS (public IP). **Client** runs where your apps run (laptop, home server). Same **`TUNNELION_PSK`** everywhere. Tunnel **names** must match across relay config, client config, and Caddy.

**Local multi-tunnel test (no VPS):** [guide-local-test.md](guide-local-test.md)

### Trust model, PSK, and control bind

- The PSK proves both ends know the secret; **traffic on the control TCP connection is not encrypted** by tunnelion. Use a **private path** (VPN, SSH tunnel, trusted LAN) or wrap the TCP session if you need confidentiality on the wire.
- **PSK:** use a long random value, not a reused password. Example: `export TUNNELION_PSK="$(openssl rand -hex 32)"` (run once, copy the value to relay and client environments).
- **`relay.yaml`** often uses `control.addr: 0.0.0.0:9780` on the VPS so clients can reach the relay. **Firewall** that TCP port (allow only your client or VPN egress) in addition to keeping the PSK secret.

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

- VPS: create **`relay.yaml`** with `control.addr` and per-tunnel `listen` entries (see **`config/relay.yaml`** in this repo for shape).
- Client: **`tunnels.yaml`** with `relay.addr` = **`VPS:control_port`** (not the HTTP tunnel ports) and the same tunnel **keys** as relay.

---

## 3. Relay (VPS)

The relay **always** reads **`relay.yaml`** (default filename in the current working directory). Use **`TUNNELION_RELAY_CONFIG`** if the file lives elsewhere.

```bash
export TUNNELION_PSK='your-secret'
cd /path/to/dir/with/relay.yaml   # default file name: relay.yaml in cwd
# or: export TUNNELION_RELAY_CONFIG=/absolute/path/to/relay.yaml
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
2. Caddy terminates TLS; each site `reverse_proxy`s to the **tunnel listen** from `relay.yaml` (see **`config/caddy-tunnelion.caddy`** in this repo, or your generated snippet from **`tunnelion-config emit caddy`**).
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

Use a unit that sets **`WorkingDirectory`** to the folder containing **`relay.yaml`** (or set **`Environment=TUNNELION_RELAY_CONFIG=...`**), and load **`TUNNELION_PSK`** from an **`EnvironmentFile`** (mode `0600`). There is no checked-in sample service file in this repo.

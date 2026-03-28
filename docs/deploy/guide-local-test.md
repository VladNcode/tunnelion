# Local testing (multi-tunnel, one machine)

Use this to verify relay + client on **localhost** with **two tunnels** and **two upstream ports**. No Caddy or DNS.

Overview: [README](../../README.md) in the repo root.

## Ports

| Role                  | Port      | What                              |
| --------------------- | --------- | --------------------------------- |
| Control (Yamux + PSK) | **9780**  | Client `relay.addr` points here.  |
| Tunnel `app` (public) | **18001** | `curl` here → local app upstream. |
| Tunnel `api` (public) | **18002** | `curl` here → local api upstream. |
| Local app upstream    | **9001**  | e.g. `tunnelion-demo-server`      |
| Local api upstream    | **9002**  | second `tunnelion-demo-server`    |

Flow: `curl 127.0.0.1:18001` → relay → Yamux stream `app` → client → `127.0.0.1:9001`.

## Upstream (optional)

Run two demo HTTP servers on **9001** and **9002** (must match `config/tunnels.yaml` `addr` values):

```bash
cargo run --release -p tunnelion-demo-server -- 9001 app
cargo run --release -p tunnelion-demo-server -- 9002 api
```

**`tunnelion-demo-server <PORT> <APP_NAME>`** — both required. Responses are JSON with an **`app`** field set to `APP_NAME`. **`GET /api/time`** → `{ "app", "unix_ms" }`. **`GET /api/slow?ms=…`** → sleeps then `{ "app", "waited_ms", "done" }` (`ms` optional, default 1000, max 120000). Other paths → 404.

## Config

Use the **same** `TUNNELION_PSK` for relay and client.

**Relay** (repo root):

```bash
export TUNNELION_PSK='your-secret' && export TUNNELION_RELAY_CONFIG=./config/relay.yaml && cargo run --release -p tunnelion-relay
```

**Client**:

```bash
export TUNNELION_PSK='your-secret' && TUNNELION_HEADLESS=1 cargo run --release -p tunnelion-client -- ./config/tunnels.yaml
```

## Optional

- **Verbose logs**: `RUST_LOG=info,tunnelion_relay=debug,tunnelion_client=debug` (use headless client; logs will clutter the TUI).

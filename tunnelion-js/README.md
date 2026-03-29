# tunnelion-js

JSON config, WebSocket auth (`TN3V1` + secret), then binary multiplexed streams. **Not** compatible with the Rust tunnelion wire format (PSK + Yamux).

## Docker

```bash
export TUNNELION_SECRET='…'
docker compose up --build # Both relay and the client
docker compose -f compose.relay.yaml up -d --build # relay
docker compose -f compose.client.yaml up -d --build # client
```

### VPS relay

1. Install Docker on the server.
2. Relay JSON: `0.0.0.0` for `control.host` and tunnel listens (see `config/docker.relay.json`).
3. Match `ports:` in `compose.relay.yaml` to that file.
4. Open those ports on the firewall (and cloud SG if any).
5. Set `TUNNELION_SECRET` env.
6. In `tunnelion-js/`: `RELAY_CONFIG=/path/to/relay.json docker compose -f compose.relay.yaml up -d --build`

### Clients

1. Use the **same** `TUNNELION_SECRET` as the relay.
2. In `tunnels.json`, set `relay.url` to `ws://HOST:CONTROL_PORT` (or `wss://…` if TLS terminates in front of the relay).
3. Run with Docker: `TUNNELS_CONFIG=/path/to/tunnels.json docker compose -f compose.client.yaml up -d --build`

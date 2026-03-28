# tunnelion-js

JSON config, WebSocket auth (`TN3V1` + secret), then binary multiplexed streams. **Not** compatible with the Rust tunnelion wire format (PSK + Yamux).

**Requires** Node 18+.

```bash
npm install && npm run build
export TUNNELION_SECRET='…'
npm run relay -- config/relay.json
npm run client -- config/tunnels.json
```

`TUNNELION_RELAY_CONFIG` overrides the relay config path if argv is omitted. Use `wss://` in `relay.url` when the socket is TLS-terminated.

Sample JSON: [`config/relay.json`](config/relay.json), [`config/tunnels.json`](config/tunnels.json).

**Develop:** `npm run fix` (Prettier + ESLint), `npm run lint`, `npm run format:check`.

/**
 * CLI entry: relay (TCP listeners + WebSocket control).
 */

import { loadRelayConfig } from '../config.js';
import { requireSecret } from '../helpers.js';
import { startRelay } from '../relay.js';

function main(): void {
  const path = process.argv[2] ?? process.env.TUNNELION_RELAY_CONFIG ?? 'relay.json';
  const cfg = loadRelayConfig(path);
  startRelay(cfg, requireSecret());
}

main();

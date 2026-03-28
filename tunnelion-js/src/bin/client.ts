/**
 * CLI entry: long-lived tunnel client.
 */

import { runClientForever } from '../client.js';
import { loadClientConfig } from '../config.js';
import { requireSecret } from '../helpers.js';

async function main(): Promise<void> {
  const path = process.argv[2] ?? 'tunnels.json';
  const cfg = loadClientConfig(path);
  await runClientForever(cfg, requireSecret());
}

main().catch(err => {
  console.error(err);
  process.exit(1);
});

import { assert } from "chai";

/** Assert an Anchor transaction failure includes the expected program error code. */
export function expectAnchorError(err: unknown, code: string): void {
  const message = String(err);
  assert.include(message, code, `expected Anchor error ${code}, got: ${message}`);
}

/** Poll until an account exists (local validator startup / airdrop latency). */
export async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

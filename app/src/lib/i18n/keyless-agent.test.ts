import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { KEYLESS_AGENT_ID, KEYLESS_AGENT_NAME } from './backend';

// Vitest runs from app/; the crate is outside the dev server's reach, so it is read from disk.
const mcpSource = readFileSync(resolve(process.cwd(), '../crates/gt-mcp/src/lib.rs'), 'utf8');

/** The built-in agent's id and name are written twice, in Rust and here: they must stay the same. */
describe('the agent without a key', () => {
  it('has_the_same_id_as_the_rust_constant', () => {
    const m = /KEYLESS_AGENT_ID: Uuid = Uuid::from_u128\((0x[0-9a-f_]+)\)/i.exec(mcpSource);
    expect(m, 'the constant in crates/gt-mcp/src/lib.rs').not.toBeNull();
    const hex = m![1].slice(2).replace(/_/g, '').padStart(32, '0');
    const uuid = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
    expect(KEYLESS_AGENT_ID).toBe(uuid);
  });

  it('has_the_same_stored_name_as_the_rust_constant', () => {
    const m = /KEYLESS_AGENT_NAME: &str = "([^"]*)"/.exec(mcpSource);
    expect(m, 'the constant in crates/gt-mcp/src/lib.rs').not.toBeNull();
    expect(KEYLESS_AGENT_NAME).toBe(m![1]);
  });
});

import { describe, expect, it } from 'vitest';
import { runApiContract } from '../api.contract';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';
import { parseGenDate } from './mockParse';
import type { EditCommand } from '../types';
import commandsJson from '../__fixtures__/commands.json';
import { commandFixtures } from '../__fixtures__/commands';

runApiContract(async () => new MockApi(fixtureTree(500)));

describe('fixtureTree', () => {
  it('is_deterministic', () => {
    const a = fixtureTree(200, 7);
    const b = fixtureTree(200, 7);
    expect(a.persons.map((p) => p.id)).toEqual(b.persons.map((p) => p.id));
    expect(fixtureTree(200, 8).persons[3].id).not.toBe(a.persons[3].id);
  });

  it('has_exact_count_and_special_cases', () => {
    const t = fixtureTree(500);
    expect(t.persons).toHaveLength(500);
    const partners = new Map<string, number>();
    for (const f of t.families)
      for (const id of [f.partner1, f.partner2]) if (id) partners.set(id, (partners.get(id) ?? 0) + 1);
    expect(Math.max(...partners.values())).toBeGreaterThanOrEqual(12);
    expect(t.specials.cycleA).toBeTruthy();
    expect(t.specials.undated).toBeTruthy();
    const scripts = t.persons.filter((p) => /[Ѐ-ӿ֐-׿一-鿿]/.test(p.names[0].display));
    expect(scripts.length).toBeGreaterThan(500 * 0.04);
  });

  it('builds_5000_quickly', () => {
    const t0 = Date.now();
    expect(fixtureTree(5000).persons).toHaveLength(5000);
    expect(Date.now() - t0).toBeLessThan(3000);
  });
});

describe('parseGenDate', () => {
  it('handles_ranges_and_qualifiers', () => {
    const bet = parseGenDate('between 1850 and 1860', 'dmy');
    expect(bet.status).toBe('ok');
    expect(bet.display).toBe('between 1850 and 1860');
    expect(bet.sortMin!).toBeLessThan(bet.sortMax!);
    expect(parseGenDate('aft 1850', 'dmy').display).toBe('after 1850');
    expect(parseGenDate('31 Feb 1900', 'dmy').status).toBe('unparsed');
  });

  it('warns_on_ambiguous_numeric_dates', () => {
    expect(parseGenDate('03/04/1901', 'dmy').warnings.join(' ')).toMatch(/day first/);
    expect(parseGenDate('13/04/1901', 'dmy').warnings).toEqual([]);
  });
});

describe('EditCommand fixtures', () => {
  it('commands_json_has_one_object_per_variant_and_matches_ts', () => {
    expect(commandsJson).toEqual(JSON.parse(JSON.stringify(commandFixtures)));
    const types = new Set((commandFixtures as EditCommand[]).map((c) => c.type));
    expect(types.size).toBe(commandFixtures.length);
    expect(types.size).toBe(26);
  });
});

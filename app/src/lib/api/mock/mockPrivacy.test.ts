import { describe, expect, it } from 'vitest';
import { PrivacyMockApi } from './mockPrivacy';
import { fixtureTree } from './fixtures';

const opts = (living: 'exclude' | 'placeholder' | 'all', includePrivate = false) => ({ version: '7.0' as const, living, includePrivate });

describe('PrivacyMockApi counts follow gt_core::privacy', () => {
  it('modes_count_hidden_and_placeholders', async () => {
    const api = new PrivacyMockApi(fixtureTree(80));
    const ph = await api.previewExport(opts('placeholder'));
    const ex = await api.previewExport(opts('exclude'));
    const all = await api.previewExport(opts('all'));
    expect(ph.hiddenPersons).toBeGreaterThanOrEqual(1); // the undated person is living (fail-safe)
    expect(ph.placeholderPersons).toBe(ph.hiddenPersons);
    expect(ex.hiddenPersons).toBe(ph.hiddenPersons);
    expect(ex.placeholderPersons).toBe(0);
    expect(all.hiddenPersons).toBe(0);
    expect(all.placeholderPersons).toBe(0);
  });

  it('private_records_removed_unless_included', async () => {
    const api = new PrivacyMockApi(fixtureTree(200));
    const without = await api.previewExport(opts('all', false));
    const withPriv = await api.previewExport(opts('all', true));
    expect(without.removedRecords).toBeGreaterThan(0);
    expect(withPriv.removedRecords).toBe(0);
    expect(withPriv.exportedPersons).toBeGreaterThan(without.exportedPersons);
  });

  it('token_is_accepted_by_exportGedcom', async () => {
    const api = new PrivacyMockApi(fixtureTree(40));
    const p = await api.previewExport(opts('placeholder'));
    await expect(api.exportGedcom(p.options, 'C:/x.ged', p.token)).resolves.toHaveProperty('bytes');
  });
});

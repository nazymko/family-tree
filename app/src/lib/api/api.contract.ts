import { describe, expect, it } from 'vitest';
import type { Api, ApiError, PersonQuery } from './api';

export const baseQuery: PersonQuery = {
  sort: 'surname',
  desc: false,
  text: '',
  living: 'any',
  noSource: false,
  noBirthDate: false,
  privateOnly: false,
};

async function rejection(p: Promise<unknown>): Promise<ApiError> {
  try {
    await p;
  } catch (e) {
    return e as ApiError;
  }
  throw new Error('expected the promise to reject');
}

/**
 * Behaviour every Api implementation must show. MockApi runs it in Vitest; the Tauri adapter
 * runs the same suite against mocked IPC (Task 27). `make` must return a fresh api over a
 * 500-person tree that contains a person with a diacritic surname "Müller".
 */
export function runApiContract(make: () => Promise<Api>): void {
  describe('Api contract', () => {
    it('list_paging', async () => {
      const api = await make();
      const first = await api.listPersons(baseQuery, 0, 200);
      expect(first.total).toBe(500);
      expect(first.items).toHaveLength(200);
      expect(first.offset).toBe(0);
      const last = await api.listPersons(baseQuery, 400, 200);
      expect(last.items).toHaveLength(100);
      const clamped = await api.listPersons(baseQuery, 0, 5000);
      expect(clamped.items).toHaveLength(200);
    });

    it('list_sort_surname', async () => {
      const api = await make();
      const page = await api.listPersons(baseQuery, 0, 200);
      for (let i = 1; i < page.items.length; i++) {
        expect(page.items[i - 1].surname.localeCompare(page.items[i].surname, 'en')).toBeLessThanOrEqual(0);
      }
      const desc = await api.listPersons({ ...baseQuery, desc: true }, 0, 200);
      for (let i = 1; i < desc.items.length; i++) {
        expect(desc.items[i - 1].surname.localeCompare(desc.items[i].surname, 'en')).toBeGreaterThanOrEqual(0);
      }
    });

    it('list_filter_living', async () => {
      const api = await make();
      const page = await api.listPersons({ ...baseQuery, living: 'living' }, 0, 200);
      expect(page.total).toBeGreaterThan(0);
      expect(page.items.every((p) => p.living === true)).toBe(true);
      const dead = await api.listPersons({ ...baseQuery, living: 'deceased' }, 0, 200);
      expect(dead.items.every((p) => p.living === false)).toBe(true);
      expect(page.total + dead.total).toBe(500);
    });

    it('list_filters_text_nosource_nobirth_private', async () => {
      const api = await make();
      const text = await api.listPersons({ ...baseQuery, text: 'muller' }, 0, 200);
      expect(text.total).toBeGreaterThan(0);
      const noSource = await api.listPersons({ ...baseQuery, noSource: true }, 0, 200);
      expect(noSource.total).toBeGreaterThan(0);
      expect(noSource.items.every((p) => p.sourceCount === 0)).toBe(true);
      const noBirth = await api.listPersons({ ...baseQuery, noBirthDate: true }, 0, 200);
      expect(noBirth.total).toBeGreaterThan(0);
      expect(noBirth.items.every((p) => p.birth === '')).toBe(true);
      const priv = await api.listPersons({ ...baseQuery, privateOnly: true }, 0, 200);
      expect(priv.total).toBeGreaterThan(0);
      expect(priv.items.every((p) => p.private)).toBe(true);
    });

    it('letters_cover_total', async () => {
      const api = await make();
      const page = await api.listPersons(baseQuery, 0, 200);
      const letters = page.letters;
      expect(letters).toBeDefined();
      expect(letters!.length).toBeGreaterThan(3);
      expect(letters![0].offset).toBe(0);
      for (let i = 1; i < letters!.length; i++) {
        expect(letters![i].offset).toBeGreaterThan(letters![i - 1].offset);
      }
      expect(letters![letters!.length - 1].offset).toBeLessThan(page.total);
    });

    it('family_view_roundtrip', async () => {
      const api = await make();
      const page = await api.listPersons(baseQuery, 0, 50);
      for (const row of page.items.slice(0, 10)) {
        const view = await api.getFamilyView(row.id);
        expect(view.focus.id).toBe(row.id);
        const detail = await api.getPerson(row.id);
        expect(detail.ref.id).toBe(row.id);
      }
    });

    it('get_family_matches_partnership', async () => {
      const api = await make();
      const page = await api.listPersons(baseQuery, 0, 200);
      for (const row of page.items) {
        const p = await api.getPerson(row.id);
        const ps = p.partnerships.find((x) => x.children.length > 0);
        if (!ps) continue;
        const fam = await api.getFamily(ps.familyId);
        expect(fam.id).toBe(ps.familyId);
        expect(fam.partners.map((x) => x?.id)).toContain(row.id);
        expect(fam.children.map((c) => c.person.id)).toEqual(ps.children.map((c) => c.person.id));
        expect(fam.kind).toBe(ps.kind);
        return;
      }
      throw new Error('no family with children in the fixture');
    });

    it('apply_then_undo', async () => {
      const api = await make();
      const before = await api.listPersons(baseQuery, 0, 1);
      expect(before.total).toBe(500);
      const res = await api.apply({
        type: 'createPerson',
        draft: { given: 'Ada', surnames: ['Zimmermann'], sex: 'female' },
      });
      expect(res.label).toBe('Add person');
      expect(res.focus).toBeTruthy();
      expect(res.undo.canUndo).toBe(true);
      expect((await api.listPersons(baseQuery, 0, 1)).total).toBe(501);
      expect((await api.undoState()).canRedo).toBe(false);

      const undone = await api.undo();
      expect(undone).not.toBeNull();
      expect(undone!.undo.canRedo).toBe(true);
      expect((await api.listPersons(baseQuery, 0, 1)).total).toBe(500);
      expect((await api.undoState()).canRedo).toBe(true);
      expect((await api.undoState()).canUndo).toBe(false);

      const redone = await api.redo();
      expect(redone).not.toBeNull();
      expect((await api.listPersons(baseQuery, 0, 1)).total).toBe(501);
      expect((await api.undoState()).canRedo).toBe(false);
    });

    it('undo_with_nothing_returns_null', async () => {
      const api = await make();
      expect(await api.undo()).toBeNull();
      expect(await api.redo()).toBeNull();
    });

    it('edit_clears_redo', async () => {
      const api = await make();
      const draft = { given: 'A', surnames: ['B'], sex: 'unknown' as const };
      await api.apply({ type: 'createPerson', draft });
      await api.undo();
      expect((await api.undoState()).canRedo).toBe(true);
      await api.apply({ type: 'createPerson', draft });
      expect((await api.undoState()).canRedo).toBe(false);
      expect(await api.redo()).toBeNull();
    });

    it('quick_add_child_is_one_undo_step', async () => {
      const api = await make();
      const anchor = (await api.listPersons(baseQuery, 0, 1)).items[0];
      const res = await api.apply({
        type: 'quickAdd',
        anchor: anchor.id,
        relation: 'child',
        draft: { given: 'Kid', surnames: ['Kidson'], sex: 'male' },
      });
      expect(res.label).toBe('Add child');
      expect((await api.listPersons(baseQuery, 0, 1)).total).toBe(501);
      await api.undo();
      expect((await api.listPersons(baseQuery, 0, 1)).total).toBe(500);
    });

    it('save_event_keeps_unparsed_date_verbatim', async () => {
      const api = await make();
      const p = (await api.listPersons(baseQuery, 0, 1)).items[0];
      await api.apply({
        type: 'saveEvent',
        owner: { kind: 'person', id: p.id },
        event: { kind: 'residence', dateRaw: 'sometime in the 1850s?', negative: false, private: false, preferred: false },
      });
      const detail = await api.getPerson(p.id);
      const ev = detail.events.find((e) => e.kind === 'residence');
      expect(ev?.date?.raw).toBe('sometime in the 1850s?');
      expect(ev?.date?.status).toBe('unparsed');
    });

    it('delete_person_removes_row', async () => {
      const api = await make();
      const p = (await api.listPersons(baseQuery, 0, 1)).items[0];
      await api.apply({ type: 'deletePerson', id: p.id });
      expect((await api.listPersons(baseQuery, 0, 1)).total).toBe(499);
      expect((await rejection(api.getPerson(p.id))).code).toBe('not_found');
      await api.undo();
      expect((await api.getPerson(p.id)).ref.id).toBe(p.id);
    });

    it('parse_date_dmy_vs_mdy', async () => {
      const api = await make();
      const dmy = await api.parseDate('03/04/1901', 'dmy');
      expect(dmy.understoodAs).toContain('3 Apr 1901');
      expect(dmy.status).toBe('ok');
      const mdy = await api.parseDate('03/04/1901', 'mdy');
      expect(mdy.understoodAs).toContain('4 Mar 1901');
    });

    it('parse_date_forms', async () => {
      const api = await make();
      expect((await api.parseDate('abt 1850', 'dmy')).understoodAs).toMatch(/1850/);
      expect((await api.parseDate('bef 1850', 'dmy')).status).toBe('ok');
      expect((await api.parseDate('between 1850 and 1860', 'dmy')).range).not.toBeNull();
      expect((await api.parseDate('(spring of the great flood)', 'dmy')).status).toBe('phrase');
      expect((await api.parseDate('12 Mar 1888', 'dmy')).understoodAs).toContain('12 Mar 1888');
    });

    it('parse_date_unparsed_keeps_raw', async () => {
      const api = await make();
      const r = await api.parseDate('sometime after the war', 'dmy');
      expect(r.status).toBe('unparsed');
      expect(r.raw).toBe('sometime after the war');
    });

    it('search_diacritics', async () => {
      const api = await make();
      const hits = await api.search('muller', 50);
      expect(hits.some((h) => h.kind === 'person' && h.label.includes('Müller'))).toBe(true);
    });

    it('search_limit', async () => {
      const api = await make();
      expect((await api.search('a', 5)).length).toBeLessThanOrEqual(5);
      expect((await api.search('a', 500)).length).toBeLessThanOrEqual(50);
      expect(await api.search('', 10)).toEqual([]);
    });

    it('chart_window_returns_only_visible', async () => {
      const api = await make();
      const root = (await api.listPersons(baseQuery, 0, 1)).items[0];
      const meta = await api.chartOpen({ kind: 'descendants', root: root.id, generations: 4 });
      expect(meta.nodeCount).toBeGreaterThan(0);
      const rect = { x: 0, y: 0, w: 500, h: 300 };
      const win = await api.chartWindow(meta.chartId, rect, 1);
      expect(win.nodes.length).toBeGreaterThan(0);
      for (const n of win.nodes) {
        expect(n.x < rect.x + rect.w && n.x + n.w > rect.x && n.y < rect.y + rect.h && n.y + n.h > rect.y).toBe(true);
      }
      const outline = await api.chartOutline(meta.chartId, null, 50);
      expect(outline.length).toBeGreaterThan(0);
      await api.chartClose(meta.chartId);
      expect((await rejection(api.chartWindow(meta.chartId, rect, 1))).code).toBe('not_found');
    });

    it('pedigree_cycle_terminates', async () => {
      const api = await make();
      const hits = await api.search('Cycle', 5);
      const a = hits.find((h) => h.kind === 'person');
      expect(a).toBeDefined();
      const meta = await api.chartOpen({ kind: 'pedigree', root: a!.id, generations: 12 });
      expect(Number.isFinite(meta.nodeCount)).toBe(true);
      expect(meta.nodeCount).toBeLessThan(200);
    });

    it('unknown_id_rejects', async () => {
      const api = await make();
      expect((await rejection(api.getPerson('x'))).code).toBe('not_found');
      expect((await rejection(api.getFamilyView('x'))).code).toBe('not_found');
      expect((await rejection(api.getFamily('x'))).code).toBe('not_found');
      expect((await rejection(api.apply({ type: 'deletePerson', id: 'x' }))).code).toBe('not_found');
    });

    it('change_log_records_the_owner_edit_as_you', async () => {
      const api = await make();
      expect(await api.changeBatches()).toEqual([]);
      expect(await api.unreviewedPeople()).toEqual([]);
      const r = await api.apply({ type: 'createPerson', draft: { given: 'Ada', surnames: ['Koval'], sex: 'female' } });
      const page = await api.changeLog({ unreviewedOnly: false }, 0, 10);
      expect(page.total).toBe(1);
      expect(page.items[0]).toMatchObject({ actor: { kind: 'user' }, label: 'Add person', reverted: false, canUndo: false });
      expect(page.items[0].people.map((p) => p.id)).toContain(r.focus);
      expect((await api.changeLog({ unreviewedOnly: true }, 0, 10)).total).toBe(0);
      expect((await api.personAgentChanges(r.focus!, 0, 20)).items).toEqual([]);
      expect(await api.hasUntrackedChanges()).toBe(false);
      expect(await api.recordHome('note', r.focus!)).toBeNull();
      const history = await api.personHistory(r.focus!, 0, 20);
      expect(history.total).toBe(1);
      expect(history.items[0].group).toBe(page.items[0].group);
      expect((await api.personHistory(r.focus!, 1, 20)).items).toEqual([]);
      expect((await rejection(api.undoChangeGroup(page.items[0].group))).code).toBe('invalid');
    });

    it('name_change_says_which_field_went_from_what_to_what', async () => {
      const api = await make();
      const r = await api.apply({ type: 'createPerson', draft: { given: 'Ivan', surnames: ['Koval'], sex: 'male' } });
      const id = r.focus!;
      const name = (await api.getPerson(id)).names[0];
      const input = { id: name.id, kind: 'birth' as const, preferred: true, private: false, given: 'Ivan', surnames: ['Kovalchuk'] };
      await api.apply({ type: 'saveName', person: id, name: input });
      const surnames = { field: 'surnames', before: 'Koval', after: 'Kovalchuk' };
      const h = await api.history(id, 20);
      expect(h[0].fields).toEqual([surnames]);
      expect(h.slice(1).every((e) => e.fields.length === 0)).toBe(true);
      const row = (await api.changeLog({ unreviewedOnly: false }, 0, 10)).items[0];
      expect(row.changes.find((c) => c.kind === 'person')?.fields).toEqual([surnames]);
    });

    it('change_log_unknown_ids_reject', async () => {
      const api = await make();
      expect((await rejection(api.undoChangeGroup('x'))).code).toBe('not_found');
      expect((await rejection(api.undoChangeBatch('x'))).code).toBe('not_found');
    });

    it('settings_roundtrip', async () => {
      const api = await make();
      const s = await api.getSettings();
      expect(s.dayOrder).toBe('dmy');
      const next = await api.setSettings({ dayOrder: 'mdy', theme: 'dark' });
      expect(next.dayOrder).toBe('mdy');
      expect((await api.getSettings()).theme).toBe('dark');
    });

    it('app_info_names_a_release_and_the_build', async () => {
      const api = await make();
      const info = await api.appInfo();
      expect(info.base).toMatch(/^\d+\.\d+\.\d+/);
      expect(info.version.startsWith(info.base)).toBe(true);
      // a build exactly on a release tag shows the bare version
      if (info.release) expect(info.version).toBe(info.base);
    });

    it('only_project_links_open', async () => {
      const api = await make();
      expect(await api.openProjectLink('https://example.com/')).toBe(false);
      expect(await api.openProjectLink('file:///C:/Windows/notepad.exe')).toBe(false);
    });

    // --- Agent access (Task 7 defines this contract; Task 6 implements the Rust commands) ---
    it('agent_access_is_off_by_default', async () => {
      const api = await make();
      const s = await api.agentAccessState();
      expect(s).toEqual({
        enabled: false,
        port: 8765,
        running: false,
        error: null,
        requireKey: false,
        keylessCommand: 'claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp',
        connections: [],
      });
    });

    it('agent_access_does_not_require_a_key_until_asked', async () => {
      const api = await make();
      const on = await api.agentAccessRequireKey(true);
      expect(on).toMatchObject({ requireKey: true, keylessCommand: null, enabled: false });
      expect(await api.agentAccessState()).toEqual(on);
      const off = await api.agentAccessRequireKey(false);
      expect(off.requireKey).toBe(false);
      expect(off.keylessCommand).toContain('http://127.0.0.1:8765/mcp');
      expect(off.keylessCommand).not.toContain('Bearer');
    });

    it('a_connection_cannot_take_the_name_of_the_agent_without_a_key', async () => {
      const api = await make();
      for (const name of ['Agent without a key', '  AGENT  without\ta key ', 'Агент без ключа', ' агент   БЕЗ ключа ']) {
        await expect(api.agentConnectionCreate(name)).rejects.toBeTruthy();
      }
      await expect(api.agentConnectionCreate('Agent without a key 2')).resolves.toBeTruthy();
    });

    it('agent_access_set_starts_and_stops', async () => {
      const api = await make();
      const on = await api.agentAccessSet(true, 9000);
      expect(on).toMatchObject({ enabled: true, port: 9000, running: true, error: null });
      expect(await api.agentAccessState()).toEqual(on);
      const off = await api.agentAccessSet(false, 9000);
      expect(off).toMatchObject({ enabled: false, port: 9000, running: false });
    });

    it('agent_connection_key_is_shown_once', async () => {
      const api = await make();
      const made = await api.agentConnectionCreate('Claude on the laptop');
      expect(made.connection.name).toBe('Claude on the laptop');
      expect(made.key).toMatch(/^gt_[0-9a-f]{32,}$/);
      expect(made.snippet).toBe(
        `claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp --header "Authorization: Bearer ${made.key}"`,
      );
      const s = await api.agentAccessState();
      expect(s.connections.map((c) => c.id)).toEqual([made.connection.id]);
      expect(JSON.stringify(s)).not.toContain(made.key);
      await api.agentConnectionRevoke(made.connection.id);
      expect((await api.agentAccessState()).connections).toEqual([]);
    });

    it('agent_rules_are_text', async () => {
      const api = await make();
      expect((await api.agentRules()).trim().length).toBeGreaterThan(20);
    });
    // --- end of Agent access ---
    it('privacy_preview_token_changes_with_options', async () => {
      const api = await make();
      const a = await api.previewExport({ version: '7.0', living: 'placeholder', includePrivate: false });
      const b = await api.previewExport({ version: '7.0', living: 'exclude', includePrivate: false });
      expect(a.token).not.toBe(b.token);
      expect(a.exportedPersons + a.hiddenPersons).toBeGreaterThan(0);
      const out = await api.exportGedcom(a.options, 'out.ged', a.token);
      expect(out.bytes).toBeGreaterThan(0);
      expect((await rejection(api.exportGedcom(a.options, 'out.ged', 'stale'))).code).toBe('stale');
    });
  });
}

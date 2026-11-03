import { describe, expect, it } from 'vitest';
import type * as G from './generated';
import type * as T from './types';
import { MockApi } from './mock/MockApi';
import { fixtureTree } from './mock/fixtures';

// Compile-time checks (svelte-check): sample values produced by the mock must be assignable to the types the
// Rust side generates, and the hand-written re-exports must accept the same values. A shape drift between the
// mock/UI and Rust fails `npm run check`.
describe('generated DTOs match what the UI and mock use', () => {
  it('mock_values_are_assignable_to_generated_types', async () => {
    const api = new MockApi(fixtureTree(60));
    const q = { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false } as const satisfies G.PersonQuery;
    const page = (await api.listPersons(q, 0, 5)) satisfies G.Page<G.PersonRow>;
    const row = page.items[0] satisfies G.PersonRow;
    const detail = (await api.getPerson(row.id)) satisfies G.PersonDetail;
    const view = (await api.getFamilyView(row.id)) satisfies G.FamilyViewDto;
    const settings = (await api.getSettings()) satisfies G.Settings;
    const undo = (await api.undoState()) satisfies G.UndoState;
    const sources = (await api.suggestSources('', 3)) satisfies G.SourceDto[];
    const hits = (await api.search('a', 5)) satisfies G.SearchHitDto[];
    const places = (await api.suggestPlaces('', 3)) satisfies G.PlaceOption[];
    const date = (await api.parseDate('1 Jan 1900', 'dmy')) satisfies G.DatePreview;
    const preview = (await api.previewExport({ version: '7.0', living: 'exclude', includePrivate: false })) satisfies G.PrivacyPreview;
    const relink = (await api.relinkMedia()) satisfies G.RelinkResult;
    const meta = (await api.chartOpen({ kind: 'pedigree', root: row.id, generations: 3 })) satisfies G.ChartMeta;
    const win = (await api.chartWindow(meta.chartId, { x: 0, y: 0, w: 500, h: 500 }, 1)) satisfies G.ChartWindow;
    const outline = (await api.chartOutline(meta.chartId, null, 5)) satisfies G.OutlineItem[];
    const edit = (await api.apply({ type: 'updatePerson', id: row.id, private: false })) satisfies G.EditResult;
    const history = (await api.history(row.id, 5)) satisfies G.HistoryEntry[];
    const project = (await api.openProject('C:/p')) satisfies G.ProjectInfo;
    // The hand-written names are the generated types.
    const asHand: [T.PersonDetail, T.Settings, T.ProjectInfo, T.EditResult] = [detail, settings, project, edit];
    const cmd: G.EditCommand = { type: 'addCitation', target: { kind: 'person', id: row.id }, citation: { reuseLast: true } };
    const handCmd: T.EditCommand = cmd;
    expect([view, undo, sources, hits, places, date, preview, relink, win, outline, history, asHand, handCmd]).toBeDefined();
  });
});

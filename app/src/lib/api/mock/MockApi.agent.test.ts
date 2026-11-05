import { describe, expect, it, vi } from 'vitest';
import { MockApi } from './MockApi';
import { MockPlatform } from './MockPlatform';
import { fixtureTree } from './fixtures';
import type { PersonQuery } from '../types';

const ALL: PersonQuery = { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false };
const fixture = fixtureTree(30);

async function make() {
  const api = new MockApi(fixture);
  const platform = new MockPlatform();
  const people = (await api.listPersons(ALL, 0, 3)).items;
  return { api, platform, people };
}

describe('MockApi agent writes', () => {
  it('fires_the_notification_with_touched_and_agent', async () => {
    const { api, platform, people } = await make();
    const heard = vi.fn();
    const off = platform.onTreeChangedByAgent(heard);
    const r = platform.simulateAgentBatch(api, { agent: 'Reader', changes: [{ person: people[0].id, note: 'Hi' }] });
    expect(heard).toHaveBeenCalledWith({ touched: r.touched, agent: 'Reader' });
    off();
    platform.simulateAgentBatch(api, { agent: 'Reader', changes: [{ person: people[0].id, note: 'Again' }] });
    expect(heard).toHaveBeenCalledTimes(1);
  });

  it('writes_the_fact_and_leaves_the_undo_stack_alone', async () => {
    const { api, platform, people } = await make();
    platform.simulateAgentBatch(api, { changes: [{ person: people[0].id, note: 'Hi' }] });
    expect((await api.getPerson(people[0].id)).notes.map((n) => n.text)).toContain('Hi');
    expect(await api.undoState()).toMatchObject({ canUndo: false, canRedo: false });
    expect(await api.unreviewedPeople()).toEqual([people[0].id]);
  });

  it('undo_group_removes_only_that_fact_and_is_logged_as_you', async () => {
    const { api, platform, people } = await make();
    const { groups } = platform.simulateAgentBatch(api, {
      changes: [
        { person: people[0].id, note: 'One' },
        { person: people[0].id, note: 'Two' },
      ],
    });
    expect(await api.undoChangeGroup(groups[0])).toEqual({ undone: 1, skipped: [] });
    expect((await api.getPerson(people[0].id)).notes.map((n) => n.text)).not.toContain('One');
    expect((await api.getPerson(people[0].id)).notes.map((n) => n.text)).toContain('Two');
    const log = await api.changeLog({ unreviewedOnly: false }, 0, 10);
    expect(log.items[0].actor).toEqual({ kind: 'user' });
    expect(log.items[0].label).toBe('Undo: Save note');
    const undone = log.items.find((r) => r.group === groups[0])!;
    expect(undone.reverted).toBe(true);
    expect(undone.canUndo).toBe(false);
    // Undoing again is harmless.
    expect(await api.undoChangeGroup(groups[0])).toEqual({ undone: 0, skipped: [] });
  });

  it('a_changed_fact_is_skipped_and_stays_open', async () => {
    const { api, platform, people } = await make();
    const { groups, batch } = platform.simulateAgentBatch(api, {
      title: 'B',
      changes: [
        { person: people[0].id, note: 'Gone' },
        { person: people[1].id, note: 'Kept', conflict: true },
      ],
    });
    const report = await api.undoChangeBatch(batch!);
    expect(report.undone).toBe(1);
    expect(report.skipped).toHaveLength(1);
    expect(report.skipped[0]).toMatchObject({ kind: 'note', detail: 'Kept', fact: { kind: 'text', text: 'Kept' } });
    expect(report.skipped[0].person?.id).toBe(people[1].id);
    const log = await api.changeLog({ unreviewedOnly: false, batch: batch! }, 0, 10);
    expect(log.items.find((r) => r.group === groups[1])!.canUndo).toBe(true);
    expect(log.items.find((r) => r.group === groups[0])!.canUndo).toBe(false);
    expect(await api.unreviewedPeople()).toEqual([people[1].id]);
  });

  it('owner_rows_cannot_be_undone_from_the_log', async () => {
    const { api } = await make();
    await api.apply({ type: 'createPerson', draft: { given: 'Ada', surnames: ['K'], sex: 'female' } });
    const [row] = (await api.changeLog({ unreviewedOnly: false }, 0, 1)).items;
    await expect(api.undoChangeGroup(row.group)).rejects.toMatchObject({ code: 'invalid' });
  });

  it('batches_count_and_review', async () => {
    const { api, platform, people } = await make();
    const { batch, groups } = platform.simulateAgentBatch(api, {
      title: 'B',
      agent: 'Reader',
      report: 'Done',
      sourceFile: 'a.txt',
      changes: [
        { person: people[0].id, note: 'One' },
        { person: people[1].id, note: 'Two' },
      ],
    });
    const [dto] = await api.changeBatches();
    expect(dto).toMatchObject({ id: batch, title: 'B', actorName: 'Reader', report: 'Done', sourceFile: 'a.txt', groups: 2, people: 2, unreviewed: 2 });
    expect(dto.ended).not.toBeNull();
    await api.markReviewed({ kind: 'group', id: groups[0] });
    expect((await api.changeBatches())[0].unreviewed).toBe(1);
    await api.markReviewed({ kind: 'batch', id: batch! });
    expect((await api.changeBatches())[0].unreviewed).toBe(0);
    expect(await api.unreviewedPeople()).toEqual([]);
    expect((await api.changeLog({ unreviewedOnly: true }, 0, 10)).total).toBe(0);
    expect((await api.changeLog({ unreviewedOnly: false }, 0, 10)).total).toBe(2);
  });

  it('change_log_agents_are_every_author_once_with_the_latest_name', async () => {
    const { api, platform, people } = await make();
    expect(await api.changeLogAgents()).toEqual([]);
    await api.apply({ type: 'createPerson', draft: { given: 'Olena', surnames: ['Koval'], sex: 'female' } });
    platform.simulateAgentBatch(api, { agent: 'Zoe', changes: [{ person: people[0].id, note: 'One' }] });
    platform.simulateAgentBatch(api, { agent: 'Archive helper', changes: [{ person: people[0].id, note: 'Two' }] });
    platform.simulateAgentBatch(api, { agent: 'Zoe', changes: [{ person: people[0].id, note: 'Three' }] });
    expect((await api.changeLogAgents()).map((a) => a.name)).toEqual(['Archive helper', 'Zoe']);
  });

  it('a_batch_counts_the_changes_that_still_stand', async () => {
    const { api, platform, people } = await make();
    const { batch, groups } = platform.simulateAgentBatch(api, {
      title: 'B',
      changes: [
        { person: people[0].id, note: 'One' },
        { person: people[1].id, note: 'Two' },
      ],
    });
    expect((await api.changeBatches())[0].standing).toBe(2);
    await api.undoChangeGroup(groups[0]);
    expect((await api.changeBatches())[0].standing).toBe(1);
    await api.undoChangeBatch(batch!);
    expect((await api.changeBatches())[0].standing).toBe(0);
  });

  it('person_agent_changes_lists_what_stands_reviewed_or_not', async () => {
    const { api, platform, people } = await make();
    const { groups } = platform.simulateAgentBatch(api, {
      changes: [
        { person: people[0].id, note: 'One' },
        { person: people[0].id, note: 'Two' },
      ],
    });
    await api.markReviewed({ kind: 'person', id: people[0].id });
    await api.undoChangeGroup(groups[0]);
    const rows = (await api.personAgentChanges(people[0].id, 0, 20)).items;
    expect(rows.map((r) => r.group)).toEqual([groups[1]]);
    expect(rows[0].people[0].reviewed).not.toBeNull();
  });

  it('filters_by_actor_kind_id_batch_and_person', async () => {
    const { api, platform, people } = await make();
    await api.apply({ type: 'createPerson', draft: { given: 'Ada', surnames: ['K'], sex: 'female' } });
    const a = platform.simulateAgentBatch(api, { agent: 'A', title: 'TA', changes: [{ person: people[0].id, note: 'x' }] });
    platform.simulateAgentBatch(api, { agent: 'B', changes: [{ person: people[1].id, note: 'y' }] });
    const rows = (await api.changeLog({ unreviewedOnly: false }, 0, 10)).items;
    expect(rows).toHaveLength(3);
    const agentA = rows.find((r) => r.actor.kind === 'agent' && r.actor.name === 'A')!.actor as { id: string };
    expect((await api.changeLog({ unreviewedOnly: false, actorKind: 'user' }, 0, 10)).total).toBe(1);
    expect((await api.changeLog({ unreviewedOnly: false, actorKind: 'agent' }, 0, 10)).total).toBe(2);
    expect((await api.changeLog({ unreviewedOnly: false, actorId: agentA.id }, 0, 10)).total).toBe(1);
    expect((await api.changeLog({ unreviewedOnly: false, batch: a.batch! }, 0, 10)).total).toBe(1);
    expect((await api.changeLog({ unreviewedOnly: false, person: people[1].id }, 0, 10)).total).toBe(1);
    expect((await api.changeLog({ unreviewedOnly: false }, 1, 1)).items).toHaveLength(1);
  });
});

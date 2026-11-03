<script lang="ts">
  // Dev-only page: opens each editor on MockApi data. /editors.html?view=person|event|family|quick
  import { MockApi } from '../lib/api/mock/MockApi';
  import { MockPlatform } from '../lib/api/mock/MockPlatform';
  import { fixtureTree } from '../lib/api/mock/fixtures';
  import { setApi } from '../lib/api/context';
  import { settings } from '../lib/state/settings.svelte';
  import PersonEditor from '../lib/views/PersonEditor.svelte';
  import EventEditor from '../lib/editors/EventEditor.svelte';
  import FamilyEditor from '../lib/editors/FamilyEditor.svelte';
  import QuickAdd from '../lib/editors/QuickAdd.svelte';
  import type { EventDto, PartnershipDto, PersonDetail } from '../lib/api/types';

  const api = new MockApi(fixtureTree(500));
  setApi(api, new MockPlatform());
  const params = new URLSearchParams(location.search);
  const view = params.get('view') ?? 'person';
  const theme = params.get('theme');
  if (theme) document.documentElement.dataset.theme = theme;

  const none = () => {};
  interface Ctx {
    person: PersonDetail;
    ps: PartnershipDto;
    event: EventDto | undefined;
  }
  async function find(): Promise<Ctx> {
    await settings.load(api);
    const page = await api.listPersons(
      { sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false },
      0,
      200,
    );
    for (const row of page.items) {
      const d = await api.getPerson(row.id);
      const ps = d.partnerships.find((p) => p.children.length >= 3 && p.partner);
      if (ps && d.events.some((e) => e.date && e.place) && d.notes.length > 0) {
        return { person: d, ps, event: d.events.find((e) => e.date && e.place) };
      }
    }
    throw new Error('no fixture person');
  }
  const ready = find();
</script>

{#await ready}
  <p>Loading</p>
{:then c}
  {#if view === 'person'}
    <PersonEditor id={c.person.ref.id} />
  {:else if view === 'event'}
    <EventEditor owner={{ kind: 'person', id: c.person.ref.id }} event={c.event} onSaved={none} onCancel={none} />
  {:else if view === 'family'}
    <FamilyEditor family={c.ps.familyId} anchor={c.person.ref.id} onDone={none} onCancel={none} />
  {:else if view === 'quick'}
    <QuickAdd anchor={c.person.ref} relation="child" onDone={none} onCancel={none} />
  {/if}
{:catch e}
  <p>{String(e)}</p>
{/await}

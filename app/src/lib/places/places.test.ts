// The places view against the mock api: the events list with its filters, relatives that open the person, and the
// import dialog from the files found to the result. Invented villages and people only.
import userEvent from '@testing-library/user-event';
import { screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { renderWith } from '../testing/renderWith';
import { seedPlaceDemo } from './demoData';
import PlacesView from './PlacesView.svelte';

beforeEach(() => {
  nav.reset();
  project.close();
});

async function mount() {
  const api = new MockApi(fixtureTree(10));
  const demo = await seedPlaceDemo(api);
  const onOpenPerson = vi.fn();
  renderWith(PlacesView, { onOpenPerson }, { api });
  const user = userEvent.setup();
  await user.type(await screen.findByLabelText('Find a place'), 'Zelenivka');
  await user.click(await screen.findByRole('button', { name: /^Zelenivka/ }));
  await screen.findByRole('heading', { name: /Events in Zelenivka/ });
  return { api, demo, user, onOpenPerson };
}

describe('Places view', () => {
  it('lists_events_oldest_first_with_the_count_per_kind', async () => {
    await mount();
    expect(await screen.findByText('62 events')).toBeInTheDocument();
    const rows = within(await screen.findByRole('listbox', { name: 'Events of the place' })).getAllByRole('option');
    expect(rows.length).toBeGreaterThan(5);
    expect(within(rows[0]).getByText('1820')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Birth/ })).toBeInTheDocument();
  });

  it('filters_by_whose_kind_and_name', async () => {
    const { user } = await mount();
    await user.click(screen.getByRole('radio', { name: 'Relatives' }));
    expect(await screen.findByText('2 events')).toBeInTheDocument();
    await user.click(screen.getByRole('radio', { name: 'Others' }));
    expect(await screen.findByText(/^60 events/)).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText('Kind'), 'death');
    expect(await screen.findByText(/^10 events/)).toBeInTheDocument();
    await user.type(screen.getByLabelText('Name'), 'bondar');
    await waitFor(() => expect(screen.getByText(/^\d+ events?/)).toBeInTheDocument());
    await user.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect(await screen.findByText('62 events')).toBeInTheDocument();
  });

  it('filters_by_a_decade_from_the_chart', async () => {
    const { user } = await mount();
    await user.click(await screen.findByRole('button', { name: /1820/ }));
    await waitFor(() => expect((screen.getByLabelText('From year') as HTMLInputElement).value).toBe('1820'));
    expect((screen.getByLabelText('To year') as HTMLInputElement).value).toBe('1829');
  });

  it('opens_a_relative_and_marks_the_unsure_records', async () => {
    const { user, onOpenPerson, demo } = await mount();
    await user.click(screen.getByRole('radio', { name: 'Relatives' }));
    await user.click(await screen.findByRole('button', { name: /Ivan/ }));
    expect(onOpenPerson).toHaveBeenCalledWith(demo.relatives[0]);
    await user.click(screen.getByRole('radio', { name: 'Others' }));
    expect((await screen.findAllByText(/unsure/)).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Book BK-1, page \d+/).length).toBeGreaterThan(0);
  });

  it('imports_from_the_files_found_after_a_preview', async () => {
    const { user, api } = await mount();
    const run = vi.spyOn(api, 'placeImportRun');
    await user.click(screen.getByRole('button', { name: 'Import records…' }));
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: 'Choose files…' }));
    expect(await within(dialog).findByText('BK-2')).toBeInTheDocument();
    expect(within(dialog).getByText(/540 rows, 200 records in 2 books/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Create Hirka' }));
    await user.click(within(dialog).getByRole('button', { name: 'Preview' }));
    expect(await within(dialog).findByText(/200 to add, 0 already stored/)).toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Import' }));
    expect(await within(dialog).findByText(/Added 200 records/)).toBeInTheDocument();
    expect(run).toHaveBeenCalledTimes(1);
    // Again: everything is stored already.
    await user.click(within(dialog).getAllByRole('button', { name: 'Close' }).at(-1)!);
    await user.click(screen.getByRole('button', { name: 'Import records…' }));
    const again = await screen.findByRole('dialog');
    await user.click(within(again).getByRole('button', { name: 'Choose files…' }));
    // Hirka was created by the first import, so it is an existing place now and is chosen by itself.
    await user.click(await within(again).findByRole('button', { name: 'Preview' }));
    expect(await within(again).findByText(/0 to add, 200 already stored/)).toBeInTheDocument();
    expect(within(again).getByRole('button', { name: 'Import' })).toBeDisabled();
  });

  it('defaults_each_village_to_the_place_named_like_it_and_never_to_another', async () => {
    const { user, api } = await mount();
    const run = vi.spyOn(api, 'placeImportPreview');
    await user.click(screen.getByRole('button', { name: 'Import records…' }));
    const dialog = await screen.findByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: 'Choose files…' }));
    await within(dialog).findByText('BK-2');
    // Zelenivka is an existing place and is chosen; Hirka is not one, so its field is empty and offers the name.
    expect((within(dialog).getByRole('combobox', { name: /^Place: BK-1 Zelenivka/ }) as HTMLInputElement).value).toMatch(/^Zelenivka/);
    expect((within(dialog).getByRole('combobox', { name: /^Place: BK-2 Hirka/ }) as HTMLInputElement).value).toBe('');
    expect(within(dialog).getByText('No place named Hirka yet.')).toBeInTheDocument();
    expect(within(dialog).getAllByRole('button', { name: /^Create / })).toHaveLength(1);
    await user.click(within(dialog).getByRole('button', { name: 'Preview' }));
    await within(dialog).findByText(/to add/);
    const mapping = run.mock.calls[0][1];
    expect(mapping.find((m) => m.village === 'Hirka')?.place).toBeNull();
    expect(mapping.find((m) => m.village === 'Zelenivka')?.place).toBe(demoPlaceId(api));
  });

  it('shows_the_principal_people_of_each_kind_in_the_row', async () => {
    const { user } = await mount();
    await user.click(screen.getByRole('radio', { name: 'Others' }));
    const rows = () => within(screen.getByRole('listbox', { name: 'Events of the place' })).getAllByRole('option');
    await user.selectOptions(screen.getByLabelText('Kind'), 'birth');
    await waitFor(() => expect(rows().length).toBeGreaterThan(0));
    const birth = rows()[0].textContent ?? '';
    expect(birth).toMatch(/\w+ Bondar|\w+ \w+/);
    expect(birth).toMatch(/·.*Petro .* & Hanna /);
    await user.selectOptions(screen.getByLabelText('Kind'), 'marriage');
    await waitFor(() => expect(rows()[0].textContent).toMatch(/×/));
    await user.selectOptions(screen.getByLabelText('Kind'), 'death');
    await waitFor(() => expect(rows()[0].textContent).toMatch(/, age 40/));
    await user.selectOptions(screen.getByLabelText('Kind'), 'census');
    await waitFor(() => expect(rows()[0].textContent).toMatch(/\+2 members/));
    // The row names only the head; a click shows everyone with role, age, note and the source.
    expect(rows()[0].textContent).not.toMatch(/Stepan/);
    await user.click(rows()[0]);
    const detail = await screen.findByRole('heading', { name: /Everyone in this event/ });
    const panel = detail.parentElement!;
    expect(within(panel).getByText('wife')).toBeInTheDocument();
    expect(within(panel).getByText('son')).toBeInTheDocument();
    expect(panel.textContent).toMatch(/Book BK-1, page \d+, record bk1:\d+/);
  });

  it('marks_relatives_in_the_detail_and_links_them_to_the_person', async () => {
    const { user, onOpenPerson, demo } = await mount();
    await user.click(screen.getByRole('radio', { name: 'Relatives' }));
    const list = await screen.findByRole('listbox', { name: 'Events of the place' });
    await user.click(within(list).getAllByRole('option')[0]);
    const panel = (await screen.findByRole('heading', { name: /Everyone in this event/ })).parentElement!;
    expect(within(panel).getByText('in the family tree')).toBeInTheDocument();
    await user.click(within(panel).getByRole('button', { name: /Ivan/ }));
    expect(onOpenPerson).toHaveBeenCalledWith(demo.relatives[0]);
  });
});

function demoPlaceId(api: MockApi): string | undefined {
  return [...(api as unknown as { db: { places: Map<string, { id: string; name: string }> } }).db.places.values()].find((p) => p.name === 'Zelenivka')?.id;
}

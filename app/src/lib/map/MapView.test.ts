// The Map view against the mock api: modes, the no-coordinates counter, the helper with the offline place data, a
// click on a dot. Invented people and villages only.
import userEvent from '@testing-library/user-event';
import { screen, waitFor, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { renderWith } from '../testing/renderWith';
import MapView from './MapView.svelte';

beforeEach(() => {
  nav.reset();
  project.close();
});

async function koval(api: MockApi, place: string) {
  const r = await api.apply({ type: 'createPerson', draft: { given: 'Ivan', surnames: ['Koval'], sex: 'male', birthDateRaw: '1900', birthPlaceText: place } });
  nav.focus(r.focus!);
  return r.focus!;
}

async function mount(place = 'Berezivka, Zelena oblast') {
  const api = new MockApi(fixtureTree(10));
  const id = await koval(api, place);
  const onOpenPerson = vi.fn();
  renderWith(MapView, { onOpenPerson }, { api });
  const user = userEvent.setup();
  await user.selectOptions(await screen.findByLabelText('Show'), 'person');
  return { api, id, user, onOpenPerson };
}

describe('Map view', () => {
  it('offers_three_modes_and_counts_events_without_coordinates', async () => {
    const { user } = await mount();
    expect(await screen.findByText('1 event has no coordinates.')).toBeInTheDocument();
    for (const name of ['Places', 'Heat map', 'Movement']) expect(screen.getByRole('radio', { name })).toBeInTheDocument();
    await user.click(screen.getByRole('radio', { name: 'Heat map' }));
    await user.click(screen.getByRole('radio', { name: 'Movement' }));
    expect(screen.getByLabelText('Colour by')).toBeInTheDocument();
    expect(screen.getByText(/Natural Earth/)).toBeInTheDocument();
  });

  it('downloads_place_data_on_a_click_and_sets_a_point_only_when_confirmed', async () => {
    const { api, user } = await mount();
    await user.click(await screen.findByRole('button', { name: 'Set them' }));
    const helper = screen.getByRole('complementary', { name: 'Place coordinates' });
    expect(within(helper).getByText(/There is no place data yet/)).toBeInTheDocument();
    expect(within(helper).getByText(/Place data: GeoNames/)).toBeInTheDocument();

    const get = vi.spyOn(api, 'gazetteerDownload');
    const code = within(helper).getByLabelText('Country code (two letters)');
    await user.clear(code);
    await user.type(code, 'zz');
    expect(await within(helper).findByText(/About 4\.1 MB from geonames\.org/)).toBeInTheDocument();
    // Nothing was downloaded, and nothing is suggested, until the person asks.
    expect(get).not.toHaveBeenCalled();
    await user.click(within(helper).getByRole('button', { name: 'Download place data' }));
    expect(await within(helper).findByText('Place data on this computer')).toBeInTheDocument();
    expect(get).toHaveBeenCalledWith('ZZ', expect.any(Function));

    // The best match is listed with its region; the other village of the same name is offered second.
    const names = await within(helper).findAllByText('Berezivka');
    expect(names.length).toBe(2);
    expect(within(helper).getByText(/matches the region/)).toBeInTheDocument();
    const before = await api.mapData({ kind: 'all' });
    expect(before.places.every((p) => p.lat === null)).toBe(true);

    await user.click(within(helper).getAllByRole('button', { name: 'Use this' })[0]);
    await waitFor(() => expect(screen.queryByText('1 event has no coordinates.')).not.toBeInTheDocument());
    const after = await api.mapData({ kind: 'all' });
    const set = after.places.find((p) => p.name === 'Berezivka')!;
    expect(set.lat).toBeCloseTo(50.52, 6);
    expect(set.lon).toBeCloseTo(30.28, 6);
    // It went through the normal edit path, so it can be undone.
    await api.undo();
    expect((await api.mapData({ kind: 'all' })).places.find((p) => p.name === 'Berezivka')!.lat).toBeNull();
  });

  it('clicking_the_map_sets_the_point_for_the_chosen_place', async () => {
    const { api, user } = await mount();
    await user.click(await screen.findByRole('button', { name: 'Set them' }));
    await user.click(await screen.findByRole('button', { name: 'Click on the map' }));
    expect(screen.getByText(/Click the map where "Berezivka, Zelena oblast" is/)).toBeInTheDocument();
    const canvas = screen.getByLabelText(/Map of the places in the tree/);
    canvas.dispatchEvent(new MouseEvent('pointerdown', { clientX: 100, clientY: 100, bubbles: true }));
    canvas.dispatchEvent(new MouseEvent('pointerup', { clientX: 100, clientY: 100, bubbles: true }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).places.find((p) => p.name === 'Berezivka')!.lat).not.toBeNull());
  });

  it('opens_the_people_of_a_place_from_its_dot', async () => {
    const { api, id, onOpenPerson } = await mount();
    await api.apply({ type: 'setPlaceCoordinates', place: (await api.mapData({ kind: 'person', person: id })).places[0].id, latE6: 50_520_000, lonE6: 30_280_000 });
    project.rev++; // an edit elsewhere refreshes the view
    await waitFor(() => expect(screen.queryByText('1 event has no coordinates.')).not.toBeInTheDocument());
    const canvas = screen.getByLabelText(/Map of the places in the tree/);
    // The only point is fitted to the middle of the (200 x 200 in jsdom) view.
    canvas.dispatchEvent(new MouseEvent('pointerdown', { clientX: 100, clientY: 100, bubbles: true }));
    canvas.dispatchEvent(new MouseEvent('pointerup', { clientX: 100, clientY: 100, bubbles: true }));
    const panel = await screen.findByRole('complementary', { name: 'Place' });
    expect(within(panel).getByText('Berezivka, Zelena oblast')).toBeInTheDocument();
    expect(within(panel).getByText(/Birth 1900/)).toBeInTheDocument();
    within(panel).getByRole('button', { name: /Ivan/ }).click();
    expect(onOpenPerson).toHaveBeenCalledWith(id);
  });

  it('filters_events_by_kind_and_keeps_the_picture_free_of_living_people_by_default', async () => {
    const { api, id, user } = await mount();
    await api.apply({ type: 'saveEvent', owner: { kind: 'person', id }, event: { kind: 'residence', dateRaw: '1925', placeText: 'Dubky, Zelena oblast', negative: false, private: false, preferred: false, participants: [{ person: id, role: 'principal' }] } });
    project.rev++;
    expect(await screen.findByText('2 events have no coordinates.')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Residence' }));
    expect(await screen.findByText('1 event has no coordinates.')).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: 'Include living people in the picture' })).not.toBeChecked();
  });
});

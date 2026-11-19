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
import { notices } from '../ui/notices.svelte';
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

  it('adds_an_old_map_places_it_by_its_corners_and_keeps_the_layer_list_in_order', async () => {
    const api = new MockApi(fixtureTree(10));
    const { platform } = renderWith(MapView, { onOpenPerson: vi.fn() }, { api });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Old maps' }));
    const panel = screen.getByRole('complementary', { name: 'Old maps' });
    expect(within(panel).getByText('No old maps yet.')).toBeInTheDocument();

    // The first map comes with no .map file: after adding it, the owner places it by its corners.
    platform.next.file = 'C:/Maps/Zelena oblast 1865.jpg';
    await user.click(within(panel).getByRole('button', { name: 'Add a map…' }));
    expect(within(panel).getByText('No .map file: you place it by hand.')).toBeInTheDocument();
    await user.type(within(panel).getByLabelText('Year'), '1865');
    await user.type(within(panel).getByLabelText('Source (web address or archive reference)'), 'https://example.org/z');
    await user.click(within(panel).getByRole('button', { name: 'Add to the map' }));
    expect(await screen.findByText(/Drag the corners of “Zelena oblast 1865”/)).toBeInTheDocument();
    for (const name of ['Top left corner', 'Top right corner', 'Bottom right corner', 'Bottom left corner', 'Move the whole map'])
      expect(screen.getByRole('button', { name })).toBeInTheDocument();
    // A corner moved with the keyboard, then saved.
    screen.getByRole('button', { name: 'Bottom right corner' }).focus();
    await user.keyboard('{Shift>}{ArrowRight}{/Shift}');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers[0].corners).toHaveLength(4));
    const first = (await api.mapData({ kind: 'all' })).layers[0];
    expect(first).toMatchObject({ title: 'Zelena oblast 1865', year: '1865', source: 'https://example.org/z', opacity: 70, visible: true });

    // The second comes with its OziExplorer .map file: it is placed at once.
    platform.next.file = 'C:/Maps/sheet 30-13.jpg';
    await user.click(within(panel).getByRole('button', { name: 'Add a map…' }));
    expect(within(panel).getByText('Corners from sheet 30-13_ozf.map')).toBeInTheDocument();
    await user.click(within(panel).getByRole('button', { name: 'Add to the map' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers).toHaveLength(2));
    expect(screen.queryByText(/Drag the corners/)).not.toBeInTheDocument();
    const layers = (await api.mapData({ kind: 'all' })).layers;
    expect(layers.map((l) => l.title)).toEqual(['Zelena oblast 1865', 'sheet 30-13']);
    expect(layers[1].corners).toHaveLength(4);

    // The list shows the top map first; moving it down puts it under the other.
    const rows = within(panel).getAllByRole('listitem');
    expect(within(rows[0]).getByText('sheet 30-13')).toBeInTheDocument();
    await user.click(within(panel).getByRole('button', { name: 'Move sheet 30-13 down' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers.map((l) => l.title)).toEqual(['sheet 30-13', 'Zelena oblast 1865']));

    // Hidden, then off the map: the picture stays a media record.
    await user.click(within(panel).getByRole('checkbox', { name: 'Show sheet 30-13' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers.find((l) => l.title === 'sheet 30-13')!.visible).toBe(false));
    const media = (await api.mapData({ kind: 'all' })).layers.find((l) => l.title === 'sheet 30-13')!.media;
    await user.click(within(panel).getAllByRole('button', { name: 'Remove from map' })[1]);
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers).toHaveLength(1));
    expect((await api.unlinkedMedia()).some((m) => m.id === media)).toBe(true);
  });

  it('groups_forty_maps_by_series_shows_a_series_in_one_undo_step_and_one_map_alone', async () => {
    const api = new MockApi(fixtureTree(10));
    const series = ['Three-verst map', 'Two-verst map', 'Schubert', 'Strelbitsky', 'Rizzi-Zannoni'];
    const ids: string[] = [];
    for (let i = 0; i < 40; i++) {
      const s = series[i % 5];
      const lat = 52 - Math.floor(i / 8) * 0.6;
      const lon = 26 + (i % 8) * 0.8;
      const c = (la: number, lo: number) => ({ latE6: Math.round(la * 1e6), lonE6: Math.round(lo * 1e6) });
      ids.push(await api.addMapLayerFile(`C:/Maps/${s} ${i}.jpg`, `${s} sheet ${i}`, { series: s, opacity: 70, visible: false, corners: [c(lat, lon), c(lat, lon + 0.8), c(lat - 0.6, lon + 0.8), c(lat - 0.6, lon)] }));
    }
    const asked = vi.spyOn(api, 'mediaUrl');
    const shown = vi.spyOn(notices, 'show');
    renderWith(MapView, { onOpenPerson: vi.fn() }, { api });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Old maps (40)' }));
    const panel = screen.getByRole('complementary', { name: 'Old maps' });
    // Five series, folded (40 maps), each with its count; nothing hidden was ever asked for.
    for (const s of series) expect(within(panel).getByRole('button', { name: s })).toHaveAttribute('aria-expanded', 'false');
    expect(within(panel).getAllByText('0 of 8 shown')).toHaveLength(5);
    expect(asked).not.toHaveBeenCalled();

    // The Schubert series shown with its group checkbox: one change, one notice, one undo step.
    const before = (await api.undoState()).undoLabel;
    await user.click(within(panel).getByRole('checkbox', { name: 'Show the series Schubert' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers.filter((l) => l.visible)).toHaveLength(8));
    expect(shown).toHaveBeenCalledTimes(1);
    expect(shown).toHaveBeenLastCalledWith({ message: 'Schubert shown (8 maps).' });
    expect((await api.undoState()).undoLabel).toBe('Show maps on the Map');
    await waitFor(() => expect(within(panel).getByRole('checkbox', { name: 'Show the series Schubert' })).toBeChecked());
    // Only the shown maps' pictures are asked for.
    await waitFor(() => expect(asked).toHaveBeenCalled());
    const schubert = new Set((await api.mapData({ kind: 'all' })).layers.filter((l) => l.series === 'Schubert').map((l) => l.media));
    for (const [id] of asked.mock.calls) expect(schubert.has(id)).toBe(true);
    await api.undo();
    expect((await api.mapData({ kind: 'all' })).layers.filter((l) => l.visible)).toHaveLength(0);
    expect((await api.undoState()).undoLabel).toBe(before);
    await api.redo();
    project.rev++;

    // A partly shown series: its box is mixed, and a click shows the rest.
    await user.click(within(panel).getByRole('button', { name: 'Two-verst map' }));
    await user.click(within(panel).getByRole('checkbox', { name: 'Show Two-verst map sheet 1' }));
    await waitFor(() => expect((within(panel).getByRole('checkbox', { name: 'Show the series Two-verst map' }) as HTMLInputElement).indeterminate).toBe(true));

    // Only this one: every other map hidden, in one step.
    shown.mockClear();
    await user.click(within(panel).getByRole('button', { name: 'Show only Two-verst map sheet 6' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers.filter((l) => l.visible).map((l) => l.title)).toEqual(['Two-verst map sheet 6']));
    expect(shown).toHaveBeenCalledTimes(1);
    expect((await api.undoState()).undoLabel).toBe('Choose maps shown on the Map');
    await api.undo();
    expect((await api.mapData({ kind: 'all' })).layers.filter((l) => l.visible)).toHaveLength(9);
    await api.redo();
    project.rev++;

    // The filter finds a sheet by its number; Show all and Hide all are one step each.
    await user.type(within(panel).getByRole('searchbox', { name: 'Find a map by title or sheet' }), 'sheet 17');
    await waitFor(() => expect(within(panel).getAllByRole('checkbox', { name: /^Show .* sheet/ })).toHaveLength(1));
    await user.clear(within(panel).getByRole('searchbox', { name: 'Find a map by title or sheet' }));
    await user.click(within(panel).getByRole('button', { name: 'Hide all' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers.every((l) => !l.visible)).toBe(true));
    expect((await api.undoState()).undoLabel).toBe('Hide maps on the Map');
    expect(ids).toHaveLength(40);
  });

  it('groups_maps_by_place_filters_them_by_years_and_place_and_shows_one_places_maps_in_one_step', async () => {
    const api = new MockApi(fixtureTree(10));
    const id = await koval(api, 'Berezivka, Zelena oblast');
    await api.apply({ type: 'setPlaceCoordinates', place: (await api.mapData({ kind: 'person', person: id })).places[0].id, latE6: 50_520_000, lonE6: 30_280_000 });
    const c = (la: number, lo: number) => ({ latE6: Math.round(la * 1e6), lonE6: Math.round(lo * 1e6) });
    const box = (la: number, lo: number) => [c(la + 0.3, lo - 0.3), c(la + 0.3, lo + 0.3), c(la - 0.3, lo + 0.3), c(la - 0.3, lo - 0.3)];
    await api.addMapLayerFile('C:/Maps/a.jpg', 'Plan one 1648, sheet 1-1', { series: 'Plans', opacity: 70, visible: false, corners: box(50.52, 30.28) });
    await api.addMapLayerFile('C:/Maps/b.jpg', 'Schubert 10-verst, sheet 51 (1826–1840)', { series: 'Schubert', opacity: 70, visible: false, corners: box(50.6, 30.3) });
    await api.addMapLayerFile('C:/Maps/c.jpg', 'Far sheet', { series: 'Schubert', opacity: 70, visible: false, corners: box(10, 10) });
    renderWith(MapView, { onOpenPerson: vi.fn() }, { api });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Old maps (3)' }));
    const panel = screen.getByRole('complementary', { name: 'Old maps' });
    // The place list offers the place with its two maps.
    expect(await within(panel).findByRole('option', { name: 'Berezivka (2)' })).toBeInTheDocument();

    await user.click(within(within(panel).getByRole('group', { name: 'Group by' })).getByRole('button', { name: 'Place' }));
    expect(within(panel).getByRole('button', { name: 'Berezivka' })).toBeInTheDocument();
    expect(within(panel).getByRole('button', { name: 'Other maps' })).toBeInTheDocument();
    expect(within(panel).getAllByRole('checkbox', { name: /^Show .*sheet(?!.*under the middle)/ })).toHaveLength(3);

    // Years: 1800-1900 keeps the Schubert sheet, and the sheet with no year while the box is ticked.
    await user.type(within(panel).getByLabelText('From year'), '1800');
    await user.type(within(panel).getByLabelText('To year'), '1900');
    await waitFor(() => expect(within(panel).getAllByRole('checkbox', { name: /^Show .*sheet(?!.*under the middle)/ })).toHaveLength(2));
    await user.click(within(panel).getByRole('checkbox', { name: 'No year' }));
    await waitFor(() => expect(within(panel).getAllByRole('checkbox', { name: /^Show .*sheet(?!.*under the middle)/ })).toHaveLength(1));
    await user.click(within(panel).getByRole('checkbox', { name: 'No year' }));
    await user.clear(within(panel).getByLabelText('From year'));
    await user.clear(within(panel).getByLabelText('To year'));

    // One place: only its group; "Only these" shows just its two maps, in one undo step.
    await user.selectOptions(within(panel).getByLabelText('Place'), 'Berezivka (2)');
    await waitFor(() => expect(within(panel).queryByRole('button', { name: 'Other maps' })).not.toBeInTheDocument());
    await user.click(within(panel).getByRole('button', { name: 'Show only the maps of Berezivka' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers.filter((l) => l.visible)).toHaveLength(2));
    expect((await api.undoState()).undoLabel).toBe('Show maps on the Map');
  });

  it('a_map_file_that_does_not_fit_the_picture_opens_the_corners_instead_of_placing_it', async () => {
    const api = new MockApi(fixtureTree(10));
    const { platform } = renderWith(MapView, { onOpenPerson: vi.fn() }, { api });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Old maps' }));
    const panel = screen.getByRole('complementary', { name: 'Old maps' });
    // In the mock, a picture named "crop" was cropped from the one its .map file was made for.
    platform.next.file = 'C:/Maps/sheet 30-13 crop.jpg';
    await user.click(within(panel).getByRole('button', { name: 'Add a map…' }));
    await user.click(within(panel).getByRole('button', { name: 'Add to the map' }));
    expect(await screen.findByText(/Drag the corners of “sheet 30-13 crop”/)).toBeInTheDocument();
    expect(notices.current?.message).toMatch(/does not fit sheet 30-13 crop/);
    // Laid on the Map but not placed: the corners wait for the owner's Save.
    const l = (await api.mapData({ kind: 'all' })).layers[0];
    expect(l.corners).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers[0].corners?.[0]).toEqual({ latE6: 50_600_000, lonE6: 30_000_000 }));
  });

  it('edits_a_maps_corners_as_typed_degrees_and_refuses_a_wrong_one', async () => {
    const api = new MockApi(fixtureTree(10));
    const id = await api.addMapLayerFile('C:/Maps/a.png', 'Atlas square', { opacity: 50, visible: true });
    renderWith(MapView, { onOpenPerson: vi.fn() }, { api });
    const user = userEvent.setup();
    await user.click(await screen.findByRole('button', { name: 'Old maps (1)' }));
    const panel = screen.getByRole('complementary', { name: 'Old maps' });
    expect(within(panel).getByText('not placed yet')).toBeInTheDocument();
    expect(within(panel).getByRole('slider', { name: 'Opacity of Atlas square' })).toHaveValue('50');
    await user.click(within(panel).getByRole('button', { name: 'Edit' }));
    const values = ['50.6', '30', '50.6', '31.2', '49.9', '31.2', '49.9', '30'];
    const fields = ['Top left corner', 'Top right corner', 'Bottom right corner', 'Bottom left corner'].flatMap((c) => [`${c}: Latitude`, `${c}: Longitude`]);
    for (const [i, f] of fields.entries()) await user.type(within(panel).getByLabelText(f), i === 0 ? '95' : values[i]);
    await user.click(within(panel).getByRole('button', { name: 'Save' }));
    expect(await within(panel).findByRole('alert')).toHaveTextContent('Each corner needs a latitude');
    await user.clear(within(panel).getByLabelText('Top left corner: Latitude'));
    await user.type(within(panel).getByLabelText('Top left corner: Latitude'), '50,6');
    await user.click(within(panel).getByRole('button', { name: 'Save' }));
    await waitFor(async () => expect((await api.mapData({ kind: 'all' })).layers[0].corners?.[0]).toEqual({ latE6: 50_600_000, lonE6: 30_000_000 }));
    expect((await api.mapData({ kind: 'all' })).layers[0].media).toBe(id);
  });
});

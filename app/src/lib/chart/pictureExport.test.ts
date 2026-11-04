import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeFakeContext } from '../../test-setup';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { MockPlatform } from '../api/mock/MockPlatform';
import { setLanguage } from '../i18n';
import { en } from '../i18n/en';
import { uk } from '../i18n/uk';
import { notices } from '../ui/notices.svelte';
import { planPicture } from './picture';
import { captionText, chartTitle, renderPng, savePicture, suggestedName, type BuiltPicture } from './pictureExport';

const PNG_HEAD = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

function stubCanvas() {
  const made: { canvas: HTMLCanvasElement; ctx: ReturnType<typeof makeFakeContext> }[] = [];
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(function (this: HTMLCanvasElement) {
    const ctx = makeFakeContext();
    made.push({ canvas: this, ctx });
    return ctx as unknown as CanvasRenderingContext2D;
  } as never);
  vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (cb: BlobCallback) {
    const bytes = new Uint8Array([...PNG_HEAD, 9]);
    cb(Object.assign(new Blob([bytes], { type: 'image/png' }), { arrayBuffer: async () => bytes.buffer }));
  } as never);
  return made;
}

function built(over: { w?: number; h?: number } = {}): BuiltPicture {
  const scene = { nodes: [], edges: [] };
  const plan = planPicture('screen', scene, { cam: { x: 0, y: 0, zoom: 1 }, size: { w: over.w ?? 800, h: over.h ?? 600 }, dpr: 1 });
  return { plan, theme: { paper: '#fbfbf8', ink2: '#626a79', font: 'sans-serif' }, caption: 'Pedigree - Ivan Koval · 4 November 2026', paint: vi.fn() };
}

beforeEach(() => notices.dismiss());
afterEach(() => vi.restoreAllMocks());

describe('names', () => {
  it('suggests_a_file_name_for_each_kind_of_picture', () => {
    expect(suggestedName('whole', 'pedigree', 'Ivan Koval', 'Olena Koval')).toBe('Pedigree - Ivan Koval.png');
    expect(suggestedName('whole', 'whole', '', '')).toBe('Whole tree.png');
    expect(suggestedName('branch', 'descendants', 'Ivan Koval', 'Olena Koval')).toBe('Descendants - Olena Koval (branch).png');
    expect(suggestedName('screen', 'relatives', 'Ivan Koval', '')).toBe('Relatives - Ivan Koval (screen).png');
  });

  it('writes_the_caption_in_the_interface_language', () => {
    const day = new Date(2026, 10, 4);
    expect(captionText('pedigree', 'Ivan Koval', day)).toBe('Pedigree - Ivan Koval · November 4, 2026');
    setLanguage('uk');
    expect(chartTitle('pedigree', 'Іван Коваль')).toBe('Предки - Іван Коваль');
    expect(captionText('whole', '', day)).toMatch(/^Усе дерево · 4 листопада 2026/);
  });

  it('has_the_picture_texts_in_both_languages', () => {
    for (const key of Object.keys(en.picture) as (keyof typeof en.picture)[]) {
      expect(uk.picture[key], key).toBeTruthy();
      expect(uk.picture[key], key).not.toBe(en.picture[key]);
    }
    // No stray bytes from a wrong encoding.
    expect(JSON.stringify(uk.picture)).not.toMatch(/[ÐÑ]|\uFFFD/);
  });
});

describe('renderPng', () => {
  it('paints_the_chart_then_a_solid_background_behind_it_then_the_caption', async () => {
    const made = stubCanvas();
    const b = built();
    const bytes = await renderPng(b);
    expect([...bytes.slice(0, 8)]).toEqual(PNG_HEAD);
    const { canvas, ctx } = made[0];
    expect(canvas.width).toBe(800);
    expect(canvas.height).toBe(630);
    expect(b.paint).toHaveBeenCalledOnce();
    const ops = ctx.calls.map((c) => c.op);
    expect(ops.indexOf('clip')).toBeLessThan(ops.indexOf('fillRect'));
    expect(ops.indexOf('fillRect')).toBeLessThan(ops.indexOf('fillText'));
    // The fill goes under the drawing and covers the whole picture, so nothing is transparent.
    expect(ctx.calls.find((c) => c.op === 'fillRect')!.args).toEqual([0, 0, 800, 630]);
    expect(ctx.globalCompositeOperation).toBeDefined();
  });

  it('fails_with_a_plain_message_when_no_picture_comes_out', async () => {
    stubCanvas();
    vi.spyOn(HTMLCanvasElement.prototype, 'toBlob').mockImplementation(function (cb: BlobCallback) {
      cb(null);
    } as never);
    await expect(renderPng(built())).rejects.toThrow('The picture could not be made. Try saving a smaller part of the chart.');
  });
});

describe('savePicture', () => {
  function setup() {
    stubCanvas();
    const api = new MockApi(fixtureTree(5));
    const platform = new MockPlatform();
    const busy: boolean[] = [];
    return { api, platform, busy, setBusy: (b: boolean) => busy.push(b) };
  }

  it('asks_where_to_save_then_writes_the_png_and_tells_the_user', async () => {
    const s = setup();
    await savePicture({ api: s.api, platform: s.platform }, { fileName: 'Pedigree - Ivan Koval.png', build: async () => built(), setBusy: s.setBusy });
    expect(s.platform.calls[0]).toEqual({
      method: 'pickSavePath',
      args: [{ title: 'Save the chart as a picture', defaultName: 'Pedigree - Ivan Koval.png', extensions: ['png'] }],
    });
    expect(s.api.savedPictures).toHaveLength(1);
    expect(s.api.savedPictures[0].dest).toBe('C:/Mock/Pedigree - Ivan Koval.png');
    expect(notices.current?.message).toBe('Picture saved');
    expect(s.busy).toEqual([true, false]);
  });

  it('says_so_when_the_picture_was_scaled_down', async () => {
    const s = setup();
    await savePicture(
      { api: s.api, platform: s.platform },
      { fileName: 'Whole tree.png', build: async () => built({ w: 40960, h: 1000 }), setBusy: s.setBusy },
    );
    expect(notices.current?.message).toBe('The chart is very large, so the picture was saved at 40%.');
  });

  it('does_nothing_when_the_save_dialog_is_cancelled', async () => {
    const s = setup();
    s.platform.next.save = null;
    const build = vi.fn(async () => built());
    await savePicture({ api: s.api, platform: s.platform }, { fileName: 'x.png', build, setBusy: s.setBusy });
    expect(build).not.toHaveBeenCalled();
    expect(s.busy).toEqual([]);
    expect(notices.current).toBeNull();
  });

  it('reports_an_empty_picture_and_a_failed_write_in_plain_words', async () => {
    const s = setup();
    await savePicture({ api: s.api, platform: s.platform }, { fileName: 'x.png', build: async () => null, setBusy: s.setBusy });
    expect(notices.current?.message).toBe('There is nothing to save yet.');
    vi.spyOn(s.api, 'savePng').mockRejectedValue({ code: 'io', message: 'That folder does not exist. Choose another folder and try again.' });
    await savePicture({ api: s.api, platform: s.platform }, { fileName: 'x.png', build: async () => built(), setBusy: s.setBusy });
    expect(notices.current?.message).toBe('That folder does not exist. Choose another folder and try again.');
    expect(s.busy.at(-1)).toBe(false);
  });

  it('shows_the_message_in_the_interface_language', async () => {
    const s = setup();
    setLanguage('uk');
    await savePicture({ api: s.api, platform: s.platform }, { fileName: 'x.png', build: async () => built(), setBusy: s.setBusy });
    expect(notices.current?.message).toBe('Зображення збережено');
  });
});

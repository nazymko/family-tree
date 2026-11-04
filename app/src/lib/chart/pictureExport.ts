import type { Api, Platform } from '../api/api';
import type { ChartKind } from '../api/types';
import { currentLocale, t, type MessageKey } from '../i18n';
import { notices } from '../ui/notices.svelte';
import { CAPTION_H, MARGIN, formatOf, pictureFileName, scalePlan, type PictureFormat, type PicturePlan, type PictureScope } from './picture';
import { SvgContext, escapeXml, parseFont } from './svgContext';

/** Everything needed to paint one picture. */
export interface BuiltPicture {
  plan: PicturePlan;
  /** Colours of the strip below the chart (the chart itself is painted by `paint` with its own theme). */
  theme: { paper: string; ink2: string; font: string };
  /** One short line: the chart name and the date. */
  caption: string;
  /** Draws the chart area: a chart draw function called with `plan.cam`, `plan.view` and `plan.scale` as its dpr. */
  paint: (ctx: CanvasRenderingContext2D, plan: PicturePlan) => void;
}

const KIND_WORD: Record<ChartKind, MessageKey> = {
  pedigree: 'picture.kindPedigree',
  descendants: 'picture.kindDescendants',
  relatives: 'picture.kindRelatives',
  whole: 'picture.kindWhole',
};

/** The chart's name for the caption and the file name, e.g. "Pedigree - Ivan Koval" or "Whole tree". */
export function chartTitle(kind: ChartKind, rootName: string): string {
  const word = t(KIND_WORD[kind]);
  return rootName ? `${word} - ${rootName}` : word;
}

/** Suggested file name for the save dialog. */
export function suggestedName(scope: PictureScope, kind: ChartKind, rootName: string, selectedName: string): string {
  if (scope === 'branch' && selectedName) return pictureFileName(`${chartTitle(kind, selectedName)}${t('picture.branchSuffix')}`);
  if (scope === 'screen') return pictureFileName(`${chartTitle(kind, rootName)}${t('picture.screenSuffix')}`);
  return pictureFileName(chartTitle(kind, rootName));
}

/** "Pedigree - Ivan Koval · 4 November 2026", in the interface language. */
export function captionText(kind: ChartKind, rootName: string, now = new Date()): string {
  const date = now.toLocaleDateString(currentLocale(), { year: 'numeric', month: 'long', day: 'numeric' });
  return `${chartTitle(kind, rootName)} · ${date}`;
}

/** Paints a picture offscreen and returns the PNG bytes. */
export async function renderPng(b: BuiltPicture): Promise<Uint8Array> {
  const { plan } = b;
  const canvas = document.createElement('canvas');
  canvas.width = plan.width;
  canvas.height = plan.height;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error(t('picture.failed'));
  const chartW = Math.min(plan.width, Math.ceil(plan.view.w * plan.scale));
  const chartH = Math.min(plan.height, Math.ceil(plan.view.h * plan.scale));

  // The chart, clipped to its own area so nothing spills onto the caption strip.
  ctx.save();
  ctx.beginPath();
  ctx.rect(0, 0, chartW, chartH);
  ctx.clip();
  b.paint(ctx, plan);
  ctx.restore();

  // The background goes behind what is drawn, so the picture is never transparent.
  ctx.save();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalCompositeOperation = 'destination-over';
  ctx.fillStyle = b.theme.paper;
  ctx.fillRect(0, 0, plan.width, plan.height);
  ctx.restore();

  // The caption.
  ctx.setTransform(plan.scale, 0, 0, plan.scale, 0, 0);
  ctx.font = `12px ${b.theme.font}`;
  ctx.fillStyle = b.theme.ink2;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  const left = plan.view.w > 2 * MARGIN ? MARGIN : 12;
  let text = b.caption;
  const room = plan.view.w - left - 12;
  while (text.length > 1 && ctx.measureText(text).width > room) text = text.slice(0, -2).trimEnd() + '…';
  ctx.fillText(text, left, plan.view.h + CAPTION_H / 2);

  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
  if (!blob) throw new Error(t('picture.failed'));
  return new Uint8Array(await blob.arrayBuffer());
}

function messageOf(e: unknown): string {
  if (typeof e === 'object' && e !== null && typeof (e as { message?: unknown }).message === 'string') {
    const m = (e as { message: string }).message;
    if (m) return m;
  }
  return t('picture.failed');
}

/** Lets the browser paint (a frame, then a turn of the event loop). Never waits more than 100 ms, so it cannot hang. */
export function yieldFrame(): Promise<void> {
  return new Promise<void>((resolve) => {
    let done = false;
    const finish = () => {
      if (!done) {
        done = true;
        resolve();
      }
    };
    if (typeof requestAnimationFrame === 'function') requestAnimationFrame(() => setTimeout(finish, 0));
    setTimeout(finish, 100);
  });
}

/** A short pause between heavy steps so the page stays responsive. */
export const later = (): Promise<void> => new Promise<void>((resolve) => setTimeout(resolve, 0));

/**
 * Paints the picture as PNG and, when the browser cannot make a canvas that big, tries again at half the size (twice at
 * most). Returns the bytes and the plan that finally worked, so the message can name the real percentage.
 */
export async function renderPngAdaptive(b: BuiltPicture): Promise<{ bytes: Uint8Array; plan: PicturePlan }> {
  let plan = b.plan;
  let last: unknown = null;
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      return { bytes: await renderPng({ ...b, plan }), plan };
    } catch (e) {
      last = e;
      await later();
      plan = scalePlan(plan, 0.5);
    }
  }
  throw last;
}

/** Draws the picture as an SVG document (text, no size limit) using the same chart draw function. */
export function renderSvg(b: BuiltPicture): Uint8Array {
  const { plan } = b;
  const ctx = new SvgContext(plan.width, plan.height);
  b.paint(ctx as unknown as CanvasRenderingContext2D, plan);
  const left = plan.view.w > 2 * MARGIN ? MARGIN : 12;
  const f = parseFont(`12px ${b.theme.font}`);
  const caption =
    `<text x="${left}" y="${plan.view.h + CAPTION_H / 2}" font-family="${escapeXml(f.family)}" font-size="${f.size}" dominant-baseline="central" ` +
    `fill="${escapeXml(b.theme.ink2)}">${escapeXml(b.caption)}</text>\n`;
  return new TextEncoder().encode(ctx.toSvg(b.theme.paper, plan.view, caption));
}

/**
 * Asks where to save (PNG or SVG), builds and paints the picture, writes it and tells the user. `setBusy` lets the chart
 * show "Saving the picture…" while it works; the work waits for a frame first so that message can appear.
 */
export async function savePicture(
  deps: { api: Api; platform: Platform },
  args: { fileName: string; build: (format: PictureFormat) => Promise<BuiltPicture | null>; setBusy: (busy: boolean) => void },
): Promise<void> {
  const dest = await deps.platform.pickSavePath({
    title: t('picture.dialogTitle'),
    defaultName: args.fileName,
    extensions: ['png', 'svg'],
    filters: [
      { name: t('picture.formatPng'), extensions: ['png'] },
      { name: t('picture.formatSvg'), extensions: ['svg'] },
    ],
  });
  if (!dest) return;
  const format = formatOf(dest);
  args.setBusy(true);
  try {
    await yieldFrame();
    const built = await args.build(format);
    if (!built) {
      notices.show({ message: t('picture.nothing') });
      return;
    }
    await later();
    let plan = built.plan;
    let bytes: Uint8Array;
    if (format === 'svg') {
      bytes = renderSvg(built);
    } else {
      ({ bytes, plan } = await renderPngAdaptive(built));
    }
    await deps.api.savePicture(dest, bytes);
    const scaled = format === 'png' && plan.reduced;
    const message = scaled
      ? t('picture.savedScaled', { percent: plan.percent }) + (plan.percent < 50 ? ` ${t('picture.svgHint')}` : '')
      : t('picture.saved');
    notices.show({ message, actionLabel: t('exp.reveal'), onaction: () => void deps.platform.reveal(dest) });
  } catch (e) {
    notices.show({ message: messageOf(e) });
  } finally {
    args.setBusy(false);
  }
}

import type { Api, Platform } from '../api/api';
import type { ChartKind } from '../api/types';
import { currentLocale, t, type MessageKey } from '../i18n';
import { notices } from '../ui/notices.svelte';
import { CAPTION_H, MARGIN, pictureFileName, type PicturePlan, type PictureScope } from './picture';

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

const later = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

/**
 * Asks where to save, builds and paints the picture, writes it and tells the user. `setBusy` lets the chart show
 * "Saving the picture…" while it works; the work yields to the event loop first so that message can appear.
 */
export async function savePicture(
  deps: { api: Api; platform: Platform },
  args: { fileName: string; build: () => Promise<BuiltPicture | null>; setBusy: (busy: boolean) => void },
): Promise<void> {
  const dest = await deps.platform.pickSavePath({ title: t('picture.dialogTitle'), defaultName: args.fileName, extensions: ['png'] });
  if (!dest) return;
  args.setBusy(true);
  try {
    await later();
    const built = await args.build();
    if (!built) {
      notices.show({ message: t('picture.nothing') });
      return;
    }
    await later();
    const bytes = await renderPng(built);
    await deps.api.savePng(dest, bytes);
    notices.show({
      message: built.plan.reduced ? t('picture.savedScaled', { percent: built.plan.percent }) : t('picture.saved'),
      actionLabel: t('exp.reveal'),
      onaction: () => void deps.platform.reveal(dest),
    });
  } catch (e) {
    notices.show({ message: messageOf(e) });
  } finally {
    args.setBusy(false);
  }
}

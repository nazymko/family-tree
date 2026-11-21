import type { ChartNode } from '../api/types';
import { planSheets, type PlanOptions, type SheetPlan } from './sheetPlan';
import type { DirScene } from './pieceView';

/**
 * A sheets plan as data a worker can take: `PlanOptions` with the caption widths measured beforehand (a worker has no
 * fonts of the page), for each node as a branch's top and as the one brothers and sisters belong with.
 */
export interface SheetJob {
  dirs: DirScene[];
  options: Omit<PlanOptions, 'capW'>;
  capTop: Map<number, number>;
  capSiblings: Map<number, number>;
}

/** The job's plan: `planSheets` with the measured caption widths. */
export function runSheetJob(job: SheetJob): SheetPlan {
  const capW = (top: ChartNode, siblingsOf: ChartNode | null) => (siblingsOf ? job.capSiblings.get(siblingsOf.id) : job.capTop.get(top.id)) ?? 0;
  return planSheets(job.dirs, { ...job.options, capW });
}

/** Measures every caption a plan of `dirs` may ask for (the cut is made on `screenDir`'s scene). */
export function sheetJob(dirs: DirScene[], options: Omit<PlanOptions, 'capW'>, capW: PlanOptions['capW']): SheetJob {
  const cut = dirs.find((d) => d.direction === options.screenDir) ?? dirs[0];
  const capTop = new Map<number, number>();
  const capSiblings = new Map<number, number>();
  for (const n of cut?.scene.nodes ?? []) {
    capTop.set(n.id, capW(n, null));
    if (n.person) capSiblings.set(n.id, capW(n, n));
  }
  return { dirs: dirs.map((d) => ({ direction: d.direction, scene: d.scene, keys: d.keys })), options, capTop, capSiblings };
}

/**
 * Plans off the main thread (the plan of a big chart takes a second or more): in a worker where the page has workers,
 * else (tests, or a worker that failed to start) right here.
 */
export function planSheetsAsync(job: SheetJob): Promise<SheetPlan> {
  if (typeof Worker === 'undefined') return Promise.resolve(runSheetJob(job));
  return new Promise((resolve) => {
    let w: Worker;
    try {
      w = new Worker(new URL('./sheetPlan.worker.ts', import.meta.url), { type: 'module' });
    } catch {
      resolve(runSheetJob(job));
      return;
    }
    const done = (plan: SheetPlan) => {
      w.terminate();
      resolve(plan);
    };
    w.onmessage = (e: MessageEvent<SheetPlan>) => done(e.data);
    w.onerror = () => done(runSheetJob(job));
    w.postMessage(job);
  });
}

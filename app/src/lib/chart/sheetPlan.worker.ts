import { runSheetJob, type SheetJob } from './sheetPlanJob';

/** Plans one "sheets by branch" set off the main thread (see `planSheetsAsync`). */
self.onmessage = (e: MessageEvent<SheetJob>) => {
  self.postMessage(runSheetJob(e.data));
};

import type { ImportReportDto, ProjectInfo } from '../api/types';

class ProjectState {
  info = $state<ProjectInfo | null>(null);
  /** The report of the import that created the open project; cleared once the user opens the tree. */
  report = $state<ImportReportDto | null>(null);
  /** 'saving' while an edit is in flight. Edits persist immediately, so this is normally 'saved'. */
  save = $state<'saved' | 'saving' | 'failed'>('saved');
  /** Bumped after every apply, undo and redo; views that load data re-run when it changes. */
  rev = $state(0);

  open(info: ProjectInfo, report: ImportReportDto | null = null): void {
    this.info = info;
    this.report = report;
    this.save = 'saved';
  }

  close(): void {
    this.info = null;
    this.report = null;
    this.save = 'saved';
  }
}

export const project = new ProjectState();

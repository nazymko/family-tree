import type { Api, StartupOutcome } from '../api/api';
import { enterProject } from './openTree';

/** The message shown on the start screen when the last tree could not be reopened at startup. */
class StartupState {
  notice = $state<StartupOutcome['notice']>(null);
}

export const startup = new StartupState();

/** Asks the backend what the start-up auto-open did and shows the tree, or the message on the start screen. */
export async function runStartup(api: Api): Promise<void> {
  let outcome: StartupOutcome;
  try {
    outcome = await api.startupOutcome();
  } catch {
    return; // an older backend or no backend: the start screen is the fallback
  }
  if (outcome.opened) {
    await enterProject(api, outcome.opened);
  } else {
    startup.notice = outcome.notice;
  }
}

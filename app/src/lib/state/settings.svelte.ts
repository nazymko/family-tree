import type { Api, Settings } from '../api/api';
import { applyTheme } from './theme';
import { project } from './project.svelte';

const DEFAULTS: Settings = {
  dayOrder: 'dmy',
  nameOrder: 'given-first',
  theme: 'system',
  backupsToKeep: 10,
  livingMaxAge: 100,
  homePerson: null,
};

class SettingsStore {
  value = $state<Settings>({ ...DEFAULTS });
  loaded = $state(false);

  async load(api: Api): Promise<Settings> {
    this.#adopt(await api.getSettings());
    return this.value;
  }

  async update(api: Api, patch: Partial<Settings>): Promise<Settings> {
    this.#adopt(await api.setSettings(patch));
    return this.value;
  }

  #adopt(s: Settings): void {
    this.value = s;
    this.loaded = true;
    applyTheme(s.theme);
    // The ribbon, the Home shortcut and the People marker read the home person from the open project.
    if (project.info && project.info.homePerson !== s.homePerson) project.info = { ...project.info, homePerson: s.homePerson };
  }
}

export const settings = new SettingsStore();

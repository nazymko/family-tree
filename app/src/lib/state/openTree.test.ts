import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import type { Settings } from '../api/api';
import { closeTree, enterProject } from './openTree';
import { nav } from './nav.svelte';
import { project } from './project.svelte';
import { settings } from './settings.svelte';

/** Answers `getSettings` differently before and after `openProject`, like the real per-project backend. */
class PerProjectApi extends MockApi {
  opened = false;
  async getSettings(): Promise<Settings> {
    const s = await super.getSettings();
    return this.opened ? { ...s, livingMaxAge: 110 } : { ...s, livingMaxAge: 100, homePerson: null };
  }
  async openProject(dir: string) {
    this.opened = true;
    return super.openProject(dir);
  }
  async closeProject() {
    this.opened = false;
    return super.closeProject();
  }
}

beforeEach(() => {
  nav.reset();
  project.close();
});

describe('openTree', () => {
  it('opening_a_project_reloads_its_settings_and_home_person', async () => {
    const api = new PerProjectApi(fixtureTree(50));
    await settings.load(api);
    expect(settings.value.livingMaxAge).toBe(100);
    expect(settings.value.homePerson).toBeNull();

    await enterProject(api, await api.openProject('C:/Trees/A'));
    expect(settings.value.livingMaxAge).toBe(110);
    expect(settings.value.homePerson).not.toBeNull();
    expect(project.info?.homePerson).toBe(settings.value.homePerson);
  });

  it('saving_a_new_home_person_moves_the_project_home_without_a_restart', async () => {
    const api = new MockApi(fixtureTree(50));
    await enterProject(api, await api.openProject('C:/Trees/A'));
    const other = (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 3, 1)).items[0]!.id;
    expect(project.info?.homePerson).not.toBe(other);
    await settings.update(api, { homePerson: other });
    expect(project.info?.homePerson).toBe(other);
    await settings.update(api, { homePerson: null });
    expect(project.info?.homePerson).toBeNull();
  });

  it('closing_the_tree_releases_it_and_returns_to_welcome', async () => {
    const api = new MockApi(fixtureTree(50));
    const spy = vi.spyOn(api, 'closeProject');
    await enterProject(api, await api.openProject('C:/Trees/A'));
    expect(nav.route.name).toBe('family');
    await closeTree(api);
    expect(spy).toHaveBeenCalledOnce();
    expect(project.info).toBeNull();
    expect(nav.route.name).toBe('welcome');
  });
});

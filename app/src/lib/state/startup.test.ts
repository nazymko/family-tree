import { beforeEach, describe, expect, it } from 'vitest';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { project } from './project.svelte';
import { runStartup, startup } from './startup.svelte';

beforeEach(() => {
  project.close();
  startup.notice = null;
});

describe('runStartup', () => {
  it('opens_the_tree_the_backend_already_opened', async () => {
    const api = new MockApi(fixtureTree(20));
    const info = await api.openProject('C:/Genealogy/Schmidt family');
    api.startupOutcome = async () => ({ opened: info, notice: null });
    await runStartup(api);
    expect(project.info?.dir).toBe(info.dir);
    expect(startup.notice).toBeNull();
  });

  it('keeps_the_start_screen_and_the_message_when_the_open_failed', async () => {
    const api = new MockApi(fixtureTree(20));
    api.startupOutcome = async () => ({ opened: null, notice: { kind: 'failed', dir: 'D:/x', message: 'locked' } });
    await runStartup(api);
    expect(project.info).toBeNull();
    expect(startup.notice?.message).toBe('locked');
  });

  it('survives_a_backend_without_the_command', async () => {
    const api = new MockApi(fixtureTree(20));
    api.startupOutcome = async () => {
      throw new Error('unknown command');
    };
    await runStartup(api);
    expect(project.info).toBeNull();
  });
});

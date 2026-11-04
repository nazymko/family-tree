import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import Shell from './Shell.svelte';
import People from './People.svelte';
import ExportDialog from './ExportDialog.svelte';
import ImportWizard from './ImportWizard.svelte';
import { renderWith } from '../testing/renderWith';
import { apiContext } from '../api/context';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { PrivacyMockApi } from '../api/mock/mockPrivacy';
import { fixtureTree } from '../api/mock/fixtures';
import { observeApi } from '../state/observe';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import { toasts } from '../state/toast.svelte';
import { setLanguage } from '../i18n';

// The test setup forces English before each test; these tests switch to Ukrainian on purpose.
beforeEach(() => {
  toasts.clear();
  project.close();
  setLanguage('uk');
});

async function open(view: 'family' | 'people') {
  const api = observeApi(new MockApi(fixtureTree(60)));
  const info = await api.openProject('C:/Genealogy/Test');
  project.open(info);
  nav.reset();
  nav.go({ name: view });
  if (view === 'family') nav.focus(info.homePerson);
  return { api, ctx: apiContext(api, new MockPlatform()) };
}

describe('Ukrainian interface', () => {
  it('shell_navigation_uses_genealogy_terms', async () => {
    const { ctx } = await open('family');
    render(Shell, { context: ctx });
    for (const name of ['Родовід', 'Нащадки', 'Джерела', 'Налаштування']) {
      expect((await screen.findAllByText(name)).length).toBeGreaterThan(0);
    }
    expect(screen.queryByText('Pedigree')).toBeNull();
  });

  it('people_list_is_labelled_in_ukrainian', async () => {
    const { ctx } = await open('people');
    render(People, { context: ctx });
    expect(await screen.findByLabelText('Фільтр за іменем')).toBeInTheDocument();
    const box = await screen.findByRole('listbox', { name: 'Особи' });
    await waitFor(() => expect(within(box).queryAllByRole('option').length).toBeGreaterThan(0));
  });

  it('export_dialog_is_in_ukrainian', async () => {
    renderWith(ExportDialog, { onClose: () => {} }, { api: new PrivacyMockApi(fixtureTree(30)), platform: new MockPlatform() });
    expect(await screen.findByRole('dialog', { name: 'Експорт' })).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Закрити' }).length).toBe(2);
    expect(screen.getByRole('region', { name: 'Попередній перегляд приватності' })).toBeInTheDocument();
  });

  it('import_wizard_is_in_ukrainian', async () => {
    renderWith(ImportWizard, { mode: 'import', oncancel: () => {} });
    expect(screen.getByRole('heading', { name: 'Імпорт файлу родинного дерева' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Скасувати' })).toBeInTheDocument();
    expect(screen.getByText('Файл не вибрано')).toBeInTheDocument();
  });
});

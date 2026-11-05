import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import Host from '../chart/testing/Host.svelte';
import { attachContext, fakeRaf, makeApi, richestRoot } from '../chart/testing/helpers';
import { apiContext } from '../api/context';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import { nav } from '../state/nav.svelte';
import { project } from '../state/project.svelte';
import TraceView from '../trace/TraceView.svelte';
import Relatives from './Relatives.svelte';
import WholeTree from './WholeTree.svelte';
import { setLanguage } from '../i18n';

// The test setup forces English before each test; these tests switch to Ukrainian on purpose.
beforeEach(() => {
  nav.reset();
  setLanguage('uk');
});
afterEach(() => vi.unstubAllGlobals());

describe('Ukrainian screens of the newer views', () => {
  it('relatives_chart_is_in_ukrainian', async () => {
    const { api, fx } = makeApi();
    nav.focus(await richestRoot(api, fx, 'relatives'));
    fakeRaf();
    render(Host, { props: { api, view: Relatives } });
    expect(screen.getByRole('heading', { name: 'Родичі' })).toBeInTheDocument();
    const canvas = (await screen.findByRole('application', { name: 'Діаграма родичів' })) as HTMLCanvasElement;
    attachContext(canvas);
    expect(screen.getByRole('toolbar', { name: 'Інструменти діаграми родичів' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Центрувати на мені' })).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: 'Показати дядьків і тіток' })).toBeInTheDocument();
    expect(screen.queryByText('Centre on me')).toBeNull();
  });

  it('whole_tree_is_in_ukrainian', async () => {
    fakeRaf();
    const { api } = makeApi(120);
    render(Host, { props: { api, view: WholeTree } });
    expect(screen.getByRole('heading', { name: 'Увесь родовід' })).toBeInTheDocument();
    attachContext((await screen.findByRole('application', { name: 'Діаграма всього родоводу' })) as HTMLCanvasElement);
    expect(await screen.findByText(/осіб у \d+ групах|осіб у 1 групі/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Вписати все' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Показати в родоводі' })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'Знайти особу…' })).toBeInTheDocument();
  });

  it('trace_screen_is_in_ukrainian', async () => {
    const api = new MockApi(fixtureTree(40));
    project.open(await api.openProject('C:/Genealogy/Test'));
    render(TraceView, { context: apiContext(api, new MockPlatform()) });
    expect(await screen.findByRole('heading', { name: 'Відтворення родоводу зі світлини' })).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Вибрати світлину…' })).toBeInTheDocument();
    expect(screen.queryByText('Choose a picture')).toBeNull();
  });

  it('trace_review_panel_and_card_form_are_in_ukrainian', async () => {
    const api = new MockApi(fixtureTree(40));
    project.open(await api.openProject('C:/Genealogy/Test'));
    await api.stageTraceImage('C:/Mock/wall chart.png');
    render(TraceView, { context: apiContext(api, new MockPlatform()) });
    expect(await screen.findByRole('complementary', { name: 'Перевірка' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Додати до родоводу' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Прочитати імена за допомогою ШІ…' })).toBeInTheDocument();
    expect(screen.getByText('Обведіть рамкою ім’я на світлині або клацніть, щоб поставити позначку.')).toBeInTheDocument();
  });
});

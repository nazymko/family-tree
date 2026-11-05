import { afterEach, describe, expect, it } from 'vitest';
import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import AgentAccessSettings from './AgentAccessSettings.svelte';
import { renderWith } from '../testing/renderWith';
import { setLanguage } from '../i18n';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';

afterEach(() => setLanguage('en'));

const WARNING =
  'A connected agent can read everything in the open tree, including living relatives, private notes and pictures. An agent may send what it reads to its AI provider.';
const TOGGLE = 'Let agents on this computer connect';

function mount(api = new MockApi(fixtureTree(20))) {
  renderWith(AgentAccessSettings, {}, { api });
  return api;
}

async function createConnection(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.type(screen.getByRole('textbox', { name: 'Name for a new connection' }), name);
  await user.click(screen.getByRole('button', { name: 'Create connection' }));
  return screen.findByRole('dialog');
}

describe('AgentAccessSettings', () => {
  it('off_by_default_warning_visible', async () => {
    mount();
    const toggle = await screen.findByRole('checkbox', { name: TOGGLE });
    expect(toggle).not.toBeChecked();
    expect(screen.getByText(WARNING)).toBeInTheDocument();
    expect(screen.getByText('Not running.')).toBeInTheDocument();
  });

  it('turning_on_starts_the_server_on_the_port', async () => {
    const user = userEvent.setup();
    const api = mount();
    await user.click(await screen.findByRole('checkbox', { name: TOGGLE }));
    expect(await screen.findByText('Listening on 127.0.0.1:8765.')).toBeInTheDocument();
    expect((await api.agentAccessState()).running).toBe(true);
  });

  it('create_shows_key_once', async () => {
    const user = userEvent.setup();
    const api = mount();
    await screen.findByRole('checkbox', { name: TOGGLE });
    const dialog = await createConnection(user, 'Claude on the laptop');
    const key = within(dialog).getByTestId('agent-key').textContent ?? '';
    expect(key).toMatch(/^gt_[0-9a-f]+$/);
    expect(within(dialog).getByTestId('agent-snippet').textContent).toContain(`Bearer ${key}`);
    expect(within(dialog).getByText(/shown once/)).toBeInTheDocument();

    await user.click(within(dialog).getByRole('button', { name: 'Copy key' }));
    expect(await navigator.clipboard.readText()).toBe(key);
    expect(await within(dialog).findByText('Copied')).toBeInTheDocument();

    await fireEvent.mouseDown(dialog.parentElement!);
    expect(screen.getByTestId('agent-key').textContent).toBe(key);
    await user.click(within(dialog).getByRole('button', { name: 'Done' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.body.textContent).not.toContain(key);
    expect(screen.getByText('Claude on the laptop')).toBeInTheDocument();
    expect(JSON.stringify(await api.agentAccessState())).not.toContain(key);
  });

  it('revoke_removes_row_after_confirm', async () => {
    const user = userEvent.setup();
    const api = mount();
    await screen.findByRole('checkbox', { name: TOGGLE });
    const made = await createConnection(user, 'Old agent');
    await user.click(within(made).getByRole('button', { name: 'Done' }));

    await user.click(screen.getByRole('button', { name: 'Revoke Old agent' }));
    const confirm = await screen.findByRole('dialog');
    // Cancelling keeps the row.
    await user.click(within(confirm).getByRole('button', { name: 'Cancel' }));
    expect(screen.getByText('Old agent')).toBeInTheDocument();
    expect((await api.agentAccessState()).connections).toHaveLength(1);

    await user.click(screen.getByRole('button', { name: 'Revoke Old agent' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Revoke access' }));
    await waitFor(() => expect(screen.queryByText('Old agent')).toBeNull());
    expect((await api.agentAccessState()).connections).toEqual([]);
    // The focused button went away with the row: focus must stay in the screen, not fall to the body.
    await waitFor(() => expect(document.activeElement).not.toBe(document.body));
    expect(screen.getByRole('heading', { name: 'Connections' }).closest('section')?.contains(document.activeElement)).toBe(true);
  });

  it('out_of_range_port_shows_the_corrected_value', async () => {
    const user = userEvent.setup();
    const api = mount();
    await screen.findByRole('checkbox', { name: TOGGLE });
    const port = screen.getByRole('spinbutton', { name: 'Port' });
    await user.clear(port);
    await user.type(port, '80');
    await user.tab();
    await waitFor(() => expect(port).toHaveValue(1024));
    expect((await api.agentAccessState()).port).toBe(1024);
    await user.clear(port);
    await user.type(port, '70000');
    await user.tab();
    await waitFor(() => expect(port).toHaveValue(65535));
  });

  it('port_error_shown', async () => {
    const user = userEvent.setup();
    const api = mount();
    api.agentTakenPorts.add(9100);
    await screen.findByRole('checkbox', { name: TOGGLE });
    const port = screen.getByRole('spinbutton', { name: 'Port' });
    await user.clear(port);
    await user.type(port, '9100');
    await user.tab();
    await user.click(screen.getByRole('checkbox', { name: TOGGLE }));
    expect(await screen.findByText('Port 9100 is already used by another program. Choose another port.')).toBeInTheDocument();
    expect(screen.getByText('Not running.')).toBeInTheDocument();
  });

  it('rules_readable', async () => {
    const user = userEvent.setup();
    const api = mount();
    await user.click(await screen.findByRole('button', { name: 'Rules agents receive' }));
    const dialog = await screen.findByRole('dialog', { name: 'Rules agents receive' });
    const text = await api.agentRules();
    await waitFor(() => expect(within(dialog).getByTestId('agent-rules').textContent).toBe(text));
    expect(within(dialog).queryByRole('textbox')).toBeNull();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('ukrainian_strings', async () => {
    setLanguage('uk');
    mount();
    expect(await screen.findByRole('heading', { name: 'Доступ для агентів' })).toBeInTheDocument();
    expect(
      screen.getByText(
        'Підключений агент може читати все у відкритому родоводі, зокрема дані живих родичів, приватні нотатки й фото. Агент може надсилати прочитане своєму постачальнику ШІ.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Правила, які отримують агенти' })).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/дерев/i);
  });
});

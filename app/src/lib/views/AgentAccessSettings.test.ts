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

const REQUIRE = 'Require a key';
const NO_KEY_WARNING = 'Without a key, any program on this computer can read and change the family tree. Web pages have no access.';
const KEYLESS = 'claude mcp add --transport http gene-tree http://127.0.0.1:8765/mcp';

describe('AgentAccessSettings', () => {
  it('no_key_is_required_by_default_with_a_plain_warning_and_the_keyless_command', async () => {
    mount();
    const switchEl = await screen.findByRole('checkbox', { name: REQUIRE });
    expect(switchEl).not.toBeChecked();
    expect(await screen.findByText(NO_KEY_WARNING)).toBeInTheDocument();
    expect((await screen.findByTestId('agent-keyless-command')).textContent).toBe(KEYLESS);
    // Access itself is still off.
    expect(screen.getByRole('checkbox', { name: TOGGLE })).not.toBeChecked();
    expect(screen.getByText('Not running.')).toBeInTheDocument();
  });

  it('requiring_a_key_hides_the_warning_and_the_keyless_command_and_turning_it_off_brings_them_back', async () => {
    const user = userEvent.setup();
    const api = mount();
    await user.click(await screen.findByRole('checkbox', { name: REQUIRE }));
    await waitFor(() => expect(screen.queryByText(NO_KEY_WARNING)).toBeNull());
    expect(screen.queryByTestId('agent-keyless-command')).toBeNull();
    expect((await api.agentAccessState()).requireKey).toBe(true);

    await user.click(screen.getByRole('checkbox', { name: REQUIRE }));
    expect(await screen.findByText(NO_KEY_WARNING)).toBeInTheDocument();
    expect(screen.getByTestId('agent-keyless-command').textContent).toBe(KEYLESS);
    expect((await api.agentAccessState()).requireKey).toBe(false);
  });

  it('the_connections_stay_usable_without_a_key_and_a_new_one_still_has_its_key', async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole('checkbox', { name: REQUIRE });
    expect(screen.getByText(/gives an agent its own name in the log of changes/)).toBeInTheDocument();
    const dialog = await createConnection(user, 'Claude on the laptop');
    const key = within(dialog).getByTestId('agent-key').textContent ?? '';
    expect(within(dialog).getByTestId('agent-snippet').textContent).toContain(`Bearer ${key}`);
  });

  it('the_switch_warning_and_command_read_in_ukrainian', async () => {
    setLanguage('uk');
    mount();
    expect(await screen.findByRole('checkbox', { name: 'Вимагати ключ' })).not.toBeChecked();
    expect(
      await screen.findByText('Без ключа будь-яка програма на цьому комп’ютері може читати й змінювати родовід. Вебсторінки доступу не мають.'),
    ).toBeInTheDocument();
    expect((await screen.findByTestId('agent-keyless-command')).textContent).toBe(KEYLESS);
    expect(document.body.textContent).not.toContain('дерево');
  });

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

  it('port_error_reads_in_ukrainian', async () => {
    setLanguage('uk');
    const user = userEvent.setup();
    const api = mount();
    api.agentTakenPorts.add(9100);
    await screen.findByRole('checkbox', { name: 'Дозволити агентам на цьому комп’ютері підключатися' });
    const port = screen.getByRole('spinbutton', { name: 'Порт' });
    await user.clear(port);
    await user.type(port, '9100');
    await user.tab();
    await user.click(screen.getByRole('checkbox', { name: 'Дозволити агентам на цьому комп’ютері підключатися' }));
    expect(await screen.findByText('Порт 9100 вже використовує інша програма. Виберіть інший порт.')).toBeInTheDocument();
  });

  it('moving_to_a_taken_port_keeps_the_working_server', async () => {
    const user = userEvent.setup();
    const api = mount();
    api.agentTakenPorts.add(9100);
    await user.click(await screen.findByRole('checkbox', { name: TOGGLE }));
    await screen.findByText('Listening on 127.0.0.1:8765.');
    const port = screen.getByRole('spinbutton', { name: 'Port' });
    await user.clear(port);
    await user.type(port, '9100');
    await user.tab();
    expect(await screen.findByText('Port 9100 is already used by another program. Choose another port.')).toBeInTheDocument();
    expect(screen.getByText('Listening on 127.0.0.1:8765.')).toBeInTheDocument();
    expect(port).toHaveValue(8765);
  });

  it('failed_revoke_shows_the_state_and_can_be_tried_again', async () => {
    setLanguage('uk');
    const user = userEvent.setup();
    const api = mount();
    await screen.findByRole('heading', { name: 'Доступ для агентів' });
    const made = await api.agentConnectionCreate('Reader');
    const text = 'Access stopped for now, but this could not be recorded; it may return after restart. Try again.';
    const translated = 'Доступ поки що зупинено, але цього не вдалося записати; після перезапуску він може повернутися. Спробуйте ще раз.';
    const realRevoke = api.agentConnectionRevoke.bind(api);
    const realState = api.agentAccessState.bind(api);
    let failing = true;
    api.agentConnectionRevoke = async (id) => {
      if (failing) throw { code: 'io', message: text };
      return realRevoke(id);
    };
    api.agentAccessState = async () => {
      const state = await realState();
      return failing ? { ...state, error: text } : state;
    };

    // The row came in after this screen first read the state.
    const name = screen.getByRole('textbox', { name: 'Назва нового підключення' });
    await user.type(name, 'Second');
    await user.click(screen.getByRole('button', { name: 'Створити підключення' }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Готово' }));

    await user.click(screen.getByRole('button', { name: `Скасувати доступ: ${made.connection.name}` }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Скасувати доступ' }));

    // Once, though both the answer and the refreshed state say it.
    await waitFor(() => expect(screen.getAllByText(translated)).toHaveLength(1));
    expect(screen.getByText(made.connection.name)).toBeInTheDocument();
    expect(document.body.textContent).not.toContain(text);

    failing = false;
    await user.click(screen.getByRole('button', { name: `Скасувати доступ: ${made.connection.name}` }));
    await user.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Скасувати доступ' }));
    await waitFor(() => expect(screen.queryByText(made.connection.name)).toBeNull());
    expect(screen.queryByText(translated)).toBeNull();
  });

  it('name_rules_answer_in_the_language_of_the_screen', async () => {
    setLanguage('uk');
    const user = userEvent.setup();
    const api = mount();
    await screen.findByRole('heading', { name: 'Доступ для агентів' });
    await api.agentConnectionCreate('Reader');
    await user.type(screen.getByRole('textbox', { name: 'Назва нового підключення' }), 'reader');
    await user.click(screen.getByRole('button', { name: 'Створити підключення' }));
    expect(await screen.findByText('Підключення з назвою «reader» вже є. Виберіть іншу назву.')).toBeInTheDocument();
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

describe('MockApi connection names', () => {
  const refusal = (api: MockApi, name: string) =>
    api.agentConnectionCreate(name).then(
      () => 'made',
      (e: { message: string }) => e.message,
    );

  it('follows_the_backend_rules', async () => {
    const api = new MockApi(fixtureTree(1));
    expect((await api.agentConnectionCreate('  Reader  ')).connection.name).toBe('Reader');
    expect(await refusal(api, 'x'.repeat(101))).toBe('Keep the connection name to 100 characters or fewer.');
    expect(await refusal(api, 'я'.repeat(100))).toBe('made');
    expect(await refusal(api, 'Two\nlines')).toBe('The connection name cannot contain line breaks or other control characters.');
    expect(await refusal(api, ' READER ')).toBe('A connection named "READER" already exists. Choose another name.');
    expect(await refusal(api, '   ')).toBe('Give the connection a name.');
  });
});

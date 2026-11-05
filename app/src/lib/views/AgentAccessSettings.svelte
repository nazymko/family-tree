<script lang="ts">
  import { tick } from 'svelte';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import type { AgentAccessState, ConnectionDto, NewConnectionDto } from '../api/api';
  import { currentLocale, t } from '../i18n';
  import { agentErrorText } from '../i18n/backend';

  const api = getApi();

  let agent = $state<AgentAccessState | null>(null);
  let enabled = $state(false);
  let port = $state('8765');
  let error = $state('');
  let busy = $state(false);
  let name = $state('');

  // Held only while the "created" dialog is open. Closing it forgets the key for good.
  let fresh = $state<NewConnectionDto | null>(null);
  let copied = $state<'key' | 'snippet' | 'failed' | null>(null);
  let revoking = $state<ConnectionDto | null>(null);
  let rulesOpen = $state(false);
  let rules = $state<string | null>(null);
  let connectionsHeading = $state<HTMLElement>();

  function take(next: AgentAccessState) {
    agent = next;
    enabled = next.enabled;
    port = String(next.port);
  }

  $effect(() => {
    let live = true;
    api
      .agentAccessState()
      .then((s) => live && take(s))
      .catch(() => live && (error = t('agent.loadFailed')));
    return () => {
      live = false;
    };
  });

  const raw = (e: unknown) => (e instanceof Error ? e.message : typeof e === 'object' && e && 'message' in e ? String((e as { message: unknown }).message) : String(e));
  const message = (e: unknown) => agentErrorText(raw(e));

  async function apply(nextEnabled: boolean, nextPort: number) {
    error = '';
    busy = true;
    try {
      take(await api.agentAccessSet(nextEnabled, nextPort));
    } catch (e) {
      error = message(e);
      if (agent) take(agent);
    } finally {
      busy = false;
    }
  }

  function commitPort() {
    if (!agent) return;
    const n = Math.round(Number(port));
    const ok = String(port ?? '').trim() !== '' && Number.isFinite(n);
    const next = ok ? Math.min(65535, Math.max(1024, n)) : agent.port;
    port = String(next);
    if (next !== agent.port) void apply(agent.enabled, next);
  }

  async function create() {
    const clean = name.trim();
    if (!clean || busy) return;
    error = '';
    busy = true;
    try {
      const made = await api.agentConnectionCreate(clean);
      name = '';
      copied = null;
      fresh = made;
      take(await api.agentAccessState());
    } catch (e) {
      error = message(e);
    } finally {
      busy = false;
    }
  }

  function closeFresh() {
    fresh = null;
    copied = null;
  }

  async function copy(what: 'key' | 'snippet') {
    if (!fresh) return;
    try {
      await navigator.clipboard.writeText(what === 'key' ? fresh.key : fresh.snippet);
      copied = what;
    } catch {
      copied = 'failed';
    }
  }

  async function revoke() {
    const target = revoking;
    if (!target) return;
    error = '';
    try {
      await api.agentConnectionRevoke(target.id);
      take(await api.agentAccessState());
    } catch (e) {
      error = message(e);
      // Part of it may have worked, or the connection may be gone: show what is true now.
      await api.agentAccessState().then(take, () => {});
    }
    revoking = null;
    // The button that opened the dialog is gone with its row: put focus on a stable spot in this screen.
    await tick();
    connectionsHeading?.focus();
  }

  function openRules() {
    rulesOpen = true;
    rules = null;
    void api
      .agentRules()
      .then((r) => (rules = r))
      .catch((e) => (rules = message(e)));
  }

  const dateOf = (ms: number) => new Date(ms).toLocaleDateString(currentLocale());
</script>

<section class="agent" aria-labelledby="agent-title">
  <h2 id="agent-title">{t('agent.title')}</h2>
  <p class="hint">{t('agent.intro')}</p>
  {#if error}<Banner kind="error">{error}</Banner>{/if}

  <div class="form">
    <Checkbox
      label={t('agent.enable')}
      bind:checked={enabled}
      disabled={!agent || busy}
      onchange={(on) => void apply(on, Number(port) || agent?.port || 8765)}
    />
    <Banner kind="warning">{t('agent.warning')}</Banner>
    <TextField
      label={t('agent.port')}
      type="number"
      min="1024"
      max="65535"
      hint={t('agent.portHint')}
      bind:value={port}
      onchange={commitPort}
    />
    {#if agent?.error && agentErrorText(agent.error) !== error}<Banner kind="error">{agentErrorText(agent.error)}</Banner>{/if}
    <p class="status" role="status">{agent?.running ? t('agent.listening', { port: agent.port }) : t('agent.stopped')}</p>
  </div>

  <div class="connections">
    <h3 bind:this={connectionsHeading} tabindex="-1">{t('agent.connections')}</h3>
    <p class="hint">{t('agent.connectionsHint')}</p>
    {#if agent && agent.connections.length > 0}
      <ul>
        {#each agent.connections as c (c.id)}
          <li>
            <span class="who">{c.name}</span>
            <span class="when">{t('agent.createdOn', { date: dateOf(c.created) })}</span>
            <Button aria-label={t('agent.revokeNamed', { name: c.name })} onclick={() => (revoking = c)}>{t('agent.revoke')}</Button>
          </li>
        {/each}
      </ul>
    {:else if agent}
      <p class="hint">{t('agent.none')}</p>
    {/if}
    <form
      class="new"
      onsubmit={(e) => {
        e.preventDefault();
        void create();
      }}
    >
      <TextField label={t('agent.nameLabel')} hint={t('agent.nameHint')} bind:value={name} autocomplete="off" />
      <Button type="submit" variant="primary" disabled={!name.trim() || busy}>{t('agent.create')}</Button>
    </form>
  </div>

  <div><Button onclick={openRules}>{t('agent.rules')}</Button></div>
</section>

<Dialog
  open={fresh !== null}
  title={fresh ? t('agent.keyTitle', { name: fresh.connection.name }) : ''}
  onclose={closeFresh}
  dismissOnScrim={false}
>
  {#if fresh}
    <div class="dlg">
      <Banner kind="warning">{t('agent.keyWarning')}</Banner>
      <p class="label">{t('agent.keyLabel')}</p>
      <code class="secret" data-testid="agent-key">{fresh.key}</code>
      <div><Button onclick={() => void copy('key')}>{t('agent.copyKey')}</Button></div>
      <p class="label">{t('agent.snippetLabel')}</p>
      <code class="secret" data-testid="agent-snippet">{fresh.snippet}</code>
      <div><Button onclick={() => void copy('snippet')}>{t('agent.copySnippet')}</Button></div>
      <p class="status" role="status">{copied === 'failed' ? t('agent.copyFailed') : copied ? t('agent.copied') : ''}</p>
    </div>
  {/if}
  {#snippet footer()}
    <Button variant="primary" onclick={closeFresh}>{t('agent.done')}</Button>
  {/snippet}
</Dialog>

<Dialog open={revoking !== null} title={revoking ? t('agent.revokeTitle', { name: revoking.name }) : ''} onclose={() => (revoking = null)}>
  <p>{t('agent.revokeBody')}</p>
  {#snippet footer()}
    <Button onclick={() => (revoking = null)}>{t('common.cancel')}</Button>
    <Button variant="danger" onclick={() => void revoke()}>{t('agent.revokeConfirm')}</Button>
  {/snippet}
</Dialog>

<Dialog open={rulesOpen} title={t('agent.rules')} onclose={() => (rulesOpen = false)}>
  <p class="hint">{t('agent.rulesHint')}</p>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- Focusable so the keyboard can scroll a long, read-only text. -->
  <pre class="rules" data-testid="agent-rules" tabindex="0">{rules ?? t('agent.rulesLoading')}</pre>
</Dialog>

<style>
  .agent {
    display: grid;
    gap: var(--s-5);
    padding-top: var(--s-5);
    border-top: 1px solid var(--rule);
  }
  h2,
  h3,
  p {
    margin: 0;
  }
  .form,
  .connections,
  .dlg {
    display: grid;
    gap: var(--s-4);
  }
  .hint {
    color: var(--ink-2);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-3);
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-4);
  }
  .who {
    font-weight: 600;
  }
  .when {
    color: var(--ink-2);
    flex: 1;
  }
  .new {
    display: grid;
    gap: var(--s-4);
    justify-items: start;
  }
  .label {
    font-weight: 600;
  }
  .secret,
  .rules {
    display: block;
    padding: var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
    overflow-wrap: anywhere;
    white-space: pre-wrap;
    user-select: all;
  }
  .rules {
    margin: 0;
    max-height: 50vh;
    overflow: auto;
    font: inherit;
    user-select: text;
  }
  h3:focus {
    outline: none;
  }
</style>

<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import TextField from '../ui/TextField.svelte';
  import FieldChanges from '../ui/FieldChanges.svelte';
  import LogChanges from './LogChanges.svelte';
  import { getApi, getPlatform } from '../api/context';
  import type { MergeLineDto, MergePreviewDto, MergeResultDto } from '../api/types';
  import { plural, t, type MessageKey } from '../i18n';
  import { errMessage } from '../editors/errors';
  import { nameOf, sizeText } from './mergeText';

  let {
    onClose,
    onShowChanges,
    onUndo,
    onOpenPerson,
  }: {
    onClose: () => void;
    /** Go to the change log, where the merge is listed as one batch. */
    onShowChanges: () => void;
    /** Ask whether to undo a finished merge; the caller owns that question. */
    onUndo: (batch: string, title: string) => void;
    onOpenPerson: (id: string) => void;
  } = $props();

  const api = getApi();
  const platform = getPlatform();

  let step = $state<'pick' | 'preview' | 'done'>('pick');
  let source = $state('');
  let name = $state('');
  let preview = $state<MergePreviewDto | null>(null);
  let result = $state<MergeResultDto | null>(null);
  let busy = $state(false);
  let error = $state('');
  let notice = $state('');

  async function chooseZip() {
    const p = await platform.pickFile({ title: t('merge.mergePickZip'), extensions: ['zip'] });
    if (p) choose(p);
  }
  async function chooseFolder() {
    const p = await platform.pickFolder(t('merge.mergePickFolder'));
    if (p) choose(p);
  }
  function choose(p: string) {
    source = p;
    if (!name.trim()) name = nameOf(p);
  }

  async function look(stale = false) {
    if (!source || !name.trim() || busy) return;
    busy = true;
    error = '';
    notice = stale ? t('merge.stale') : '';
    try {
      preview = await api.mergePreview(source, name.trim());
      step = 'preview';
    } catch (e) {
      error = errMessage(e);
      notice = '';
      step = 'pick';
    } finally {
      busy = false;
    }
  }

  async function apply() {
    if (!preview || busy || preview.toApply === 0) return;
    busy = true;
    error = '';
    try {
      result = await api.mergeApply(preview.token);
      preview = null;
      step = 'done';
    } catch (e) {
      if (typeof e === 'object' && e && 'code' in e && (e as { code: string }).code === 'stale') {
        busy = false;
        await look(true);
        return;
      }
      error = errMessage(e);
    } finally {
      busy = false;
    }
  }

  function close() {
    // A preview that was never applied is dropped on the backend too.
    if (step === 'preview') void api.mergeCancel().catch(() => {});
    onClose();
  }

  const SECTIONS = [
    { key: 'added', title: 'merge.added', hint: null },
    { key: 'changed', title: 'merge.changed', hint: null },
    { key: 'undecided', title: 'merge.undecided', hint: 'merge.undecidedHint' },
    { key: 'removed', title: 'merge.removed', hint: 'merge.removedHint' },
    { key: 'blocked', title: 'merge.blocked', hint: 'merge.blockedHint' },
  ] as const satisfies readonly { key: keyof MergePreviewDto; title: MessageKey; hint: MessageKey | null }[];

  const CHECKS: Record<string, MessageKey> = {
    readable_copy: 'merge.checkReadable',
    same_tree: 'merge.checkSame',
    history: 'merge.checkHistory',
  };
</script>

{#snippet line(l: MergeLineDto)}
  <li class="line">
    <LogChanges changes={[l.change]} />
    {#if l.change.op === 'update' && l.change.fields.length === 0}<p class="why">{t('merge.onlyLinks')}</p>{/if}
    {#if l.why}<p class="why">{t(`merge.why.${l.why}` as MessageKey)}</p>{/if}
    {#if l.alsoEditedHere}<p class="why">{t('merge.alsoEdited')}</p>{/if}
    {#if l.yours.length > 0}
      <p class="why">{t('merge.yours')}</p>
      <FieldChanges fields={l.yours} />
    {/if}
    {#if l.maybeSame.length > 0}
      <p class="why">{t('merge.maybeSame')}</p>
      <ul class="same">
        {#each l.maybeSame as m (m.id)}
          <li>
            <button type="button" class="link" onclick={() => onOpenPerson(m.id)}
              ><bdi dir="auto">{m.name}</bdi>{#if m.lifespan}&nbsp;<span class="tnum">{m.lifespan}</span>{/if}</button
            >
          </li>
        {/each}
      </ul>
    {/if}
    {#if l.person}
      <p><button type="button" class="link" onclick={() => onOpenPerson(l.person!)}>{t('merge.openPerson')}</button></p>
    {/if}
  </li>
{/snippet}

<Dialog open title={step === 'done' ? t('merge.doneTitle') : step === 'preview' ? t('merge.previewTitle') : t('merge.mergeTitle')} onclose={close} dismissOnScrim={false}>
  <div class="merge">
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    {#if notice}<Banner kind="warning">{notice}</Banner>{/if}

    {#if step === 'pick'}
      <p>{t('merge.mergeIntro')}</p>
      <div class="row">
        <Button onclick={chooseZip}>{t('merge.mergeChooseZip')}</Button>
        <Button onclick={chooseFolder}>{t('merge.mergeChooseFolder')}</Button>
      </div>
      {#if source}<p class="chosen"><bdi dir="auto">{t('merge.mergeChosen', { name: nameOf(source) })}</bdi></p>{/if}
      <TextField label={t('merge.mergeWhose')} hint={t('merge.mergeWhoseHint')} bind:value={name} />
      {#if busy}<p role="status">{t('merge.mergeLooking')}</p>{/if}
    {:else if step === 'preview' && preview}
      <Banner kind="info">{t('merge.banner')}</Banner>
      <ul class="checks">
        {#each preview.checks as c (c.kind)}
          <li class:bad={!c.ok}>{c.ok ? '✓' : '!'} {t(CHECKS[c.kind])}</li>
        {/each}
      </ul>
      {#if preview.history === 'none'}<Banner kind="warning">{t('merge.historyNone')}</Banner>{/if}
      {#if preview.history === 'unrelated'}<Banner kind="warning">{t('merge.historyUnrelated')}</Banner>{/if}
      {#if preview.nothingNew}<p>{t('merge.nothingNew')}</p>{/if}
      {#each SECTIONS as s (s.key)}
        {@const items = preview[s.key] as MergeLineDto[]}
        {#if items.length > 0}
          <section>
            <h3>{t(s.title)} ({items.length})</h3>
            {#if s.hint}<p class="hint">{t(s.hint)}</p>{/if}
            <ul class="lines">
              {#each items as l, i (i)}{@render line(l)}{/each}
            </ul>
          </section>
        {/if}
      {/each}
      {#if preview.leftAlone > 0}<p class="hint">{t('merge.leftAlone', { n: preview.leftAlone })}</p>{/if}
      {#if preview.photos > 0}<p class="hint">{t('merge.photos', { n: preview.photos, size: sizeText(preview.photoBytes) })}</p>{/if}
    {:else if step === 'done' && result}
      <p class="done" role="status">{t('merge.doneBody', { added: result.added, changed: result.changed, photos: result.photosCopied })}</p>
      {#if result.leftForYou > 0}<p>{plural('merge.doneLeft', result.leftForYou)}</p>{/if}
    {/if}
  </div>
  {#snippet footer()}
    {#if step === 'pick'}
      <Button onclick={close}>{t('merge.cancel')}</Button>
      <Button variant="primary" disabled={busy || !source || !name.trim()} onclick={() => look()}>{t('merge.mergeLook')}</Button>
    {:else if step === 'preview' && preview}
      <Button disabled={busy} onclick={close}>{t('merge.cancel')}</Button>
      <Button variant="primary" disabled={busy || preview.toApply === 0} onclick={apply}>
        {busy ? t('merge.applying') : preview.toApply === 0 ? t('merge.applyNothing') : t('merge.applyButton', { n: preview.toApply })}
      </Button>
    {:else if result}
      <Button onclick={() => onUndo(result!.batch, result!.name)}>{t('merge.undoMerge')}</Button>
      <Button onclick={onShowChanges}>{t('merge.showChanges')}</Button>
      <Button variant="primary" onclick={onClose}>{t('common.close')}</Button>
    {/if}
  {/snippet}
</Dialog>

<style>
  .merge {
    display: grid;
    gap: var(--s-4);
  }
  .row {
    display: flex;
    gap: var(--s-3);
    flex-wrap: wrap;
  }
  .chosen,
  .hint,
  .why {
    font-size: var(--t-sm);
    color: var(--ink-1);
    max-width: var(--measure);
    margin: 0;
  }
  .checks {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-1);
  }
  .checks .bad {
    font-weight: 600;
  }
  section {
    display: grid;
    gap: var(--s-2);
  }
  h3 {
    font-size: var(--t-base);
    font-family: var(--font-sans);
    font-weight: 600;
    margin: var(--s-3) 0 0;
  }
  .lines {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: var(--s-3);
  }
  .line {
    padding: var(--s-3);
    background: var(--paper-2);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    display: grid;
    gap: var(--s-2);
  }
  .same {
    margin: 0;
    padding-inline-start: var(--s-5);
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    color: var(--accent, inherit);
    text-decoration: underline;
    cursor: pointer;
    font: inherit;
  }
  .done {
    font-weight: 500;
  }
</style>

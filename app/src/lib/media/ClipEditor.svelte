<script lang="ts" module>
  import type { OwnerRef, PersonDetail } from '../api/api';
  import { eventKindLabel } from '../i18n/backend';
  import { t } from '../i18n';

  export interface TargetOption {
    value: string;
    label: string;
    group?: string;
  }

  export const targetKey = (o: OwnerRef): string => `${o.kind}:${o.id}`;

  export function parseTarget(key: string): OwnerRef | null {
    const [kind, id] = key.split(':');
    return kind && id && ['note', 'citation', 'event', 'person', 'family'].includes(kind) ? ({ kind, id } as OwnerRef) : null;
  }

  const firstLine = (s: string, max = 60) => {
    const line = s.split('\n')[0].trim();
    return line.length > max ? `${line.slice(0, max - 1)}…` : line;
  };

  /** The records of one person a clip can go on: the person, their notes, events, citations and families. */
  export function targetOptions(d: PersonDetail): TargetOption[] {
    const out: TargetOption[] = [{ value: targetKey({ kind: 'person', id: d.ref.id }), label: t('rec.thePerson', { name: d.ref.name }) }];
    const notes = t('rec.target.note');
    for (const n of d.notes) out.push({ value: targetKey({ kind: 'note', id: n.id }), label: firstLine(n.text) || notes, group: notes });
    const events = t('rec.target.event');
    for (const e of d.events) {
      const when = e.date?.display ? ` ${e.date.display}` : '';
      out.push({ value: targetKey({ kind: 'event', id: e.id }), label: `${eventKindLabel(e.kind, e.label)}${when}`, group: events });
      for (const n of e.notes) out.push({ value: targetKey({ kind: 'note', id: n.id }), label: `${eventKindLabel(e.kind, e.label)}: ${firstLine(n.text, 40)}`, group: notes });
    }
    const cites = t('rec.target.citation');
    const cite = (c: PersonDetail['citations'][number]) => ({
      value: targetKey({ kind: 'citation', id: c.id }),
      label: [c.source?.title ?? t('rec.untitledSource'), c.page].filter(Boolean).join(', '),
      group: cites,
    });
    for (const c of d.citations) out.push(cite(c));
    for (const e of d.events) for (const c of e.citations) out.push({ ...cite(c), label: `${eventKindLabel(e.kind, e.label)}: ${cite(c).label}` });
    const fams = t('rec.target.family');
    for (const p of d.partnerships) out.push({ value: targetKey({ kind: 'family', id: p.familyId }), label: p.partner?.name ?? fams, group: fams });
    for (const s of d.parentSets) out.push({ value: targetKey({ kind: 'family', id: s.familyId }), label: s.parents.flatMap((x) => (x ? [x.name] : [])).join(', ') || fams, group: fams });
    return out;
  }
</script>

<script lang="ts">
  import Button from '../ui/Button.svelte';
  import IconButton from '../ui/IconButton.svelte';
  import TextField from '../ui/TextField.svelte';
  import Select from '../ui/Select.svelte';
  import Banner from '../ui/Banner.svelte';
  import PersonPicker from '../editors/PersonPicker.svelte';
  import { getApi } from '../api/context';
  import type { Id, MediaDto, PersonRef, RecordingClipDto } from '../api/api';
  import { errMessage } from '../editors/errors';
  import { clockMs, parseClock } from './clips';

  let {
    media,
    durationMs,
    now,
    editing,
    person: startPerson,
    onListen,
    onDone,
    onCancel,
  }: {
    media: MediaDto;
    durationMs: number | null;
    /** Where the player is, in milliseconds. */
    now: () => number;
    /** The clip being changed; absent for a new one. */
    editing?: RecordingClipDto;
    /** Whose records to offer first (the page the player was opened from). */
    person?: PersonRef | null;
    onListen: (startMs: number, endMs: number) => void;
    onDone: () => void;
    onCancel: () => void;
  } = $props();

  const api = getApi();
  const uid = $props.id();
  // svelte-ignore state_referenced_locally
  const at = editing ? editing.clip.startMs : now();
  let startText = $state(clockMs(at, true));
  // svelte-ignore state_referenced_locally
  let endText = $state(clockMs(editing ? editing.clip.endMs : Math.min(at + 30_000, durationMs ?? at + 30_000), true));
  // svelte-ignore state_referenced_locally
  let label = $state(editing?.clip.label ?? '');
  // svelte-ignore state_referenced_locally
  let person = $state<PersonRef | null>(editing?.person ?? startPerson ?? null);
  let detail = $state<PersonDetail | null>(null);
  // svelte-ignore state_referenced_locally
  let target = $state(editing ? targetKey(editing.target) : '');
  let error = $state('');
  let saving = $state(false);
  let seq = 0;

  $effect(() => {
    const p = person;
    const mine = ++seq;
    detail = null;
    if (!p) return;
    void api.getPerson(p.id).then(
      (d) => {
        if (mine === seq) detail = d;
      },
      () => {},
    );
  });

  const options = $derived.by(() => {
    const list = detail ? targetOptions(detail) : [];
    // The record a clip is on stays choosable even when it is not one of this person's.
    if (editing && !list.some((o) => o.value === targetKey(editing.target)))
      list.unshift({ value: targetKey(editing.target), label: editing.title ?? t(`rec.target.${editing.target.kind as 'note'}`) });
    return [{ value: '', label: t('rec.pickRecord'), disabled: true }, ...list];
  });

  const startMs = $derived(parseClock(startText));
  const endMs = $derived(parseClock(endText));
  const timeError = $derived(
    startMs === null || endMs === null
      ? t('rec.badTime')
      : endMs <= startMs
        ? t('rec.endAfterStart')
        : durationMs !== null && endMs > durationMs
          ? t('rec.pastEnd', { time: clockMs(durationMs) })
          : '',
  );

  function nudge(which: 'start' | 'end', by: number) {
    const v = which === 'start' ? startMs : endMs;
    if (v === null) return;
    const next = clockMs(Math.max(0, Math.min(v + by, durationMs ?? Number.MAX_SAFE_INTEGER)), true);
    if (which === 'start') startText = next;
    else endText = next;
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    const to = parseTarget(target);
    if (timeError || startMs === null || endMs === null) {
      error = timeError || t('rec.badTime');
      return;
    }
    if (!to) {
      error = t('rec.pickTarget');
      return;
    }
    saving = true;
    try {
      const text = label.trim();
      if (editing) {
        const moved = targetKey(to) !== targetKey(editing.target);
        await api.apply({
          type: 'updateAudioClip',
          target: editing.target,
          clip: editing.clip.id as Id,
          startMs,
          endMs,
          label: text,
          ...(moved ? { moveTo: to } : {}),
        });
      } else {
        await api.apply({ type: 'addAudioClip', target: to, media: media.id, startMs, endMs, ...(text ? { label: text } : {}) });
      }
      onDone();
    } catch (err) {
      error = errMessage(err);
    } finally {
      saving = false;
    }
  }
</script>

<form class="editor" aria-labelledby="{uid}-h" onsubmit={save} data-testid="clip-editor">
  <h3 id="{uid}-h">{editing ? t('rec.editClip') : t('rec.newClip')}</h3>
  <div class="times">
    <div class="time">
      <TextField label={t('rec.start')} bind:value={startText} dir="ltr" inputmode="decimal" autocomplete="off" />
      <div class="nudge" dir="ltr">
        <IconButton icon="chevron-left" label={t('rec.startEarlier')} onclick={() => nudge('start', -1000)} />
        <Button onclick={() => (startText = clockMs(now(), true))}>{t('rec.markStart')}</Button>
        <IconButton icon="chevron-right" label={t('rec.startLater')} onclick={() => nudge('start', 1000)} />
      </div>
    </div>
    <div class="time">
      <TextField label={t('rec.end')} bind:value={endText} dir="ltr" inputmode="decimal" autocomplete="off" />
      <div class="nudge" dir="ltr">
        <IconButton icon="chevron-left" label={t('rec.endEarlier')} onclick={() => nudge('end', -1000)} />
        <Button onclick={() => (endText = clockMs(now(), true))}>{t('rec.markEnd')}</Button>
        <IconButton icon="chevron-right" label={t('rec.endLater')} onclick={() => nudge('end', 1000)} />
      </div>
    </div>
  </div>
  {#if timeError}<p class="warn" role="status">{timeError}</p>{/if}
  <TextField label={t('rec.label')} hint={t('rec.labelHint')} bind:value={label} maxlength={200} dir="auto" />
  <fieldset>
    <legend>{t('rec.linkTo')}</legend>
    <PersonPicker label={t('rec.findPerson')} value={person} onPick={(p) => (person = p)} />
    <Select label={t('rec.record')} options={options} bind:value={target} disabled={options.length <= 1} />
  </fieldset>
  {#if error}<Banner kind="error">{error}</Banner>{/if}
  <div class="btns">
    <Button disabled={!!timeError} onclick={() => startMs !== null && endMs !== null && onListen(startMs, endMs)}>{t('rec.listen')}</Button>
    <span class="grow"></span>
    <Button onclick={onCancel}>{t('common.cancel')}</Button>
    <Button variant="primary" type="submit" disabled={saving}>{editing ? t('rec.saveClip') : t('rec.addClip')}</Button>
  </div>
</form>

<style>
  .editor {
    display: grid;
    gap: var(--s-3);
    padding: var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-0);
  }
  h3 {
    margin: 0;
    font-size: var(--t-md);
  }
  .times {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
    gap: var(--s-3);
  }
  .time {
    display: grid;
    gap: var(--s-2);
  }
  .nudge {
    display: flex;
    align-items: center;
    gap: var(--s-1);
  }
  fieldset {
    display: grid;
    gap: var(--s-3);
    margin: 0;
    padding: var(--s-3);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
  }
  legend {
    padding: 0 var(--s-2);
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  .warn {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--ink-0);
    border-inline-start: 3px solid var(--foxing);
    padding-inline-start: var(--s-3);
  }
  .btns {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-2);
  }
  .grow {
    flex: 1;
  }
</style>

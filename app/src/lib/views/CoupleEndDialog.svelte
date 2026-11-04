<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import RadioGroup from '../ui/RadioGroup.svelte';
  import Checkbox from '../ui/Checkbox.svelte';
  import DateField from '../editors/DateField.svelte';
  import { getApi } from '../api/context';
  import type { PersonRef } from '../api/types';
  import { settings } from '../state/settings.svelte';
  import { toasts } from '../state/toast.svelte';
  import { errMessage } from '../editors/errors';
  import { t } from '../i18n';
  import { END_KIND, ROLE_ALONE, ROLE_LEFT, type CoupleEnd, type EndReason } from './coupleEnd';

  /** Records how a couple ended: why, who left (or died), when, and whether the other raised the children alone. */
  let {
    family,
    partners,
    end,
    onclose,
  }: { family: string; partners: PersonRef[]; end: CoupleEnd | null; onclose: () => void } = $props();

  const api = getApi();
  // svelte-ignore state_referenced_locally
  let reason = $state<EndReason>(end?.reason ?? 'separation');
  // svelte-ignore state_referenced_locally
  let left = $state<string>(end?.left?.id ?? '');
  // svelte-ignore state_referenced_locally
  let date = $state(end?.event.date?.raw ?? '');
  // svelte-ignore state_referenced_locally
  let alone = $state(!!end?.alone);
  let busy = $state(false);

  const leaver = $derived(partners.find((p) => p.id === left) ?? null);
  const stayer = $derived(leaver ? (partners.find((p) => p.id !== leaver.id) ?? null) : null);

  async function save() {
    busy = true;
    try {
      const keep = (end?.event.participants ?? []).filter((p) => p.role !== ROLE_LEFT && p.role !== ROLE_ALONE);
      const participants = [
        ...keep.map((p) => ({ person: p.person.id, role: p.role })),
        ...(leaver ? [{ person: leaver.id, role: ROLE_LEFT }] : []),
        ...(alone && stayer ? [{ person: stayer.id, role: ROLE_ALONE }] : []),
      ];
      // An imported divorce keeps its kind (an annulment stays one) unless the reason itself changed.
      const kind = end && end.reason === reason ? end.event.kind : END_KIND[reason];
      await api.apply({
        type: 'saveEvent',
        owner: { kind: 'family', id: family },
        event: {
          ...(end ? { id: end.event.id } : {}),
          kind,
          dateRaw: date.trim(),
          ...(end?.event.description ? { description: end.event.description } : {}),
          ...(end?.event.cause ? { cause: end.event.cause } : {}),
          negative: false,
          private: end?.event.private ?? false,
          preferred: end?.event.preferred ?? true,
          participants,
        },
      });
      onclose();
    } catch (e) {
      toasts.push({ text: errMessage(e), kind: 'error' });
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!end) return;
    busy = true;
    try {
      await api.apply({ type: 'deleteEvent', owner: { kind: 'family', id: family }, event: end.event.id });
      onclose();
    } catch (e) {
      toasts.push({ text: errMessage(e), kind: 'error' });
    } finally {
      busy = false;
    }
  }
</script>

<Dialog open title={t('family.endTitle', { who: partners.map((p) => p.name).join(t('family.treeAnd')) })} onclose={() => !busy && onclose()}>
  <div class="form">
    <RadioGroup
      legend={t('family.endReason')}
      options={[
        { value: 'separation', label: t('family.endSeparation') },
        { value: 'divorce', label: t('family.endDivorce') },
        { value: 'death', label: t('family.endDeath') },
      ]}
      bind:value={reason}
    />
    {#if partners.length > 1}
      <RadioGroup
        legend={reason === 'death' ? t('family.endWhoDied') : t('family.endWhoLeft')}
        options={[...partners.map((p) => ({ value: p.id, label: p.name })), { value: '', label: t('family.endNotStated') }]}
        bind:value={left}
      />
    {/if}
    <DateField label={t('family.endWhen')} value={date} onChange={(r) => (date = r)} order={settings.value.dayOrder} />
    {#if stayer}
      <Checkbox label={t('family.endAlone', { who: stayer.name })} bind:checked={alone} />
    {/if}
  </div>
  {#snippet footer()}
    {#if end}<Button variant="danger" onclick={remove} disabled={busy}>{t('family.endRemove')}</Button>{/if}
    <Button onclick={onclose} disabled={busy}>{t('common.cancel')}</Button>
    <Button variant="primary" onclick={save} disabled={busy}>{t('common.save')}</Button>
  {/snippet}
</Dialog>

<style>
  .form {
    display: grid;
    gap: var(--s-4);
    min-width: min(26rem, 80vw);
  }
</style>

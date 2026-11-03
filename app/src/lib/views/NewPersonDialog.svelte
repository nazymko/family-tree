<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import TextField from '../ui/TextField.svelte';
  import Select from '../ui/Select.svelte';
  import { getApi } from '../api/context';
  import type { Id, SexKind } from '../api/types';
  import type { AddRelation as Relation } from '../state/ui.svelte';

  let {
    open,
    onclose,
    ondone,
    relation,
    anchor,
  }: {
    open: boolean;
    onclose: () => void;
    /** Called with the new person's id once Rust has confirmed the edit. */
    ondone: (id: Id | null) => void;
    relation?: Relation;
    anchor?: Id;
  } = $props();

  const api = getApi();
  let given = $state('');
  let surname = $state('');
  let sex = $state<string>('unknown');
  let birth = $state('');
  let error = $state('');
  let busy = $state(false);

  const title = $derived(relation ? `Add ${relation}` : 'Add person');

  $effect(() => {
    if (open) {
      given = '';
      surname = '';
      sex = relation === 'father' ? 'male' : relation === 'mother' ? 'female' : 'unknown';
      birth = '';
      error = '';
    }
  });

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (!given.trim() && !surname.trim()) {
      error = 'Enter a given name or a surname.';
      return;
    }
    busy = true;
    try {
      const draft = {
        given: given.trim() || undefined,
        surnames: surname.trim() ? [surname.trim()] : [],
        sex: sex as SexKind,
        birthDateRaw: birth.trim() || undefined,
      };
      const r =
        relation && anchor
          ? await api.apply({ type: 'quickAdd', anchor, relation, draft })
          : await api.apply({ type: 'createPerson', draft });
      ondone(r.focus);
    } catch (err) {
      error = (err as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<Dialog {open} {title} {onclose}>
  <form id="new-person" onsubmit={save}>
    <TextField label="Given names" bind:value={given} autocomplete="off" />
    <TextField label="Surname" bind:value={surname} autocomplete="off" error={error || undefined} />
    <Select
      label="Sex"
      bind:value={sex}
      options={[
        { value: 'unknown', label: 'Unknown' },
        { value: 'female', label: 'Female' },
        { value: 'male', label: 'Male' },
        { value: 'intersex', label: 'Intersex' },
        { value: 'other', label: 'Other' },
      ]}
    />
    <TextField label="Birth date" bind:value={birth} hint="For example 3 Apr 1901, or about 1850. Optional." />
  </form>
  {#snippet footer()}
    <Button onclick={onclose}>Cancel</Button>
    <Button variant="primary" type="submit" form="new-person" disabled={busy}>{title}</Button>
  {/snippet}
</Dialog>

<style>
  form {
    display: grid;
    gap: var(--s-4);
    min-width: min(26rem, 80vw);
  }
</style>

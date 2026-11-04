<script lang="ts">
  import { getApi } from '../api/context';
  import type { Id } from '../api/types';
  import { parentFacts, type ParentFacts } from '../family/addOptions';
  import { t } from '../i18n';
  import { project } from '../state/project.svelte';
  import Menu from '../ui/Menu.svelte';
  import { relativeItems } from './relativeItems';

  /** The toolbar's "Add relative" button: the same add items as the right-click menu, for the selected person. */
  let { person }: { person: Id | null } = $props();

  const api = getApi();
  let facts = $state<{ id: Id; value: ParentFacts } | null>(null);

  // The rules follow the selected person, and change after each edit (a parent may have just been added).
  $effect(() => {
    const id = person;
    void project.rev;
    let live = true;
    if (!id) {
      facts = null;
      return;
    }
    // Rules of another person must not outlive a real change of selection for long; they are replaced as soon as they load.
    void api
      .getFamilyView(id)
      .then((v) => {
        if (live) facts = { id, value: parentFacts(v) };
      })
      .catch(() => {
        if (live) facts = null;
      });
    return () => {
      live = false;
    };
  });

  // While the facts of a newly selected person load, the last ones stay, so the button does not vanish (and lose focus) on each reload.
  const items = $derived(person && facts ? relativeItems(facts.value, person) : []);
</script>

{#if items.length}
  <Menu label={t('chartAdd.addRelative')} icon="plus" {items} />
{/if}

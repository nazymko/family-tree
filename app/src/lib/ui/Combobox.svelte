<script lang="ts" generics="T">
  import Field from './Field.svelte';
  import './controls.css';

  let {
    label,
    items,
    value,
    text,
    onText,
    onPick,
    render,
    detail,
    createLabel,
    onCreate,
    hint,
    error,
  }: {
    label: string;
    items: T[];
    value: T | null;
    text: string;
    onText: (s: string) => void;
    onPick: (t: T) => void;
    render: (t: T) => string;
    /** Optional second line under each option (for example a place chain). */
    detail?: (t: T) => string | undefined;
    createLabel?: (text: string) => string;
    onCreate?: (text: string) => void;
    hint?: string;
    error?: string;
  } = $props();

  const uid = $props.id();
  const listId = `${uid}-list`;
  let open = $state(false);
  let active = $state(0);

  const trimmed = $derived(text.trim());
  const showCreate = $derived(
    !!createLabel &&
      !!onCreate &&
      trimmed.length > 0 &&
      !items.some((i) => render(i).toLowerCase() === trimmed.toLowerCase()),
  );
  const rowCount = $derived(items.length + (showCreate ? 1 : 0));
  const activeId = $derived(open && rowCount > 0 ? `${uid}-opt-${active}` : undefined);

  function pickAt(i: number) {
    if (i < items.length) onPick(items[i]);
    else if (showCreate) onCreate?.(trimmed);
    open = false;
  }

  function onkeydown(e: KeyboardEvent) {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        if (!open) {
          open = true;
          active = 0;
        } else if (rowCount > 0) active = (active + 1) % rowCount;
        break;
      case 'ArrowUp':
        e.preventDefault();
        if (!open) {
          open = true;
          active = Math.max(rowCount - 1, 0);
        } else if (rowCount > 0) active = (active - 1 + rowCount) % rowCount;
        break;
      case 'Home':
        if (open) {
          e.preventDefault();
          active = 0;
        }
        break;
      case 'End':
        if (open) {
          e.preventDefault();
          active = Math.max(rowCount - 1, 0);
        }
        break;
      case 'Enter':
        if (open && rowCount > 0) {
          e.preventDefault();
          pickAt(Math.min(active, rowCount - 1));
        }
        break;
      case 'Escape':
        if (open) {
          e.preventDefault();
          e.stopPropagation();
          open = false;
        }
        break;
    }
  }

  function oninput(e: Event & { currentTarget: HTMLInputElement }) {
    onText(e.currentTarget.value);
    open = true;
    active = 0;
  }
</script>

<Field {label} {hint} {error}>
  {#snippet children(f)}
    <div class="wrap">
      <input
        class="control"
        id={f.id}
        role="combobox"
        type="text"
        autocomplete="off"
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={listId}
        aria-activedescendant={activeId}
        aria-describedby={f.describedBy}
        aria-invalid={f.invalid}
        aria-haspopup="listbox"
        value={text}
        data-picked={value ? 'true' : undefined}
        {oninput}
        {onkeydown}
        onblur={() => (open = false)}
      />
      <ul id={listId} class="list" role="listbox" aria-label={label} hidden={!open || rowCount === 0}>
        {#each items as it, i (i)}
          <li
            id="{uid}-opt-{i}"
            role="option"
            tabindex="-1"
            aria-selected={open && i === active}
            class:active={i === active}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => pickAt(i)}
            onkeydown={() => {}}
          >
            {render(it)}
            {#if detail?.(it)}<span class="detail">{detail(it)}</span>{/if}
          </li>
        {/each}
        {#if showCreate}
          <li
            id="{uid}-opt-{items.length}"
            role="option"
            tabindex="-1"
            aria-selected={open && active === items.length}
            class="create"
            class:active={active === items.length}
            onmousedown={(e) => e.preventDefault()}
            onclick={() => pickAt(items.length)}
            onkeydown={() => {}}
          >
            {createLabel?.(trimmed)}
          </li>
        {/if}
      </ul>
    </div>
  {/snippet}
</Field>

<style>
  .wrap {
    position: relative;
  }
  .list {
    position: absolute;
    z-index: 20;
    inset-inline: 0;
    top: calc(100% + 2px);
    max-height: 15rem;
    margin: 0;
    padding: var(--s-2) 0;
    overflow: auto;
    list-style: none;
    background: var(--paper-1);
    border: 1px solid var(--rule);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
  }
  .list[hidden] {
    display: none;
  }
  li {
    padding: var(--s-3) var(--s-4);
    cursor: pointer;
  }
  li.active {
    background: var(--ribbon-wash);
  }
  .detail {
    display: block;
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  .create {
    border-top: 1px solid var(--rule);
    color: var(--ribbon);
    font-weight: 500;
  }
</style>

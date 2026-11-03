<script lang="ts">
  import Ribbon from './Ribbon.svelte';
  import Lock from './Lock.svelte';

  type Sex = 'male' | 'female' | 'intersex' | 'unknown' | 'other';
  const SEX: Record<Sex, { glyph: string; name: string }> = {
    male: { glyph: '♂', name: 'Male' },
    female: { glyph: '♀', name: 'Female' },
    intersex: { glyph: '⚥', name: 'Intersex' },
    unknown: { glyph: '?', name: 'Sex unknown' },
    other: { glyph: '•', name: 'Other sex' },
  };

  let {
    name,
    lifespan = '',
    sex = 'unknown',
    living = false,
    isPrivate = false,
    home = false,
    current = false,
    lang,
    onclick,
    href,
    relationship,
    chips = [],
    detail,
    flipKey,
  }: {
    name: string;
    lifespan?: string;
    sex?: Sex;
    living?: boolean;
    isPrivate?: boolean;
    home?: boolean;
    current?: boolean;
    lang?: string;
    onclick?: () => void;
    href?: string;
    /** When set, the accessible name becomes "name, lifespan, relationship". */
    relationship?: string;
    /** Visible relationship chips such as "Adopted". */
    chips?: string[];
    /** One extra line, e.g. the birth text of a child. */
    detail?: string;
    /** Value for data-flip-key (re-centring animation). */
    flipKey?: string;
  } = $props();

  const accName = $derived(
    relationship ? [name, lifespan, relationship].filter((x) => x && x.length > 0).join(', ') : undefined,
  );
</script>

{#snippet body()}
  <span class="sex" role="img" aria-label={SEX[sex].name}>{SEX[sex].glyph}</span>
  <span class="text">
    <bdi dir="auto" {lang} class="name">{name}</bdi>
    <span class="meta tnum">
      {#if living}<span class="living"><span class="dot" aria-hidden="true"></span>Living</span>{/if}
      {#if lifespan}<span>{lifespan}</span>{/if}
      {#if isPrivate}<Lock />{/if}
    </span>
    {#if chips.length > 0 || detail}
      <span class="meta">
        {#each chips as c (c)}<span class="chip">{c}</span>{/each}
        {#if detail}<span class="tnum">{detail}</span>{/if}
      </span>
    {/if}
  </span>
  {#if home}<Ribbon title="Home person" size="1.1rem" />{/if}
{/snippet}

{#if href}
  <a
    class="pc"
    class:current
    {href}
    aria-label={accName}
    data-flip-key={flipKey}
    aria-current={current ? 'true' : undefined}>{@render body()}</a
  >
{:else}
  <button
    type="button"
    class="pc"
    class:current
    aria-label={accName}
    data-flip-key={flipKey}
    aria-current={current ? 'true' : undefined}
    {onclick}
  >
    {@render body()}
  </button>
{/if}

<style>
  .pc {
    display: flex;
    align-items: center;
    gap: var(--s-4);
    min-width: 11rem;
    min-height: 3.5rem;
    padding: var(--s-3) var(--s-4);
    border: 1px solid var(--rule);
    border-radius: var(--r-1);
    background: var(--paper-1);
    color: var(--ink-0);
    text-align: start;
    text-decoration: none;
    cursor: pointer;
    transition: border-color var(--dur-fast) var(--ease);
  }
  .pc:hover {
    border-color: var(--ink-2);
  }
  .current {
    background: var(--ribbon-wash);
    border-color: var(--ribbon);
  }
  .sex {
    flex: none;
    width: 1.25rem;
    text-align: center;
    color: var(--ink-1);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .name {
    font-family: var(--font-serif);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    gap: var(--s-3);
    font-size: var(--t-sm);
    line-height: var(--t-sm-lh);
    color: var(--ink-1);
  }
  .living {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    color: var(--verdigris);
    font-weight: 500;
  }
  .chip {
    padding: 0 var(--s-3);
    border: 1px solid var(--foxing);
    border-radius: 999px;
    color: var(--foxing);
    font-size: var(--t-xs);
    font-weight: 500;
  }
  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: currentColor;
  }
</style>

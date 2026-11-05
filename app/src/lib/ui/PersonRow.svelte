<script lang="ts">
  import Ribbon from './Ribbon.svelte';
  import AgentBadge from './AgentBadge.svelte';
  import type { PersonRow } from '../api/types';
  import { t, plural } from '../i18n';

  let {
    row,
    home = false,
    selected = false,
    agentChanged = false,
  }: { row: PersonRow | null; home?: boolean; selected?: boolean; agentChanged?: boolean } = $props();
</script>

<div class="pr" class:selected>
  {#if row}
    <bdi dir="auto" class="name">{row.name}</bdi>
    <span class="life tnum">{row.lifespan}</span>
    <span class="flags">
      {#if row.living}<span class="living"><span class="dot" aria-hidden="true"></span><span>{t('common.living')}</span></span>{/if}
      {#if row.private}<span class="priv">{t('common.private')}</span>{/if}
      {#if agentChanged}<AgentBadge />{/if}
    </span>
    <span class="src tnum">
      {#if row.sourceCount > 0}{plural('common.sources', row.sourceCount)}{:else}{t('common.noSource')}{/if}
    </span>
    <span class="rb">{#if home}<Ribbon size="1.1rem" />{/if}</span>
  {:else}
    <span class="name ghost" aria-hidden="true"></span>
  {/if}
</div>

<style>
  .pr {
    display: grid;
    grid-template-columns: minmax(10rem, 3fr) minmax(6rem, 1.2fr) minmax(5rem, 1fr) minmax(5.5rem, 1fr) 1.5rem;
    align-items: center;
    gap: var(--s-4);
    width: 100%;
    padding: 0 var(--s-4);
    border-inline-start: 3px solid transparent;
    color: var(--ink-0);
  }
  .selected {
    background: var(--ribbon-wash);
    border-inline-start-color: var(--ribbon);
  }
  .name {
    font-family: var(--font-serif);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ghost {
    height: 0.75rem;
    width: 40%;
    background: var(--rule);
    border-radius: 2px;
  }
  .life,
  .src {
    color: var(--ink-1);
    font-size: var(--t-sm);
    white-space: nowrap;
  }
  .living {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    color: var(--verdigris);
    font-size: var(--t-sm);
    font-weight: 500;
  }
  .dot {
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: currentColor;
  }
  .priv {
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  .flags {
    display: flex;
    gap: var(--s-3);
  }
  .rb {
    color: var(--ribbon);
    line-height: 0;
  }
</style>

<script lang="ts">
  import Field from './Field.svelte';
  import type { HTMLTextareaAttributes } from 'svelte/elements';
  import './controls.css';

  interface Props extends Omit<HTMLTextareaAttributes, 'value'> {
    label: string;
    hint?: string;
    error?: string;
    value?: string;
    /** Draw the index-card ruling behind the text (notes). */
    ruled?: boolean;
  }
  let { label, hint, error, value = $bindable(''), ruled = false, ...rest }: Props = $props();
</script>

<Field {label} {hint} {error}>
  {#snippet children(f)}
    <textarea
      class="control"
      class:ruled
      id={f.id}
      aria-describedby={f.describedBy}
      aria-invalid={f.invalid}
      bind:value
      {...rest}
    ></textarea>
  {/snippet}
</Field>

<style>
  textarea {
    min-height: 7rem;
    resize: vertical;
  }
  .ruled {
    font-family: var(--font-serif);
    font-size: var(--t-md);
    font-weight: 450;
    line-height: var(--rule-step);
    padding-block: 0 var(--s-3);
    background-color: var(--paper-1);
    background-image: repeating-linear-gradient(to bottom, transparent 0 27px, var(--rule) 27px 28px);
    background-attachment: local;
  }
</style>

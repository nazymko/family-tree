<script lang="ts">
  import type { FieldChangeDto } from '../api/types';
  import { t, type MessageKey } from '../i18n';

  /** One line per changed field of a person: `Surname: Koval → Kovalchuk`. */
  let { fields }: { fields: FieldChangeDto[] } = $props();

  const LABELS: Record<string, MessageKey> = {
    sex: 'editor.sex',
    kind: 'editor.kindOfName',
    preferred: 'editor.preferredName',
    private: 'editor.private',
    prefix: 'editor.prefix',
    given: 'editor.given',
    call: 'editor.call',
    surnamePrefix: 'editor.surnamePrefix',
    surnames: 'editor.surnames',
    patronymic: 'editor.patronymic',
    suffix: 'editor.suffix',
    nickname: 'editor.nickname',
    lang: 'editor.language',
    date: 'editor.dateInUse',
    proof: 'fieldChange.proof',
    confidence: 'fieldChange.confidence',
    raw: 'fieldChange.raw',
    citations: 'fieldChange.citations',
    notes: 'fieldChange.notes',
    unknown: 'fieldChange.unknown',
    position: 'fieldChange.position',
  };

  const KINDS = ['birth', 'married', 'aka', 'religious', 'nickname', 'immigrant', 'professional'];
  const SEXES = ['male', 'female', 'intersex', 'unknown'];

  function label(field: string): string {
    const key = LABELS[field];
    return key ? t(key) : field;
  }

  /** A stored token in words; text fields come out as stored. `null` when the side is empty. */
  function word(field: string, v: string | null): string | null {
    if (v === null || v === '') return null;
    if (field === 'preferred' || field === 'private') return t(v === 'yes' ? 'fieldChange.yes' : 'fieldChange.no');
    if (field === 'proof') return t(v === 'proven' ? 'fieldChange.proven' : 'fieldChange.disproven');
    if (field === 'kind' || field === 'sex') {
      if (v.startsWith('other:')) return v.slice(6) || t(field === 'kind' ? 'nameKind.other' : 'sex.other');
      if (field === 'kind' && KINDS.includes(v)) return t(`nameKind.${v}` as MessageKey);
      if (field === 'sex' && SEXES.includes(v)) return t(`sex.${v}` as MessageKey);
    }
    return v;
  }

  function who(f: FieldChangeDto): string {
    if (f.nameNumber === undefined) return '';
    const kind = word('kind', f.nameKind ?? null) ?? '';
    return t('fieldChange.nameTag', { n: f.nameNumber, kind });
  }
</script>

{#if fields.length > 0}
  <ul class="fields">
    {#each fields as f, i (i)}
      {@const b = word(f.field, f.before)}
      {@const a = word(f.field, f.after)}
      <li>
        {#if f.nameNumber !== undefined}<span class="who">{who(f)}</span>{/if}
        <span class="label">{label(f.field)}:</span>
        <span class="val before">{#if b === null}<em class="none">{t('fieldChange.empty')}</em>{:else}<bdi dir="auto">{b}</bdi>{/if}</span>
        <span class="arrow" aria-hidden="true">→</span>
        <span class="val after">{#if a === null}<em class="none">{t('fieldChange.empty')}</em>{:else}<bdi dir="auto">{a}</bdi>{/if}</span>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .fields {
    list-style: none;
    margin: var(--s-1) 0 0;
    padding: 0;
    display: grid;
    gap: var(--s-1);
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 var(--s-2);
    overflow-wrap: anywhere;
    min-width: 0;
  }
  .who {
    color: var(--ink-2);
    flex-basis: 100%;
  }
  .label {
    color: var(--ink-2);
  }
  .none {
    color: var(--ink-2);
  }
  .arrow {
    color: var(--ink-2);
  }
</style>

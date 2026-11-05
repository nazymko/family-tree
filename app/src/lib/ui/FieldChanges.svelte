<script lang="ts">
  import type { FieldChangeDto } from '../api/types';
  import { t, type MessageKey } from '../i18n';

  /** A table of a person's changed fields: the field, its value before and its value after. */
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

  /** Singular `Surname` unless either side holds several surnames (joined with a comma). */
  function label(field: string, f?: FieldChangeDto): string {
    if (field === 'surnames' && f && !(f.before ?? '').includes(', ') && !(f.after ?? '').includes(', ')) return t('fieldChange.surname');
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

  /** Neighbouring rows about the same name share one heading instead of repeating it. */
  const groups = $derived.by(() => {
    const out: { heading: string; rows: FieldChangeDto[] }[] = [];
    for (const f of fields) {
      const heading = who(f);
      const last = out[out.length - 1];
      if (last && last.heading === heading) last.rows.push(f);
      else out.push({ heading, rows: [f] });
    }
    return out;
  });
</script>

{#snippet value(v: string | null)}
  {#if v === null}<em class="none">{t('fieldChange.empty')}</em>{:else}<bdi dir="auto">{v}</bdi>{/if}
{/snippet}

{#if fields.length > 0}
  <table class="fields">
    <thead>
      <tr>
        <th scope="col">{t('fieldChange.field')}</th>
        <th scope="col">{t('fieldChange.before')}</th>
        <th scope="col">{t('fieldChange.after')}</th>
      </tr>
    </thead>
    {#each groups as g, i (i)}
      <tbody>
        {#if g.heading}<tr><th class="who" colspan="3" scope="rowgroup">{g.heading}</th></tr>{/if}
        {#each g.rows as f, j (j)}
          <tr>
            <th scope="row">{label(f.field, f)}</th>
            <td>{@render value(word(f.field, f.before))}</td>
            <td>{@render value(word(f.field, f.after))}</td>
          </tr>
        {/each}
      </tbody>
    {/each}
  </table>
{/if}

<style>
  .fields {
    width: 100%;
    table-layout: fixed;
    border-collapse: collapse;
    margin: var(--s-1) 0 0;
    font-size: var(--t-sm);
    color: var(--ink-1);
  }
  th,
  td {
    padding: var(--s-1) var(--s-2);
    border-bottom: 1px solid var(--rule);
    text-align: start;
    vertical-align: top;
    overflow-wrap: anywhere;
    font-weight: 400;
  }
  thead th,
  tbody th {
    color: var(--ink-2);
  }
  thead th {
    font-weight: 500;
  }
  .who {
    font-weight: 500;
  }
  .none {
    color: var(--ink-2);
  }
</style>

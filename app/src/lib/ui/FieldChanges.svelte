<script lang="ts">
  import type { FieldChangeDto } from '../api/types';
  import { plural, t, type MessageKey } from '../i18n';
  import { dateText, eventKindLabel } from '../i18n/backend';
  import { withSexIcon } from './sex';
  import type { SexKind } from '../api/types';
  import { collapse, diffText, sideOf, type TextDiff, type ViewSeg } from './textDiff';

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

  /** Fields of the records other than a person; each has a label `fieldChange.<id>`. */
  const OWN = new Set([
    'noteKind', 'text', 'shared', 'eventKind', 'when', 'place', 'description', 'age', 'cause', 'negative', 'participant',
    'partner1', 'partner2', 'unionKind', 'child', 'childRel1', 'childRel2', 'childPreferred', 'childCitations', 'childUnknown',
    'source', 'page', 'quality', 'original', 'primaryInfo', 'direct', 'title', 'author', 'publication', 'abbreviation',
    'repository', 'placeNames', 'parent', 'placeKind', 'lat', 'lon', 'placeRaw', 'repoName', 'address', 'urls', 'path',
    'originalName', 'mime', 'sha256', 'media', 'extIds', 'mediaPrimary', 'mediaRegion',
  ]);

  /** Joins the names of one row's `of` (the partners of a family); the page puts its own "and" between them. */
  const LIST_SEP = '\u001f';
  /** Stands for a record that was deleted and cannot be read any more. */
  const REMOVED = '\u0001removed';
  /** Stands for the entry of unnamed brothers and sisters, which has no name. */
  const SIBLINGS = '\u0001siblings';
  const YES_NO = new Set(['preferred', 'private', 'shared', 'negative', 'original', 'primaryInfo', 'direct', 'childPreferred', 'mediaPrimary']);
  const PEOPLE = new Set(['partner1', 'partner2', 'child']);
  const NOTE_KINDS = ['general', 'story', 'research', 'transcription', 'conclusion'];
  const UNION: Record<string, MessageKey> = {
    married: 'famEd.unionMarried',
    civil_union: 'famEd.unionCivil',
    unmarried: 'famEd.unionUnmarried',
    unknown: 'famEd.unionUnknown',
  };
  const RELS: Record<string, MessageKey> = {
    birth: 'famEd.relBirth',
    adopted: 'famEd.relAdopted',
    step: 'famEd.relStep',
    foster: 'famEd.relFoster',
    guardian: 'famEd.relGuardian',
    sealed: 'famEd.relSealed',
    unknown: 'famEd.relUnknown',
  };
  const ROLES = ['principal', 'witness', 'godparent', 'clergy', 'officiator', 'informant', 'parent', 'father', 'mother', 'child', 'spouse', 'husband', 'wife', 'friend', 'neighbor', 'multiple'];

  const KINDS = ['birth', 'married', 'aka', 'religious', 'nickname', 'immigrant', 'professional'];
  const SEXES = ['male', 'female', 'intersex', 'unknown'];

  /** Singular `Surname` unless either side holds several surnames (joined with a comma). */
  function label(field: string, f?: FieldChangeDto): string {
    if (field === 'surnames' && f && !(f.before ?? '').includes(', ') && !(f.after ?? '').includes(', ')) return t('fieldChange.surname');
    if (OWN.has(field)) return t(`fieldChange.${field}` as MessageKey);
    const key = LABELS[field];
    return key ? t(key) : field;
  }

  /** A stored token in words; text fields come out as stored. `null` when the side is empty. */
  function word(field: string, v: string | null): string | null {
    const w = plain(field, v);
    return w === null ? null : w.replaceAll(REMOVED, t('fieldChange.removed')).replaceAll(SIBLINGS, t('person.siblingMarker'));
  }

  /** The people of a row's `of`, an unknown one in words, joined with the language's "and". */
  function ofText(of: string): string {
    return of
      .split(LIST_SEP)
      .map((p) => (p === 'Unknown' ? t('changeLog.unknownPerson') : p === SIBLINGS ? t('person.siblingMarker') : p))
      .join(` ${t('fieldChange.and')} `);
  }

  function plain(field: string, v: string | null): string | null {
    if (v === null || v === '') return null;
    if (field === 'unknown' || field === 'childUnknown') return plural('fieldChange.items', Number(v));
    if (YES_NO.has(field)) return t(v === 'yes' ? 'fieldChange.yes' : 'fieldChange.no');
    if (field === 'proof') return t(v === 'proven' ? 'fieldChange.proven' : 'fieldChange.disproven');
    if (field === 'kind' || field === 'sex') {
      if (field === 'sex' && v.startsWith('other:')) return withSexIcon('other', v.slice(6) || t('sex.other'));
      if (v.startsWith('other:')) return v.slice(6) || t('nameKind.other');
      if (field === 'kind' && KINDS.includes(v)) return t(`nameKind.${v}` as MessageKey);
      if (field === 'sex' && SEXES.includes(v)) return withSexIcon(v as SexKind, t(`sex.${v}` as MessageKey));
    }
    if (field === 'eventKind') return eventKindLabel(v);
    if (field === 'when') return dateText(v);
    if (PEOPLE.has(field)) return v === 'Unknown' ? t('changeLog.unknownPerson') : v;
    const mapped = (map: Record<string, MessageKey>, other: MessageKey) => (v.startsWith('other:') ? v.slice(6) || t(other) : map[v] ? t(map[v]) : v);
    if (field === 'unionKind') return mapped(UNION, 'famEd.unionOther');
    if (field === 'childRel1' || field === 'childRel2') return mapped(RELS, 'famEd.relOther');
    if (field === 'noteKind') return v.startsWith('other:') ? v.slice(6) || t('noteKind.other') : NOTE_KINDS.includes(v) ? t(`noteKind.${v}` as MessageKey) : v;
    if (field === 'participant') return ROLES.includes(v) ? t(`fieldChange.role.${v}` as MessageKey) : v.startsWith('other:') ? v.slice(6) || t('fieldChange.role.other') : v;
    return v;
  }

  function who(f: FieldChangeDto): string {
    if (f.nameNumber === undefined) return '';
    const kind = word('kind', f.nameKind ?? null) ?? '';
    return t('fieldChange.nameTag', { n: f.nameNumber, kind });
  }

  /** Free-text fields: a change inside one is shown word by word. */
  const TEXT = new Set(['text', 'description', 'title', 'author', 'publication', 'address', 'cause', 'raw', 'nickname', 'given', 'placeRaw', 'page']);
  /** A text longer than this many characters is shown as its changes with a little context around them. */
  const LONG = 300;

  interface RowDiff {
    diff: TextDiff;
    long: boolean;
  }

  /** The word-level diff of a row with a text on both sides and some words in common; otherwise `null` (shown plainly). */
  function rowDiff(f: FieldChangeDto): RowDiff | null {
    if (!TEXT.has(f.field) || !f.before || !f.after) return null;
    const diff = diffText(f.before, f.after);
    if (diff.sharedWords === 0 || (diff.removedWords === 0 && diff.addedWords === 0)) return null;
    return { diff, long: Math.max(f.before.length, f.after.length) > LONG };
  }

  function summary(d: TextDiff): string {
    const removed = d.removedWords > 0 ? plural('fieldChange.words', d.removedWords) : '';
    const added = d.addedWords > 0 ? plural('fieldChange.words', d.addedWords) : '';
    if (removed && added) return t('fieldChange.summaryBoth', { removed, added });
    return removed ? t('fieldChange.summaryRemoved', { words: removed }) : added ? t('fieldChange.summaryAdded', { words: added }) : '';
  }

  /** Rows (by group and position) whose whole text is shown instead of the changes only. */
  let full = $state<Record<string, boolean>>({});

  function parts(rd: RowDiff, side: 'before' | 'after', showAll: boolean): ViewSeg[] {
    const view = sideOf(rd.diff.segs, side);
    return rd.long && !showAll ? collapse(view) : view;
  }

  /** Neighbouring rows about the same name share one heading instead of repeating it. */
  const groups = $derived.by(() => {
    const out: { heading: string; id: string | undefined; rows: FieldChangeDto[] }[] = [];
    for (const f of fields) {
      const heading = who(f);
      const last = out[out.length - 1];
      // Two names can read alike ("Name 1 (birth)": one removed, one kept); the id tells them apart.
      if (last && last.heading === heading && last.id === f.nameId) last.rows.push(f);
      else out.push({ heading, id: f.nameId, rows: [f] });
    }
    return out;
  });
</script>

{#snippet value(v: string | null)}
  {#if v === null}<em class="none">{t('fieldChange.empty')}</em>{:else}<bdi dir="auto">{v}</bdi>{/if}
{/snippet}

{#snippet marked(segs: ViewSeg[])}
  <bdi dir="auto">{#each segs as s, k (k)}{#if s.kind === 'del'}<del>{s.text}</del>{:else if s.kind === 'ins'}<ins>{s.text}</ins>{:else if s.kind === 'gap'}<span class="gap" title={t('fieldChange.unchangedHidden')}>{s.text}</span>{:else}{s.text}{/if}{/each}</bdi>
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
          {@const rd = rowDiff(f)}
          {@const key = `${i}-${j}`}
          <tr>
            <th scope="row">{label(f.field, f) + (f.of ? ' · ' : '')}{#if f.of}<bdi dir="auto">{ofText(f.of).replaceAll(REMOVED, t('fieldChange.removed'))}</bdi>{/if}{#if rd}
                <span class="sum">{summary(rd.diff)}</span>
                {#if rd.long}<button type="button" class="toggle" aria-expanded={full[key] === true} onclick={() => (full[key] = !full[key])}>{t(full[key] ? 'fieldChange.showChanges' : 'fieldChange.showFull')}</button>{/if}
              {/if}</th>
            {#if rd}
              <td>{@render marked(parts(rd, 'before', full[key] === true))}</td>
              <td>{@render marked(parts(rd, 'after', full[key] === true))}</td>
            {:else}
              <td>{@render value(word(f.field, f.before))}</td>
              <td>{@render value(word(f.field, f.after))}</td>
            {/if}
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
  td {
    white-space: pre-wrap;
  }
  .sum {
    display: block;
    margin-top: 2px;
    font-size: var(--t-xs, 0.85em);
    color: var(--ink-2);
  }
  .toggle {
    margin-top: 2px;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-size: var(--t-xs, 0.85em);
    color: var(--focus);
    text-decoration: underline;
    cursor: pointer;
  }
  del {
    background: color-mix(in srgb, var(--danger) 18%, transparent);
    color: var(--ink-0);
    text-decoration: line-through;
    text-decoration-color: var(--danger);
  }
  ins {
    background: color-mix(in srgb, var(--verdigris) 22%, transparent);
    color: var(--ink-0);
    text-decoration: underline;
    text-decoration-color: var(--verdigris);
  }
  .gap {
    color: var(--ink-2);
  }
  .none {
    color: var(--ink-2);
  }
</style>

import type { Id, ParentRelId } from '../api/types';
import { addOptions, type ParentFacts } from '../family/addOptions';
import { t } from '../i18n';
import { ui, type AddRelation } from '../state/ui.svelte';
import type { MenuItem } from '../ui/Menu.svelte';

/** The "Add father / mother / partner / child / sibling" items for one person, with the shared enable rules. */
export function relativeItems(
  facts: ParentFacts,
  person: Id,
  request: (relation: AddRelation, anchor: Id, newParentSet?: boolean, parentRel?: ParentRelId) => void = (...args) => ui.requestAdd(...args),
): MenuItem[] {
  return addOptions(facts).map((o) => ({
    label: t(o.label),
    onselect: () => (o.parentRel ? request(o.relation, person, o.newParentSet, o.parentRel) : request(o.relation, person, o.newParentSet)),
    disabled: o.disabled,
    title: o.hint ? t(o.hint) : undefined,
  }));
}

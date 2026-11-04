import type { FamilyViewDto } from '../api/types';
import type { MessageKey } from '../i18n';
import type { AddRelation } from '../state/ui.svelte';

/** What the add rules need to know about a person's parents. */
export interface ParentFacts {
  fatherRecorded: boolean;
  motherRecorded: boolean;
  /** At least one parent in any set of parents (a sibling needs somebody to belong to). */
  hasAnyParent: boolean;
}

/** One way to add a relative, with its rules already applied. The caller turns it into a menu item or a button. */
export interface AddOption {
  relation: AddRelation;
  /** Adds a second set of parents instead of filling the first. */
  newParentSet?: boolean;
  label: MessageKey;
  disabled: boolean;
  /** Why it is disabled. */
  hint?: MessageKey;
}

/** The facts from the person's family view. The preferred set of parents counts, else the first. */
export function parentFacts(view: Pick<FamilyViewDto, 'parentSets'>): ParentFacts {
  const primary = view.parentSets.find((s) => s.preferred) ?? view.parentSets[0] ?? null;
  return {
    fatherRecorded: !!primary?.parents[0],
    motherRecorded: !!primary?.parents[1],
    hasAnyParent: view.parentSets.some((s) => s.parents[0] || s.parents[1]),
  };
}

/**
 * The add rules shared by every place that offers "add a relative":
 * a father or mother that is already recorded is disabled; with both recorded the only parent choice is
 * "add another set of parents"; a sibling needs a parent.
 */
export function addOptions(f: ParentFacts): AddOption[] {
  const parents: AddOption[] =
    f.fatherRecorded && f.motherRecorded
      ? [{ relation: 'father', newParentSet: true, label: 'family.addAnotherParents', disabled: false }]
      : [
          { relation: 'father', label: 'edit.addFather', disabled: f.fatherRecorded, hint: f.fatherRecorded ? 'family.fatherRecorded' : undefined },
          { relation: 'mother', label: 'edit.addMother', disabled: f.motherRecorded, hint: f.motherRecorded ? 'family.motherRecorded' : undefined },
        ];
  return [
    ...parents,
    { relation: 'partner', label: 'edit.addPartner', disabled: false },
    { relation: 'child', label: 'edit.addChild', disabled: false },
    { relation: 'sibling', label: 'edit.addSibling', disabled: !f.hasAnyParent, hint: f.hasAnyParent ? undefined : 'family.addParentFirst' },
  ];
}

import { t } from '../i18n';

/**
 * The short line under a name that shows the surname at birth ("дів. Bondar", "née Bondar"), or '' when there is
 * nothing to show. A man gets the "at birth" form, a woman or a person of unknown sex the maiden form.
 */
export function birthSurnameText(p: { sex: string; birthSurname?: string | null } | null | undefined): string {
  const surname = p?.birthSurname?.trim();
  if (!p || !surname) return '';
  return t(p.sex === 'male' ? 'surnames.maidenMale' : 'surnames.maiden', { surname });
}

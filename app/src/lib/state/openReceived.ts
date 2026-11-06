import type { Api, Platform } from '../api/api';
import { t } from '../i18n';
import { enterProject } from './openTree';

/**
 * Opens a copy of a family tree that arrived as a zip: the person picks the zip and a new folder to unpack it into,
 * and the copy opens as a tree of its own. Returns an error to show, or null when it opened or the person backed out.
 */
export async function openReceivedCopy(api: Api, platform: Platform): Promise<string | null> {
  const zip = await platform.pickFile({ title: t('merge.openReceivedZip'), extensions: ['zip'] });
  if (!zip) return null;
  const dest = await platform.pickFolder(t('merge.openReceivedFolder'));
  if (!dest) return null;
  try {
    await enterProject(api, await api.openReceivedCopy(zip, dest));
    return null;
  } catch (e) {
    const reason = e instanceof Error ? e.message : typeof e === 'object' && e && 'message' in e ? String((e as { message: unknown }).message) : '';
    return t('merge.openReceivedFailed', { reason });
  }
}

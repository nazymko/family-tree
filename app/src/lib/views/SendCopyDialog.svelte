<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Button from '../ui/Button.svelte';
  import Banner from '../ui/Banner.svelte';
  import { getApi, getPlatform } from '../api/context';
  import { notices } from '../ui/notices.svelte';
  import type { CopySizeDto } from '../api/types';
  import { plural, t } from '../i18n';
  import { errMessage } from '../editors/errors';
  import { fileOf, sizeText } from './mergeText';
  import { handOver, revealAction } from '../ui/mobile';

  let { onClose }: { onClose: () => void } = $props();

  const api = getApi();
  const platform = getPlatform();

  let size = $state<CopySizeDto | null>(null);
  let sizeError = $state('');
  let error = $state('');
  let making = $state(false);

  $effect(() => {
    let live = true;
    api.copySize().then(
      (s) => {
        if (live) size = s;
      },
      (e) => {
        if (live) sizeError = errMessage(e);
      },
    );
    return () => {
      live = false;
    };
  });

  /** "130 people, 2 photos, 4 recordings": a part with nothing in it is left out. */
  function sizeParts(s: CopySizeDto): string {
    const parts = [plural('merge.sendPeopleCount', s.people)];
    if (s.photos > 0 || s.recordings === 0) parts.push(plural('merge.sendPhotoCount', s.photos));
    if (s.recordings > 0) parts.push(plural('merge.sendRecordingCount', s.recordings));
    return parts.join(', ');
  }

  async function make() {
    error = '';
    const dest = await platform.pickSavePath({
      title: t('merge.sendPickTitle'),
      defaultName: t('merge.sendDefaultName'),
      extensions: ['zip'],
    });
    if (!dest) return;
    making = true;
    try {
      const r = await api.sendCopyZip(dest);
      // On a phone the zip was made in the app's cache: the system save / share sheet takes it from there.
      if (!(await handOver(platform, r.path))) return;
      notices.show({
        message: t('merge.sendDone', { file: fileOf(r.path), size: sizeText(r.bytes) }),
        ...revealAction(platform, r.path, t('merge.reveal')),
      });
      onClose();
    } catch (e) {
      error = errMessage(e);
    } finally {
      making = false;
    }
  }
</script>

<Dialog open title={t('merge.sendTitle')} onclose={onClose}>
  <div class="send">
    {#if error}<Banner kind="error">{error}</Banner>{/if}
    <p>{t('merge.sendIntro')}</p>
    <Banner kind="warning">
      <p class="lead">{t('merge.sendContains')}</p>
      <ul>
        <li>{t('merge.sendLiving')}</li>
        <li>{t('merge.sendPrivate')}</li>
        <li>{t('merge.sendHistory')}</li>
        <li>{t('merge.sendPhotos')}</li>
      </ul>
      <p>{t('merge.sendOnlyTrusted')}</p>
    </Banner>
    {#if sizeError}
      <Banner kind="error">{sizeError}</Banner>
    {:else if size}
      <p class="size">
        {t('merge.sendSize', {
          size: sizeText(size.bytes),
          parts: sizeParts(size),
        })}
      </p>
    {:else}
      <p class="size">{t('merge.sendSizeLoading')}</p>
    {/if}
  </div>
  {#snippet footer()}
    <Button onclick={onClose}>{t('common.close')}</Button>
    <Button variant="primary" disabled={making || !size} onclick={make}>{t('merge.sendButton')}</Button>
  {/snippet}
</Dialog>

<style>
  .send {
    display: grid;
    gap: var(--s-5);
  }
  .lead {
    font-weight: 600;
    margin: 0 0 var(--s-2);
  }
  ul {
    margin: 0 0 var(--s-2);
    padding-inline-start: var(--s-5);
  }
  .size {
    font-weight: 500;
  }
</style>

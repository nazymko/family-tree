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
      notices.show({
        message: t('merge.sendDone', { file: fileOf(r.path), size: sizeText(r.bytes) }),
        actionLabel: t('merge.reveal'),
        onaction: () => void platform.reveal(r.path),
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
          people: plural('merge.sendPeopleCount', size.people),
          photos: plural('merge.sendPhotoCount', size.photos),
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

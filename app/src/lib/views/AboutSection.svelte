<script lang="ts">
  import { onMount } from 'svelte';
  import Banner from '../ui/Banner.svelte';
  import Button from '../ui/Button.svelte';
  import { getApi } from '../api/context';
  import type { AppInfo, UpdateStatus } from '../api/types';
  import { t } from '../i18n';
  import { LICENCE, RELEASES_URL, REPORT_URL, shortDate, versionLine } from './about';

  const api = getApi();

  let info = $state<AppInfo | null>(null);
  // null: not checked yet. The check runs only when the button is pressed; nothing here asks the network by itself.
  let status = $state<UpdateStatus | null>(null);
  let checking = $state(false);
  let openFailed = $state('');

  onMount(() => {
    void api
      .appInfo()
      .then((i) => (info = i))
      .catch(() => (info = null));
  });

  async function check() {
    checking = true;
    status = null;
    openFailed = '';
    try {
      status = await api.checkForUpdate();
    } catch {
      status = { kind: 'failed', reason: 'offline' };
    } finally {
      checking = false;
    }
  }

  async function openLink(url: string) {
    openFailed = '';
    const ok = await api.openProjectLink(url).catch(() => false);
    if (!ok) openFailed = t('about.openFailed', { url });
  }

  const failText = (reason: 'offline' | 'server' | 'badReply') =>
    reason === 'offline' ? t('about.offline') : reason === 'server' ? t('about.server') : t('about.badReply');
</script>

<section class="about" aria-labelledby="about-h">
  <h2 id="about-h">{t('about.title')}</h2>
  <p class="name">Family Tree</p>
  <p class="byline">{t('about.byline')}</p>
  {#if info}
    <p class="version tnum" data-testid="version-line">{versionLine(info)}</p>
    {#if !info.release && info.version !== info.base}<p class="hint">{t('about.ahead', { base: info.base })}</p>{/if}
    {#if info.release && info.date}<p class="hint">{t('about.built', { date: shortDate(info.date) })}</p>{/if}
  {/if}

  <div class="links">
    <a href={RELEASES_URL} onclick={(e) => (e.preventDefault(), void openLink(RELEASES_URL))}>{t('about.releases')}</a>
    <a href={REPORT_URL} onclick={(e) => (e.preventDefault(), void openLink(REPORT_URL))}>{t('about.report')}</a>
  </div>
  <p class="hint">{t('about.licence', { licence: LICENCE })}</p>

  <div class="check">
    <Button onclick={check} disabled={checking}>{checking ? t('about.checking') : t('about.check')}</Button>
    <p class="hint">{t('about.privacy')}</p>
  </div>

  {#if status?.kind === 'latest'}
    <Banner>{t('about.upToDate', { latest: status.latest })}</Banner>
  {:else if status?.kind === 'newer'}
    {@const newer = status}
    <Banner>
      <span class="newer">
        <span>
          {t('about.newer', { version: newer.version })}
          {#if newer.published}{t('about.published', { date: shortDate(newer.published) })}{/if}
        </span>
        <Button variant="primary" onclick={() => openLink(newer.url)}>{t('about.download')}</Button>
      </span>
    </Banner>
  {:else if status?.kind === 'failed'}
    <Banner kind="warning">{failText(status.reason)}</Banner>
  {/if}
  {#if openFailed}<Banner kind="warning">{openFailed}</Banner>{/if}
</section>

<style>
  .about {
    display: grid;
    gap: var(--s-4);
    padding-top: var(--s-5);
    border-top: 1px solid var(--rule);
  }
  h2,
  p {
    margin: 0;
  }
  .name {
    font-weight: 500;
  }
  .byline,
  .hint {
    color: var(--ink-2);
  }
  .links {
    display: flex;
    flex-wrap: wrap;
    gap: var(--s-5);
  }
  .links a {
    display: inline-flex;
    align-items: center;
    min-height: 2.25rem;
  }
  .check {
    display: grid;
    justify-items: start;
    gap: var(--s-3);
  }
  .newer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--s-4);
  }
</style>

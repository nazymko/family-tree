<script lang="ts">
  import Select from '../ui/Select.svelte';
  import TextField from '../ui/TextField.svelte';
  import Combobox from '../ui/Combobox.svelte';
  import Banner from '../ui/Banner.svelte';
  import { getApi } from '../api/context';
  import { settings } from '../state/settings.svelte';
  import { t, LANGUAGES } from '../i18n';
  import type { Id, SearchHitDto, Settings } from '../api/api';

  const api = getApi();
  let error = $state('');

  let threshold = $state('100');
  let backups = $state('10');
  let homeText = $state('');
  let homeItems = $state<SearchHitDto[]>([]);
  let homeName = $state('');
  let seq = 0;

  $effect(() => {
    if (!settings.loaded) void settings.load(api);
  });
  $effect(() => {
    threshold = String(settings.value.livingMaxAge);
    backups = String(settings.value.backupsToKeep);
  });
  $effect(() => {
    const id = settings.value.homePerson;
    if (!id) {
      homeName = '';
      homeText = '';
      return;
    }
    let live = true;
    void api.getPerson(id).then((p) => {
      if (!live) return;
      homeName = p.ref.name;
      homeText = p.ref.name;
    });
    return () => {
      live = false;
    };
  });
  $effect(() => {
    const q = homeText.trim();
    const mine = ++seq;
    if (!q || q === homeName) {
      homeItems = [];
      return;
    }
    void api.search(q, 8).then((r) => {
      if (mine === seq) homeItems = r.filter((h) => h.kind === 'person');
    });
  });

  async function change(patch: Partial<Settings>) {
    error = '';
    try {
      await settings.update(api, patch);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  const clamp = (value: unknown, lo: number, hi: number, fallback: number) => {
    const raw = String(value ?? '');
    const n = Math.round(Number(raw));
    return Number.isFinite(n) && raw.trim() !== '' ? Math.min(hi, Math.max(lo, n)) : fallback;
  };

  function commitThreshold() {
    const n = clamp(threshold, 1, 150, settings.value.livingMaxAge);
    threshold = String(n);
    if (n !== settings.value.livingMaxAge) void change({ livingMaxAge: n });
  }
  function commitBackups() {
    const n = clamp(backups, 1, 50, settings.value.backupsToKeep);
    backups = String(n);
    if (n !== settings.value.backupsToKeep) void change({ backupsToKeep: n });
  }
  const pickHome = (h: SearchHitDto) => {
    const id: Id = h.person?.id ?? h.id;
    homeName = h.person?.name ?? h.label;
    homeText = homeName;
    void change({ homePerson: id });
  };
</script>

<section class="settings">
  <h1>{t('settings.title')}</h1>
  {#if error}<Banner kind="error">{error}</Banner>{/if}
  <div class="form">
    <Select
      label={t('settings.dateOrder')}
      hint={t('settings.dateOrderHint')}
      options={[
        { value: 'dmy', label: t('settings.dayMonthYear') },
        { value: 'mdy', label: t('settings.monthDayYear') },
      ]}
      value={settings.value.dayOrder}
      onchange={(v) => change({ dayOrder: v as Settings['dayOrder'] })}
    />
    <Select
      label={t('settings.nameOrder')}
      options={[
        { value: 'given-first', label: t('settings.givenFirst') },
        { value: 'surname-first', label: t('settings.surnameFirst') },
      ]}
      value={settings.value.nameOrder}
      onchange={(v) => change({ nameOrder: v as Settings['nameOrder'] })}
    />
    <TextField
      label={t('settings.livingThreshold')}
      type="number"
      min="1"
      max="150"
      hint={t('settings.livingThresholdHint', { age: settings.value.livingMaxAge })}
      bind:value={threshold}
      onchange={commitThreshold}
    />
    <Combobox
      label={t('settings.homePerson')}
      items={homeItems}
      value={null}
      text={homeText}
      onText={(s) => (homeText = s)}
      onPick={pickHome}
      render={(h) => h.person?.name ?? h.label}
      hint={t('settings.homePersonHint')}
    />
    <Select
      label={t('settings.language')}
      hint={t('settings.languageHint')}
      options={[
        { value: 'system', label: t('settings.languageSystem') },
        ...LANGUAGES.map((l) => ({ value: l.code, label: l.name })),
      ]}
      value={settings.value.language}
      onchange={(v) => change({ language: v as Settings['language'] })}
    />
    <Select
      label={t('settings.theme')}
      options={[
        { value: 'light', label: t('settings.themeLight') },
        { value: 'dark', label: t('settings.themeDark') },
        { value: 'system', label: t('settings.themeSystem') },
      ]}
      value={settings.value.theme}
      onchange={(v) => change({ theme: v as Settings['theme'] })}
    />
    <TextField label={t('settings.backups')} type="number" min="1" max="50" bind:value={backups} onchange={commitBackups} />
  </div>
</section>

<style>
  .settings {
    display: grid;
    gap: var(--s-5);
    max-width: var(--measure);
  }
  .form {
    display: grid;
    gap: var(--s-5);
  }
</style>

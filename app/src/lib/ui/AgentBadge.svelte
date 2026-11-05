<script lang="ts">
  import Badge from './Badge.svelte';
  import { t } from '../i18n';
</script>

<span title={t('changeLog.badgeTitle')}><Badge tone="warning">{t('changeLog.badge')}</Badge></span>

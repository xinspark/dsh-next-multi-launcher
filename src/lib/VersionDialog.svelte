<script lang="ts">
  import Modal from './Modal.svelte';
  import type { VersionInfo } from './types';
  import { api, errorText, truncate, type UiError } from './api';

  interface Props {
    versions: VersionInfo[];
    t: (key: string, vars?: Record<string, string | number>) => string;
    onClose: () => void;
    onChanged: () => void;
  }

  let { versions, t, onClose, onChanged }: Props = $props();

  let dir = $state('');
  let error = $state<string | null>(null);
  let busy = $state(false);

  async function browse() {
    error = null;
    try {
      const picked = await api.pickDirectory();
      if (picked) dir = picked;
    } catch (err) {
      error = errorText(err as UiError);
    }
  }

  async function importVersion() {
    if (!dir.trim()) return;
    busy = true;
    error = null;
    try {
      await api.importVersion(dir.trim());
      dir = '';
      onChanged();
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      busy = false;
    }
  }

  async function remove(tag: string) {
    busy = true;
    error = null;
    try {
      await api.removeVersion(tag);
      onChanged();
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t('dialog.version.title')} {onClose}>
  <div class="field-hint">{t('dialog.version.hint')}</div>

  <div class="field">
    <div class="field-label"><span>{t('dialog.version.path')}</span></div>
    <div class="inline">
      <input class="input" bind:value={dir} placeholder="E:\FXS\APP\DSH NEXT" />
      <button class="btn ghost" onclick={browse} disabled={busy}>{t('dialog.version.browse')}</button>
      <button class="btn primary" onclick={importVersion} disabled={busy || !dir.trim()}>
        {t('dialog.version.import')}
      </button>
    </div>
  </div>

  {#if error}
    <div class="error-text">{error}</div>
  {/if}

  <div class="divider"></div>

  <div class="field-label"><span>{t('dialog.version.available')}</span></div>
  {#if versions.length === 0}
    <div class="empty">{t('dialog.version.none')}</div>
  {:else}
    {#each versions as v (v.tag)}
      <div class="version-row" class:invalid={!v.valid}>
        <span class="tag">{v.tag}</span>
        <span class="field-hint">
          {v.valid ? `内核 ${v.kernelVersion}` : t('dialog.version.invalid', { reason: v.invalidReason ?? '?' })}
        </span>
        <span class="spacer"></span>
        <span class="mono" title={v.isLink ? v.dir + ' → ' + (v.linkTarget ?? '?') : v.dir}>
          {v.isLink ? truncate(v.linkTarget ?? '?', 28) : truncate(v.dir, 28)}
        </span>
        <span class="badge" title={v.isLink ? t('version.link', { target: v.linkTarget ?? '?' }) : v.dir}>
          {v.isLink ? t('version.linkBadge') : t('version.realDir')}
        </span>
        <button class="btn tiny danger" onclick={() => remove(v.tag)} disabled={busy}>
          {t('dialog.version.remove')}
        </button>
      </div>
    {/each}
  {/if}

  <div class="modal-foot">
    <button class="btn ghost" onclick={onClose}>{t('dialog.cancel')}</button>
  </div>
</Modal>

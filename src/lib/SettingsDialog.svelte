<script lang="ts">
  import Modal from './Modal.svelte';
  import { untrack } from 'svelte';
  import { errorText, type UiError } from './api';
  import type { Config } from './types';

  interface Props {
    config: Config;
    isAdmin: boolean;
    t: (key: string, vars?: Record<string, string | number>) => string;
    onClose: () => void;
    onSave: (patch: {
      homeRoot: string;
      defaultHome: string;
      launchTimeoutSec: number;
      pollIntervalMs: number;
      restorePointerAfterLaunch: boolean;
    }) => Promise<void>;
  }

  let { config, isAdmin, t, onClose, onSave }: Props = $props();

  let homeRoot = $state(untrack(() => config.homeRoot ?? ''));
  let defaultHome = $state(untrack(() => config.defaultHome ?? ''));
  let timeoutSec = $state(untrack(() => config.launchTimeoutSec));
  let pollMs = $state(untrack(() => config.pollIntervalMs));
  let restorePointer = $state(untrack(() => config.restorePointerAfterLaunch));
  let error = $state<string | null>(null);
  let saving = $state(false);

  const defaultHomePreview = $derived(
    `${(homeRoot.trim() || '（%USERPROFILE%）').replace(/[\\/]+$/, '')}\\.dsh-<ID>`
  );

  async function submit() {
    error = null;
    saving = true;
    try {
      await onSave({
        homeRoot: homeRoot.trim(),
        defaultHome: defaultHome.trim(),
        launchTimeoutSec: timeoutSec,
        pollIntervalMs: pollMs,
        restorePointerAfterLaunch: restorePointer
      });
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      saving = false;
    }
  }
</script>

<Modal title={t('settings.title')} {onClose}>
  <div class="field">
    <div class="field-label">
      <span>{t('settings.homeRoot')}</span>
      <span class="field-hint">{t('settings.homeRootHint')}</span>
    </div>
    <input class="input" bind:value={homeRoot} placeholder="C:\\Users\\你的用户名" />
    <span class="field-hint">{t('settings.homeRootPreview', { sample: defaultHomePreview })}</span>
  </div>

  <div class="field">
    <div class="field-label">
      <span>{t('settings.defaultHome')}</span>
      <span class="field-hint">{t('settings.defaultHomeHint')}</span>
    </div>
    <input class="input" bind:value={defaultHome} />
  </div>

  <div class="field">
    <div class="field-label"><span>{t('settings.timeout')}</span></div>
    <input class="input" type="number" min="10" max="600" bind:value={timeoutSec} />
    <span class="field-hint">{t('settings.timeoutHint')}</span>
  </div>

  <div class="field">
    <div class="field-label"><span>{t('settings.poll')}</span></div>
    <input class="input" type="number" min="100" max="5000" step="100" bind:value={pollMs} />
    <span class="field-hint">{t('settings.pollHint')}</span>
  </div>

  <div class="field">
    <div class="field-label"><span>{t('settings.permission')}</span></div>
    <span class="field-hint">{isAdmin ? t('settings.admin') : t('settings.standard')}</span>
  </div>

  <div class="field">
    <label class="check">
      <input type="checkbox" bind:checked={restorePointer} />
      <span>{t('settings.restorePointer')}</span>
    </label>
  </div>

  {#if error}
    <div class="error-text">{error}</div>
  {/if}

  <div class="modal-foot">
    <button class="btn ghost" onclick={onClose} disabled={saving}>{t('dialog.cancel')}</button>
    <button class="btn primary" onclick={submit} disabled={saving}>{t('dialog.save')}</button>
  </div>
</Modal>

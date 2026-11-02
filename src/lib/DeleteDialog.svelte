<script lang="ts">
  import Modal from './Modal.svelte';
  import type { InstanceView } from './types';
  import { untrack } from 'svelte';
  import { errorText, type UiError } from './api';

  interface Props {
    view: InstanceView;
    t: (key: string, vars?: Record<string, string | number>) => string;
    onClose: () => void;
    onConfirm: (id: string, deleteData: boolean) => Promise<void>;
  }

  let { view, t, onClose, onConfirm }: Props = $props();

  let typed = $state('');
  let deleteData = $state(untrack(() => !view.isDefault));
  let error = $state<string | null>(null);
  let busy = $state(false);

  const matched = $derived(typed.trim() === view.instance.id);

  async function confirm() {
    if (!matched) return;
    busy = true;
    error = null;
    try {
      await onConfirm(view.instance.id, deleteData && !view.isDefault);
    } catch (err) {
      error = errorText(err as UiError);
      busy = false;
    }
  }
</script>

<Modal title={t('dialog.delete.title')} {onClose}>
  <div class="field">
    <div class="field-label"><span>{t('dialog.delete.hint', { id: view.instance.id })}</span></div>
    <input class="input" bind:value={typed} placeholder={view.instance.id} />
  </div>

  <div class="field">
    <div class="field-label"><span>{t('detail.home')}</span></div>
    <div class="mono" title={view.instance.home}>{view.instance.home}</div>
    {#if view.isDefault}
      <div class="field-hint">{t('dialog.delete.lockedDefault')}</div>
    {/if}
  </div>

  <label class="checkbox">
    <input type="checkbox" bind:checked={deleteData} disabled={view.isDefault} />
    <span>{t('dialog.delete.withData')}</span>
  </label>

  {#if deleteData && !view.isDefault}
    <div class="error-text">{t('dialog.delete.path', { path: view.instance.home })}</div>
  {/if}

  {#if error}
    <div class="error-text">{error}</div>
  {/if}

  <div class="modal-foot">
    <button class="btn ghost" onclick={onClose} disabled={busy}>{t('dialog.cancel')}</button>
    <button class="btn danger" onclick={confirm} disabled={!matched || busy}>
      {t('dialog.delete.confirm')}
    </button>
  </div>
</Modal>

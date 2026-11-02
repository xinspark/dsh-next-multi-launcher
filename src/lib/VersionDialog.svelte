<script lang="ts">
  import Modal from './Modal.svelte';
  import type { VersionInfo, DiscoverCandidate } from './types';
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

  /** 自动扫描结果 */
  let candidates = $state<DiscoverCandidate[]>([]);
  let scanning = $state(false);
  let scanned = $state(false);
  /** 勾选的候选路径 */
  let selected = $state<Set<string>>(new Set());

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

  async function scan() {
    scanning = true;
    error = null;
    scanned = false;
    try {
      const found = await api.discoverVersions();
      candidates = found;
      // 默认勾选：有效且未登记
      selected = new Set(found.filter((c) => c.valid && !c.registered).map((c) => c.path));
      scanned = true;
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      scanning = false;
    }
  }

  function toggle(path: string) {
    const next = new Set(selected);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    selected = next;
  }

  async function importSelected() {
    if (selected.size === 0) return;
    busy = true;
    error = null;
    let failed = 0;
    try {
      for (const path of selected) {
        try {
          await api.importVersion(path);
        } catch {
          failed += 1;
        }
      }
      onChanged();
      candidates = candidates.filter((c) => !selected.has(c.path) || !c.valid);
      selected = new Set();
      if (failed > 0) error = t('dialog.version.partialFail', { count: failed });
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

  function sourceLabel(source: string) {
    const key = `dialog.version.source.${source}`;
    const text = t(key);
    return text === key ? source : text;
  }
</script>

<Modal title={t('dialog.version.title')} {onClose}>
  <div class="field-hint">{t('dialog.version.hint')}</div>

  <div class="field">
    <div class="field-label"><span>{t('dialog.version.path')}</span></div>
    <div class="inline">
      <input class="input" bind:value={dir} placeholder="E:\\FXS\\APP\\DSH NEXT" />
      <button class="btn ghost" onclick={browse} disabled={busy}>{t('dialog.version.browse')}</button>
      <button class="btn primary" onclick={importVersion} disabled={busy || !dir.trim()}>
        {t('dialog.version.import')}
      </button>
    </div>
  </div>

  <div class="inline">
    <button class="btn ghost" onclick={scan} disabled={busy || scanning}>
      {scanning ? t('dialog.version.scanning') : t('dialog.version.scan')}
    </button>
    {#if scanned}
      <span class="field-hint">
        {candidates.length === 0
          ? t('dialog.version.scanEmpty')
          : t('dialog.version.scanFound', { count: candidates.length })}
      </span>
    {/if}
  </div>

  {#if error}
    <div class="error-text">{error}</div>
  {/if}

  {#if candidates.length > 0}
    <div class="field-label"><span>{t('dialog.version.candidates')}</span></div>
    {#each candidates as c (c.path)}
      <div class="version-row" class:invalid={!c.valid}>
        <input
          type="checkbox"
          checked={selected.has(c.path)}
          disabled={!c.valid || c.registered}
          onchange={() => toggle(c.path)}
        />
        <span class="tag">{sourceLabel(c.source)}</span>
        <span class="field-hint">
          {#if c.valid}
            {c.shellVersion} · 内核 {c.kernelVersion}
          {:else}
            {t('dialog.version.invalidDir')}
          {/if}
        </span>
        <span class="spacer"></span>
        <span class="mono" title={c.path}>{truncate(c.path, 34)}</span>
        {#if c.registered}
          <span class="badge">{t('dialog.version.registered')}</span>
        {/if}
      </div>
    {/each}
    <div class="inline">
      <button class="btn primary" onclick={importSelected} disabled={busy || selected.size === 0}>
        {t('dialog.version.importSelected', { count: selected.size })}
      </button>
    </div>
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

<script lang="ts">
  import { untrack } from 'svelte';
  import type { CatalogEntry, DiscoverCandidate, VersionInfo } from './types';
  import { api, errorText, truncate, type UiError } from './api';
  import { NATIVE_DOWNLOAD_URL, NEXT_OFFICIAL_URL, NEXT_RELEASES_URL, REPO_URL } from './links';

  interface Props {
    versions: VersionInfo[];
    t: (key: string, vars?: Record<string, string | number>) => string;
    onChanged: () => Promise<void> | void;
    onNotify: (text: string, kind?: 'ok' | 'err') => void;
  }

  let { versions, t, onChanged, onNotify }: Props = $props();

  type Channel = 'next' | 'next-preview' | 'native';

  let channel = $state<Channel>('next');
  let catalogs = $state<Record<string, CatalogEntry[]>>({});
  let loading = $state(false);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // 本地导入
  let dir = $state('');
  let candidates = $state<DiscoverCandidate[]>([]);
  let selected = $state<Set<string>>(new Set());
  let scanning = $state(false);
  let scanned = $state(false);

  const registeredTags = $derived(new Set(versions.map((v) => v.tag.toLowerCase())));

  function isInstalled(tag: string): boolean {
    const norm = tag.replace(/^v/i, '').toLowerCase();
    return registeredTags.has(norm) || registeredTags.has(tag.toLowerCase());
  }

  function installedVersion(): string {
    return versions.find((v) => v.valid)?.tag ?? '';
  }

  async function load(ch: Channel, force = false) {
    if (!force && catalogs[ch]) return;
    loading = true;
    error = null;
    try {
      const list = await api.catalogList(ch);
      catalogs = { ...catalogs, [ch]: list };
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    void load(untrack(() => channel));
  });

  function switchChannel(next: Channel) {
    channel = next;
    void load(next);
  }

  function sizeText(bytes: number): string {
    if (!bytes) return '—';
    const mb = bytes / 1024 / 1024;
    return `${mb >= 1024 ? (mb / 1024).toFixed(2) + ' GB' : mb.toFixed(0) + ' MB'}`;
  }

  function dateText(iso?: string): string {
    return iso ? iso.slice(0, 10) : '—';
  }

  async function removeVersion(tag: string) {
    busy = true;
    error = null;
    try {
      await api.removeVersion(tag);
      onNotify(t('version.removed', { tag }));
      await onChanged();
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
      await onChanged();
      candidates = candidates.filter((c) => !selected.has(c.path) || !c.valid);
      selected = new Set();
      if (failed > 0) error = t('dialog.version.partialFail', { count: failed });
      else onNotify(t('version.imported'));
    } finally {
      busy = false;
    }
  }

  async function importManual() {
    if (!dir.trim()) return;
    busy = true;
    error = null;
    try {
      await api.importVersion(dir.trim());
      dir = '';
      await onChanged();
      onNotify(t('version.imported'));
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

  function openExternal(url: string) {
    void api.openUrl(url);
  }
</script>

<div class="page">
  <div class="page-head">
    <h2>{t('versions.title')}</h2>
    <span class="field-hint">{t('versions.subtitle')}</span>
  </div>

  {#if error}
    <div class="error-text">{error}</div>
  {/if}

  <!-- ① 已登记 -->
  <section class="card">
    <div class="card-title">
      <span>{t('versions.registered')}</span>
      <span class="spacer"></span>
      <span class="field-hint">{t('versions.count', { count: versions.length })}</span>
    </div>
    {#if versions.length === 0}
      <div class="empty">{t('dialog.version.none')}</div>
    {:else}
      {#each versions as v (v.tag)}
        <div class="row-item" class:invalid={!v.valid}>
          <span class="tag">{v.tag}</span>
          <span class="field-hint">
            {v.valid
              ? t('versions.kernel', { kernel: v.kernelVersion })
              : t('dialog.version.invalid', { reason: v.invalidReason ?? '?' })}
          </span>
          <span class="spacer"></span>
          <span class="mono" title={v.isLink ? `${v.dir} → ${v.linkTarget ?? '?'}` : v.dir}>
            {v.isLink ? truncate(v.linkTarget ?? '?', 30) : truncate(v.dir, 30)}
          </span>
          <button class="btn tiny danger" onclick={() => removeVersion(v.tag)} disabled={busy}>
            {t('dialog.version.remove')}
          </button>
        </div>
      {/each}
    {/if}
  </section>

  <!-- ② 在线下载 -->
  <section class="card">
    <div class="card-title">
      <span>{t('versions.online')}</span>
      <span class="spacer"></span>
      <span class="field-hint">{t('versions.browserHint')}</span>
    </div>

    <div class="tabs">
      <button class="tab" class:active={channel === 'next'} onclick={() => switchChannel('next')}>
        {t('versions.channel.next')}
      </button>
      <button
        class="tab"
        class:active={channel === 'next-preview'}
        onclick={() => switchChannel('next-preview')}
      >
        {t('versions.channel.preview')}
      </button>
      <button class="tab" class:active={channel === 'native'} onclick={() => switchChannel('native')}>
        {t('versions.channel.native')}
      </button>
    </div>

    {#if channel === 'native'}
      <div class="field-hint warn">{t('versions.nativeWarn')}</div>
    {/if}

    {#if loading && !catalogs[channel]}
      <div class="empty">…</div>
    {:else if !catalogs[channel] || catalogs[channel].length === 0}
      <div class="empty">{t('versions.empty')}</div>
    {:else}
      {#each catalogs[channel] as entry (entry.tag)}
        <div class="row-item">
          <span class="tag">{entry.tag}</span>
          <span class="field-hint">{dateText(entry.publishedAt)}</span>
          <span class="field-hint">{sizeText(entry.size)}</span>
          <span class="spacer"></span>
          {#if isInstalled(entry.tag)}
            <span class="badge">{t('versions.installed')}</span>
          {/if}
          <button
            class="btn tiny ghost"
            onclick={() =>
              openExternal(
                channel === 'native'
                  ? NATIVE_DOWNLOAD_URL
                  : entry.url || NEXT_RELEASES_URL
              )}
          >
            {channel === 'native' ? t('versions.openOfficial') : t('versions.download')}
          </button>
        </div>
      {/each}
      <div class="inline wrap">
        <button class="btn tiny ghost" onclick={() => openExternal(channel === 'native' ? NEXT_OFFICIAL_URL : NEXT_RELEASES_URL)}>
          {t('versions.openReleases')}
        </button>
        <button class="btn tiny ghost" onclick={() => openExternal(REPO_URL)}>GitHub</button>
      </div>
    {/if}
  </section>

  <!-- ③ 本地导入 -->
  <section class="card">
    <div class="card-title">
      <span>{t('versions.local')}</span>
      <span class="spacer"></span>
      {#if installedVersion()}
        <span class="field-hint">{t('versions.current', { tag: installedVersion() })}</span>
      {/if}
    </div>

    <div class="field">
      <div class="field-label"><span>{t('dialog.version.path')}</span></div>
      <div class="inline">
        <input class="input" bind:value={dir} placeholder="E:\\FXS\\APP\\DSH NEXT" />
        <button class="btn ghost" onclick={importManual} disabled={busy || !dir.trim()}>
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

    {#if candidates.length > 0}
      {#each candidates as c (c.path)}
        <div class="row-item" class:invalid={!c.valid}>
          <input
            type="checkbox"
            checked={selected.has(c.path)}
            disabled={!c.valid || c.registered}
            onchange={() => toggle(c.path)}
          />
          <span class="tag">{sourceLabel(c.source)}</span>
          <span class="field-hint">
            {#if c.valid}
              {c.shellVersion} · {t('versions.kernel', { kernel: c.kernelVersion ?? '?' })}
            {:else}
              {t('dialog.version.invalidDir')}
            {/if}
          </span>
          <span class="spacer"></span>
          <span class="mono" title={c.path}>{truncate(c.path, 34)}</span>
          {#if c.registered}<span class="badge">{t('dialog.version.registered')}</span>{/if}
        </div>
      {/each}
      <button class="btn primary" onclick={importSelected} disabled={busy || selected.size === 0}>
        {t('dialog.version.importSelected', { count: selected.size })}
      </button>
    {/if}
  </section>
</div>

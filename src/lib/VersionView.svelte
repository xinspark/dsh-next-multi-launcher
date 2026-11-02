<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { CatalogEntry, DiscoverCandidate, VersionInfo } from './types';
  import { api, errorText, truncate, type UiError } from './api';
  import { NATIVE_DOWNLOAD_URL, NEXT_OFFICIAL_URL, NEXT_RELEASES_URL, REPO_URL } from './links';
  import { online, refreshCurrent } from './refresh.svelte.js';

  interface Props {
    versions: VersionInfo[];
    /** 下载加速前缀（设置里可改）；空串 = 直连 GitHub */
    downloadMirror?: string;
    t: (key: string, vars?: Record<string, string | number>) => string;
    onChanged: () => Promise<void> | void;
    onNotify: (text: string, kind?: 'ok' | 'err') => void;
  }

  let { versions, downloadMirror = '', t, onChanged, onNotify }: Props = $props();

  type Channel = 'next' | 'next-preview' | 'native';

  let channel = $state<Channel>('next');
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

  // 目录与更新检查的状态都在 refresh 模块里：启动拉一次、之后每 10 分钟一次，
  // 此外只有点「刷新」才拉。这里切 tab 只换显示。
  function switchChannel(next: Channel) {
    channel = next;
  }

  async function refresh() {
    await refreshCurrent(channel);
  }
  /** 在线下载默认只列最新的几条，其余折叠（旧版本很少用，列表一长就难扫） */
  const PAGE_SIZE = 3;
  let expanded = $state<Record<string, boolean>>({});

  function shownEntries(ch: Channel): CatalogEntry[] {
    const list = online.catalogs[ch] ?? [];
    return expanded[ch] ? list : list.slice(0, PAGE_SIZE);
  }

  function toggleExpanded(ch: Channel) {
    expanded = { ...expanded, [ch]: !expanded[ch] };
  }

  /** 拼加速前缀：`<mirror>/<原地址>`；没配前缀就用原地址。只对 http(s) 地址生效。 */
  function withMirror(raw: string): string {
    const prefix = (downloadMirror ?? '').trim().replace(/\/+$/, '');
    if (!prefix || !/^https?:\/\//i.test(raw)) return raw;
    return `${prefix}/${raw}`;
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
      onNotify(t('versions.removed', { tag }));
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
      else onNotify(t('versions.imported'));
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
      onNotify(t('versions.imported'));
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
      {#if online.loadedAt}
        <span class="field-hint">{t('versions.lastLoaded', { time: online.loadedAt })}</span>
      {/if}
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
      <span class="spacer"></span>
      <button
        class="btn tiny ghost"
        onclick={refresh}
        disabled={online.loadingPair || online.loadingChannel === channel}
      >
        {online.loadingPair || online.loadingChannel === channel ? t('versions.loading') : t('versions.refresh')}
      </button>
    </div>

    {#if channel === 'native'}
      <div class="field-hint warn">{t('versions.nativeWarn')}</div>
    {/if}

    {#if online.errors[channel]}
      <div class="load-error" role="alert">
        <span class="load-error-mark" aria-hidden="true">!</span>
        <span class="load-error-title">{t('versions.loadFailed')}</span>
        <span class="load-error-detail" title={online.errors[channel]}>
          {truncate(online.errors[channel], 72)}
        </span>
        <span class="spacer"></span>
        <button
          class="btn tiny ghost"
          onclick={refresh}
          disabled={online.loadingChannel === channel}
        >
          {online.loadingChannel === channel ? t('versions.loading') : t('versions.retry')}
        </button>
      </div>
      {#if channel !== 'native'}
        <div class="field-hint">{t('versions.netHint')}</div>
      {/if}
    {:else if online.loadingChannel === channel && !online.catalogs[channel]}
      <div class="loading-row" role="status">
        <span class="spinner" aria-hidden="true"></span>
        <span>{t('versions.loading')}</span>
      </div>
    {:else if !online.catalogs[channel] || online.catalogs[channel].length === 0}
      <div class="empty">{t('versions.empty')}</div>
    {:else}
      {#each shownEntries(channel) as entry (entry.tag)}
        <div class="row-item">
          <span class="tag">{entry.tag === 'latest' ? t('versions.latestTag') : entry.tag}</span>
          {#if entry.tag === 'latest' && channel !== 'native'}
            <span class="badge latest">{t('versions.latest')}</span>
          {/if}
          <span class="badge kind" title={entry.assetName}>
            {entry.kind === 'portable' ? t('versions.portable') : t('versions.installer')}
          </span>
          {#if entry.appVersion}
            <span class="field-hint">{t('versions.appVersion', { version: entry.appVersion })}</span>
          {/if}
          {#if entry.kernelVersion}
            <span class="field-hint">{t('versions.kernel', { kernel: entry.kernelVersion })}</span>
          {:else}
            <span class="field-hint" title={t('versions.kernelUnknownHint')}>
              {t('versions.kernelUnknown')}
            </span>
          {/if}
          <span class="field-hint">{dateText(entry.publishedAt)}</span>
          <span class="field-hint">{sizeText(entry.size)}</span>
          <span class="spacer"></span>
          {#if isInstalled(entry.appVersion ?? entry.tag)}
            <span class="badge">{t('versions.installed')}</span>
          {/if}
          <button
            class="btn tiny ghost"
            title={withMirror(channel === 'native' ? NATIVE_DOWNLOAD_URL : entry.url || NEXT_RELEASES_URL)}
            onclick={() =>
              openExternal(
                withMirror(channel === 'native' ? NATIVE_DOWNLOAD_URL : entry.url || NEXT_RELEASES_URL)
              )}
          >
            {channel === 'native' ? t('versions.openOfficial') : t('versions.download')}
          </button>
        </div>
      {/each}
      {#if (online.catalogs[channel]?.length ?? 0) > PAGE_SIZE}
        <button class="btn tiny ghost expand-toggle" onclick={() => toggleExpanded(channel)}>
          {expanded[channel]
            ? t('versions.collapse')
            : t('versions.showAll', { count: online.catalogs[channel].length })}
        </button>
      {/if}
    {/if}

    <!-- 失败时这两个入口最重要：留在卡片底部，永远可点 -->
    <div class="inline wrap">
      <button
        class="btn tiny ghost"
        onclick={() => openExternal(channel === 'native' ? NEXT_OFFICIAL_URL : NEXT_RELEASES_URL)}
      >
        {t('versions.openReleases')}
      </button>
      <button class="btn tiny ghost" onclick={() => openExternal(REPO_URL)}>GitHub</button>
    </div>
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

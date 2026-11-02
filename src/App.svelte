<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fly } from 'svelte/transition';
  import { listen } from '@tauri-apps/api/event';

  import { api, errorText, type UiError } from './lib/api';
  import { translate, type Lang } from './lib/i18n';
  import type { ConfigPatch, InstanceView, PermissionReport, Snapshot, OnboardingView } from './lib/types';
  import { shortPath } from './lib/format';

  import InstanceRow from './lib/InstanceRow.svelte';
  import DetailsPane from './lib/DetailsPane.svelte';
  import InstanceDialog from './lib/InstanceDialog.svelte';
  import DeleteDialog from './lib/DeleteDialog.svelte';
  import VersionView from './lib/VersionView.svelte';
  import SettingsView from './lib/SettingsView.svelte';
  import OnboardingPanel from './lib/OnboardingPanel.svelte';
  import brandUrl from './assets/brand.png';
  import Modal from './lib/Modal.svelte';

  type View = 'launch' | 'versions' | 'settings';

  type Dialog =
    | { kind: 'new'; presetHome?: string; presetId?: string }
    | { kind: 'edit'; id: string }
    | { kind: 'delete'; id: string }
    | { kind: 'forceStop'; id: string; pid?: number };

  let view = $state<View>('launch');

  let snapshot = $state<Snapshot | null>(null);
  let selectedId = $state<string | null>(null);
  let lang = $state<Lang>('zh');
  let dialog = $state<Dialog | null>(null);
  let toast = $state<{ text: string; kind: 'ok' | 'err' } | null>(null);
  let showArchived = $state(false);
  let now = $state(Date.now());
  let loadError = $state<string | null>(null);
  let booting = $state(true);
  let permission = $state<PermissionReport | null>(null);
  /** 正在重启的实例 id：重启期间显示提示条，实例起来后自动清除 */
  let restartingId = $state<string | null>(null);
  /** 初始化向导（首次使用或从设置页手动重开时非空） */
  let onboarding = $state<OnboardingView | null>(null);
  /** 使用提示：单行轮播，每条 30s，可手动「下一个」，不可关闭（?noticeMs= 仅供调试） */
  const NOTICES = [
    'notice.restartLimit',
    'notice.homeSwitch',
    'notice.dshNextOnly',
    'notice.signing.sha256',
    'notice.signing.smartScreen'
  ] as const;
  const NOTICE_MS =
    Number(new URLSearchParams(location.search).get('noticeMs')) || 30_000;
  let noticeIndex = $state(0);
  /** 当前运行在安全模式的实例（取第一个用于提示条） */
  const safeModeView = $derived(
    snapshot?.instances.find((v) => v.running?.safeMode) ?? null
  );

  let pollTimer: ReturnType<typeof setInterval> | undefined;
  let tickTimer: ReturnType<typeof setInterval> | undefined;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let noticeTimer: ReturnType<typeof setInterval> | undefined;
  let unlisten: (() => void) | undefined;

  const t = (key: string, vars?: Record<string, string | number>) => translate(lang, key, vars);

  function normPath(p: string): string {
    return p.replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();
  }

  function sameHome(a?: string, b?: string): boolean {
    if (!a || !b) return false;
    return normPath(a) === normPath(b);
  }

  function notify(text: string, kind: 'ok' | 'err' = 'ok') {
    toast = { text, kind };
    if (toastTimer) clearTimeout(toastTimer);
    if (noticeTimer) clearInterval(noticeTimer);
    toastTimer = setTimeout(() => (toast = null), kind === 'err' ? 6000 : 2600);
  }

  async function refresh() {
    try {
      const next = await api.snapshot();
      void api
        .permissionReport()
        .then((p) => (permission = p))
        .catch(() => (permission = null));
      snapshot = next;
      loadError = null;
      if (!selectedId || !next.instances.some((v) => v.instance.id === selectedId)) {
        const first = next.instances.find((v) => !v.instance.archived) ?? next.instances[0];
        selectedId = first?.instance.id ?? null;
      }
    } catch (err) {
      loadError = errorText(err as UiError);
    } finally {
      booting = false;
    }
  }

  /** 向导打开时的刷新：同时把它的 versions 拉新。
   *  否则刚导入的版本不会出现在面板下拉里，看起来像「导入没生效」。 */
  async function refreshOnboarding() {
    await refresh();
    if (!onboarding) return;
    try {
      onboarding = await api.onboardingState();
    } catch {
      /* 取不到就沿用旧数据，不打断用户 */
    }
  }

  async function guarded(action: () => Promise<void>) {
    try {
      await action();
      await refresh();
    } catch (err) {
      notify(errorText(err as UiError), 'err');
    }
  }

  /** 切到下一条提示；手动切换会重置轮播计时，保证新的一条也能完整停留 */
  function advanceNotice() {
    noticeIndex = (noticeIndex + 1) % NOTICES.length;
    if (noticeTimer) {
      clearInterval(noticeTimer);
      noticeTimer = setInterval(advanceNotice, NOTICE_MS);
    }
  }

    onMount(async () => {
    const stored = localStorage.getItem('dshla.lang');
    if (stored === 'en' || stored === 'zh') lang = stored;
    await refresh();
    // 首次使用：从未完成/跳过向导时自动展开（设计见 docs/design-onboarding.md）
    try {
      const ob = await api.onboardingState();
      if (ob.autoOpen) onboarding = ob;
    } catch {
      /* 取不到就不打扰用户 */
    }
    if (location.hash === '#settings') view = 'settings';
    else if (location.hash === '#versions') view = 'versions';
    else if (location.hash === '#launch') view = 'launch';
    pollTimer = setInterval(refresh, 2000);
    noticeTimer = setInterval(advanceNotice, NOTICE_MS);
    tickTimer = setInterval(() => (now = Date.now()), 1000);
    try {
      unlisten = await listen('launch:progress', () => {
        void refresh();
      });
    } catch {
      /* 浏览器预览时没有 Tauri 事件，忽略 */
    }
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
    if (tickTimer) clearInterval(tickTimer);
    if (toastTimer) clearTimeout(toastTimer);
    unlisten?.();
  });

  function toggleLang() {
    lang = lang === 'zh' ? 'en' : 'zh';
    localStorage.setItem('dshla.lang', lang);
  }

  const activeViews = $derived(snapshot?.instances.filter((v) => !v.instance.archived) ?? []);
  const archivedViews = $derived(snapshot?.instances.filter((v) => v.instance.archived) ?? []);
  const selectedView = $derived(snapshot?.instances.find((v) => v.instance.id === selectedId) ?? null);
  const runningCount = $derived(activeViews.filter((v) => v.running).length);

  /** 一个可用版本都没有：连新建实例都做不了，得先把用户引到版本页 */
  const noVersionWarning = $derived(
    snapshot ? !snapshot.versions.some((v) => v.valid) : false
  );

  const defaultHomeWarning = $derived.by(() => {
    if (!snapshot) return null;
    const home = snapshot.config.defaultHome;
    if (!home) return null;
    const has = snapshot.instances.some((v) => sameHome(v.instance.home, home));
    return has ? null : home;
  });

  const unknownPointerWarning = $derived.by(() => {
    if (!snapshot) return null;
    const home = snapshot.pointer.home;
    if (!home) return null;
    if (snapshot.running.length > 0) return null;
    const has = snapshot.instances.some((v) => sameHome(v.instance.home, home));
    return has ? null : home;
  });

  const queueText = $derived.by(() => {
    if (!snapshot || snapshot.queue.length === 0) return t('footer.queueEmpty');
    const names = snapshot.queue.map(
      (id) => snapshot?.instances.find((v) => v.instance.id === id)?.instance.name ?? id
    );
    return t('footer.queue', { items: names.join(' → ') });
  });

  const lastLaunch = $derived.by(() => {
    if (!snapshot) return null;
    let best: InstanceView | null = null;
    for (const v of snapshot.instances) {
      const at = v.instance.lastLaunchAt;
      if (!at) continue;
      if (!best || (best.instance.lastLaunchAt ?? '') < at) best = v;
    }
    return best;
  });

  const dialogView = $derived.by(() => {
    if (!dialog || !('id' in dialog) || !snapshot) return null;
    return snapshot.instances.find((v) => v.instance.id === (dialog as { id: string }).id) ?? null;
  });

  const forceStopDialog = $derived(dialog?.kind === 'forceStop' ? dialog : null);

  async function onLaunch(id: string) {
    await guarded(() => api.launch(id));
  }

  async function onStop(id: string) {
    await guarded(async () => {
      const outcome = await api.stop(id, false);
      if (outcome.needsForce) {
        dialog = { kind: 'forceStop', id, pid: outcome.pid };
        return;
      }
      if (outcome.stopped) notify(t('toast.stopped'));
    });
  }

  async function onForceStop() {
    if (!dialog || dialog.kind !== 'forceStop') return;
    const id = dialog.id;
    dialog = null;
    await guarded(async () => {
      await api.stop(id, true);
      notify(t('toast.stopped'));
    });
  }

  async function onCancel(id: string) {
    await guarded(() => api.cancel(id));
  }

  async function onArchive(id: string, archived: boolean) {
    await guarded(async () => {
      await api.archiveInstance(id, archived);
      notify(archived ? t('toast.archived') : t('toast.unarchived'));
    });
  }

  async function onDelete(id: string) {
    dialog = { kind: 'delete', id };
  }

  async function doDelete(id: string, deleteData: boolean) {
    await guarded(async () => {
      await api.deleteInstance(id, deleteData);
      if (selectedId === id) selectedId = null;
      dialog = null;
      notify(t('toast.deleted'));
    });
  }

  async function saveInstance(payload: {
    id: string;
    name: string;
    version: string;
    home: string;
    notes: string;
    isNew: boolean;
    archived: boolean;
  }) {
    const inst = await api.saveInstance(payload);
    selectedId = inst.id;
    if (payload.isNew) dialog = null;
    notify(t('detail.saved'));
    await refresh();
  }

  async function saveSettings(patch: ConfigPatch) {
    await api.setConfig(patch);
    notify(t('toast.settingsSaved'));
    await refresh();
  }

  /** 切视图并同步到 hash（可分享、可刷新保持） */
  function go(next: View) {
    view = next;
    if (location.hash !== `#${next}`) {
      history.replaceState(null, '', `#${next}`);
    }
  }

  async function relaunchAsAdmin() {
    try {
      const ok = await api.relaunchElevated();
      notify(ok ? t('perm.relaunched') : t('perm.relaunch'), ok ? 'ok' : 'err');
    } catch (err) {
      notify(errorText(err as UiError), 'err');
    }
  }

  async function restartInstance(id: string, mode: 'normal' | 'safe' | 'exitSafe') {
    restartingId = id;
    await guarded(async () => {
      notify(t('toast.restarting'));
      await api.restartInstance(id, mode);
    });
  }

  // 重启提示条的生命周期：实例进入 running 或启动失败即收起
  $effect(() => {
    if (!restartingId || !snapshot) return;
    const view = snapshot.instances.find((v) => v.instance.id === restartingId);
    if (view && (view.running || view.launch.phase === 'failed')) {
      restartingId = null;
    }
  });

  async function restorePointer() {
    await guarded(async () => {
      await api.restorePointer();
      notify(t('toast.pointerRestored'));
    });
  }

  /** 设置页「重新运行初始化向导」：清 skippedAt/步骤后把面板重新打开 */
  async function rerunOnboarding() {
    try {
      onboarding = await api.onboardingReset();
      await refresh();
    } catch (err) {
      notify(errorText(err as UiError), 'err');
    }
  }

  async function openPath(path: string, create: boolean) {
    await guarded(() => api.openPath(path, create));
  }

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      notify(t('toast.copied'));
    } catch {
      /* 剪贴板不可用时静默 */
    }
  }

  function createFromDefaultHome(home: string) {
    const last = home.split('\\').filter(Boolean).pop() ?? '';
    const core = last.replace(/^\./, '').replace(/^dsh-?/, '');
    const id = (core || 'default').toLowerCase().replace(/[^a-z0-9-]/g, '-');
    dialog = { kind: 'new', presetHome: home, presetId: id || 'default' };
  }
</script>

<div class="app">
  <header class="titlebar">
    <div class="brand">
      <img class="brand-mark" src={brandUrl} alt="" aria-hidden="true" />
      <div class="brand-text">
        <div class="brand-title">{t('app.title')}</div>
        <div class="brand-sub">{t('app.subtitle')} · {t('app.version', { version: snapshot?.appVersion ?? '—' })}</div>
      </div>
    </div>
    <span class="spacer"></span>
    <nav class="nav-tabs" aria-label="main">
      <button class="nav-tab" class:active={view === 'launch'} onclick={() => go('launch')}>
        {t('nav.launch')}
      </button>
      <button class="nav-tab" class:active={view === 'versions'} onclick={() => go('versions')}>
        {t('nav.versions')}
      </button>
      <button class="nav-tab" class:active={view === 'settings'} onclick={() => go('settings')}>
        {t('nav.settings')}
      </button>
    </nav>
    <span class="spacer"></span>
    <button class="btn ghost" onclick={toggleLang}>{t('header.lang')}</button>
    <button class="btn ghost" onclick={() => refresh()}>{t('header.refresh')}</button>
    {#if view === 'launch'}
      <button class="btn primary" onclick={() => (dialog = { kind: 'new' })}>{t('header.new')}</button>
    {/if}
  </header>

  <!-- 提示条：三视图常驻（讲的是全局约束） -->
  <div class="notice notice-ticker" role="status">
    <span class="notice-mark" aria-hidden="true">i</span>
    {#key noticeIndex}
      <span class="notice-text" in:fly={{ y: 6, duration: 220 }}>{t(NOTICES[noticeIndex])}</span>
    {/key}
    <button class="notice-next" onclick={advanceNotice} title={t('notice.nextHint')}>
      {t('notice.next')}
    </button>
  </div>

  {#if view === 'launch'}
  <div class="pointerbar"><span class="pointer-label">{t('pointer.label')}</span>
    <span class="pointer-arrow">→</span>
    <span class="pointer-value" class:missing={!snapshot?.pointer.home} title={snapshot?.pointer.path ?? ''}>
      {snapshot?.pointer.home ?? t('pointer.missing')}
    </span>
    {#if snapshot && !snapshot.pointer.writable}
      <span class="pointer-badge">{t('pointer.unwritable')}</span>
    {/if}
    <span class="spacer"></span>
    <span class="pointer-badge" title={t('pointer.hint')}>{snapshot?.pointer.path ?? ''}</span>
    <button class="btn tiny ghost" onclick={restorePointer} disabled={!snapshot}>{t('pointer.restore')}</button>
  </div>

  {#if permission && permission.needsElevation && !permission.isAdmin}
    <div class="banner banner-danger">
      <span>{t('perm.banner')}</span>
      <button class="btn tiny primary" onclick={relaunchAsAdmin}>{t('perm.relaunch')}</button>
    </div>
  {:else if snapshot && !snapshot.pointer.writable}
    <div class="banner">
      <span>{t('warn.pointerUnwritable', { path: shortPath(snapshot.pointer.path, 60) })}</span>
    </div>
  {/if}

  {#if restartingId}
    {#each snapshot?.instances ?? [] as v (v.instance.id)}
      {#if v.instance.id === restartingId}
        <div class="banner banner-info">
          <span class="banner-dot" aria-hidden="true"></span>
          <span>{t('restart.banner.restarting', { name: v.instance.name })}</span>
        </div>
      {/if}
    {/each}
  {:else if safeModeView}
    <div class="banner banner-warn">
      <span>{t('restart.banner.safe', { name: safeModeView.instance.name })}</span>
      <span class="spacer"></span>
      <button
        class="btn tiny ghost"
        onclick={() => restartInstance(safeModeView!.instance.id, 'exitSafe')}
      >
        {t('restart.banner.exitSafe')}
      </button>
    </div>
  {/if}

  {#if noVersionWarning}
    <div class="banner banner-warn">
      <span>{t('warn.noVersion')}</span>
      <span class="spacer"></span>
      <button class="btn tiny ghost" onclick={() => go('versions')}>
        {t('warn.noVersionAction')}
      </button>
    </div>
  {/if}

  {#if defaultHomeWarning && !noVersionWarning}
    <div class="banner">
      <span>{t('warn.defaultHomeNoInstance', { home: shortPath(defaultHomeWarning, 52) })}</span>
      <span class="spacer"></span>
      <button class="btn tiny ghost" onclick={() => createFromDefaultHome(defaultHomeWarning!)}>
        {t('warn.defaultHomeAction')}
      </button>
    </div>
  {/if}

  {#if unknownPointerWarning}
    <div class="banner">
      <span>{t('warn.unknownPointer', { home: shortPath(unknownPointerWarning, 52) })}</span>
      <span class="spacer"></span>
      <button class="btn tiny ghost" onclick={restorePointer}>{t('warn.unknownPointerAction')}</button>
    </div>
  {/if}

  <main class="main">
    <section class="list-pane">
      <div class="pane-head">
        <span>{t('list.title')}</span>
        <span class="spacer"></span>
        <span>{t('footer.counts', { running: runningCount, total: activeViews.length })}</span>
      </div>

      <div class="list">
        {#if booting}
          <div class="empty">…</div>
        {:else if loadError}
          <div class="empty error-text">{loadError}</div>
        {:else if activeViews.length === 0}
          <div class="empty">{t('list.empty')}</div>
        {:else}
          {#each activeViews as view (view.instance.id)}
            <InstanceRow
              {view}
              {t}
              {now}
              selected={selectedId === view.instance.id}
              onSelect={(id) => (selectedId = id)}
              onLaunch={onLaunch}
              onStop={onStop}
              onRestart={restartInstance}
              onCancel={onCancel}
              onArchive={onArchive}
              onDelete={onDelete}
            />
          {/each}
        {/if}

        {#if archivedViews.length > 0}
          <div
            class="archive-toggle"
            role="button"
            tabindex="0"
            onclick={() => (showArchived = !showArchived)}
            onkeydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') showArchived = !showArchived;
            }}
          >
            <span>{showArchived ? '▾' : '▸'}</span>
            <span>{t('list.archivedCount', { count: archivedViews.length })}</span>
          </div>
          {#if showArchived}
            {#each archivedViews as view (view.instance.id)}
              <InstanceRow
                {view}
                {t}
                {now}
                selected={selectedId === view.instance.id}
                onSelect={(id) => (selectedId = id)}
                onLaunch={onLaunch}
                onStop={onStop}
                onRestart={restartInstance}
                onCancel={onCancel}
                onArchive={onArchive}
                onDelete={onDelete}
              />
            {/each}
          {/if}
        {/if}
      </div>
    </section>

    {#if selectedView}
      <DetailsPane
        view={selectedView}
        versions={snapshot?.versions ?? []}
        {t}
        {now}
        onSaved={() => notify(t('detail.saved'))}
        onOpenPath={openPath}
        onCopy={copyText}
        onDelete={onDelete}
        onLaunch={onLaunch}
        onStop={onStop}
        onRestart={restartInstance}
        save={saveInstance}
        logsDir={snapshot?.logDir ?? ''}
      />
    {:else}
      <aside class="details">
        <div class="details-title">{t('detail.title')}</div>
        <div class="empty">{t('detail.empty')}</div>
      </aside>
    {/if}
  </main>

  <footer class="footer">
    <span>{queueText}</span>
    <span class="grow">
      {#if lastLaunch}
        {t('footer.last', {
          name: lastLaunch.instance.name,
          time: lastLaunch.instance.lastLaunchAt?.slice(11, 16) ?? ''
        })}
      {:else}
        {t('footer.noLast')}
      {/if}
    </span>
    <span title={snapshot?.dataDir ?? ''}>{snapshot?.versionsDir ?? ''}</span>
  </footer>
  {:else if view === 'versions'}
    <VersionView
      versions={snapshot?.versions ?? []}
      downloadMirror={snapshot?.config.downloadMirror ?? ''}
      {t}
      onChanged={() => refresh()}
      onNotify={notify}
    />
  {:else}
    {#if snapshot}
      <SettingsView
        config={snapshot.config}
        {lang}
        dataDir={snapshot.dataDir}
        versionsDir={snapshot.versionsDir}
        logDir={snapshot.logDir}
        {t}
        onSave={saveSettings}
        onToggleLang={toggleLang}
        onRerunOnboarding={rerunOnboarding}
        onOpenPath={(p, create) => openPath(p, create ?? false)}
      />
    {:else}
      <div class="page"><div class="empty">…</div></div>
    {/if}
  {/if}
</div>

<!-- 初始化向导：盖在主界面上，首次使用时自动出现 -->
{#if onboarding}
  <OnboardingPanel
    view={onboarding}
    {t}
    onClose={() => {
      onboarding = null;
      void refresh();
    }}
    onOpenVersions={() => {
      onboarding = null;
      go('versions');
    }}
    onNotify={notify}
    onRefresh={refreshOnboarding}
  />
{/if}

{#if toast}
  <div class="toast" class:error={toast.kind === 'err'} role="status">{toast.text}</div>
{/if}

{#if dialog?.kind === 'new'}
  <InstanceDialog
    mode="new"
    versions={snapshot?.versions ?? []}
    userProfile={snapshot?.userProfile ?? ''}
    homeRoot={snapshot?.config.homeRoot ?? ''}
    defaultHome={snapshot?.config.defaultHome ?? ''}
    initialId={dialog.presetId}
    initialHome={dialog.presetHome}
    {t}
    onClose={() => (dialog = null)}
    onSubmit={saveInstance}
  />
{:else if dialog?.kind === 'edit' && dialogView}
  <InstanceDialog
    mode="edit"
    instance={dialogView.instance}
    versions={snapshot?.versions ?? []}
    userProfile={snapshot?.userProfile ?? ''}
    homeRoot={snapshot?.config.homeRoot ?? ''}
    defaultHome={snapshot?.config.defaultHome ?? ''}
    {t}
    onClose={() => (dialog = null)}
    onSubmit={saveInstance}
  />
{:else if dialog?.kind === 'delete' && dialogView}
  <DeleteDialog view={dialogView} {t} onClose={() => (dialog = null)} onConfirm={doDelete} />
{:else if forceStopDialog}
  <Modal title={t('dialog.forceStop.title')} onClose={() => (dialog = null)}>
    <div class="field-hint">
      {t('dialog.forceStop.hint', {
        name: snapshot?.instances.find((v) => v.instance.id === forceStopDialog.id)?.instance.name ?? '',
        pid: forceStopDialog.pid ?? '?'
      })}
    </div>
    <div class="modal-foot">
      <button class="btn ghost" onclick={() => (dialog = null)}>{t('dialog.cancel')}</button>
      <button class="btn danger" onclick={onForceStop}>{t('dialog.forceStop.confirm')}</button>
    </div>
  </Modal>
{/if}

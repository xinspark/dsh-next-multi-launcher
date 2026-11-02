<script lang="ts">
  import { untrack } from 'svelte';
  import type { Config, ConfigPatch, UpdateInfo } from './types';
  import type { Lang } from './i18n';
  import { api, errorText, type UiError } from './api';
  import { online, checkUpdate as checkUpdateShared } from './refresh.svelte.js';
  import { REPO_URL, STAR_URL } from './links';

  interface Props {
    config: Config;
    lang: Lang;
    dataDir: string;
    logDir: string;
    versionsDir: string;
    t: (key: string, vars?: Record<string, string | number>) => string;
    onSave: (patch: ConfigPatch) => Promise<void>;
    onToggleLang: () => void;
    onRerunOnboarding: () => void;
    onOpenPath: (path: string, create?: boolean) => Promise<void> | void;
  }

  let { config, lang, dataDir, logDir, versionsDir, t, onSave, onToggleLang, onRerunOnboarding, onOpenPath }: Props =
    $props();

  let homeRoot = $state(untrack(() => config.homeRoot ?? ''));
  let defaultHome = $state(untrack(() => config.defaultHome ?? ''));
  let timeoutSec = $state(untrack(() => config.launchTimeoutSec));
  let pollMs = $state(untrack(() => config.pollIntervalMs));
  let restorePointer = $state(untrack(() => config.restorePointerAfterLaunch));
  let downloadMirror = $state(untrack(() => config.downloadMirror ?? ''));

  /** 实测可用的 GitHub 加速前缀（2026-11）；第三方服务随时会挂，所以只做快捷填充，不做默认值 */
  const MIRROR_PRESETS = ['https://gh-proxy.com/', 'https://ghproxy.net/'];

  let saving = $state(false);
  let error = $state<string | null>(null);

  // 更新检查
  const update = $derived(online.update);
  let checking = $state(false);
  const checkedAt = $derived(online.updateCheckedAt);
  let autoCheck = $state(
    untrack(() => {
      try {
        return localStorage.getItem('dshla.autoUpdate') !== '0';
      } catch {
        return true;
      }
    })
  );

  const rootPreview = $derived(
    `${(homeRoot.trim() || t('settings.rootFallback')).replace(/[\\/]+$/, '')}\\.dsh-<ID>`
  );

  const dirty = $derived(
    homeRoot !== (config.homeRoot ?? '') ||
      defaultHome !== (config.defaultHome ?? '') ||
      timeoutSec !== config.launchTimeoutSec ||
      pollMs !== config.pollIntervalMs ||
      restorePointer !== config.restorePointerAfterLaunch ||
      downloadMirror !== (config.downloadMirror ?? '')
  );

  function reset() {
    homeRoot = config.homeRoot ?? '';
    defaultHome = config.defaultHome ?? '';
    timeoutSec = config.launchTimeoutSec;
    pollMs = config.pollIntervalMs;
    restorePointer = config.restorePointerAfterLaunch;
    downloadMirror = config.downloadMirror ?? '';
    error = null;
  }

  async function save() {
    saving = true;
    error = null;
    try {
      await onSave({
        homeRoot: homeRoot.trim(),
        defaultHome: defaultHome.trim(),
        launchTimeoutSec: timeoutSec,
        pollIntervalMs: pollMs,
        restorePointerAfterLaunch: restorePointer,
        downloadMirror: downloadMirror.trim()
      });
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      saving = false;
    }
  }

  async function pickDefaultHome() {
    try {
      const picked = await api.pickDirectory();
      if (picked) defaultHome = picked;
    } catch (err) {
      error = errorText(err as UiError);
    }
  }

  // 更新检查与版本目录共用一次刷新（启动一次 + 每 10 分钟一次）；这里只做手动触发
  async function checkUpdate() {
    checking = true;
    try {
      await checkUpdateShared(true);
    } finally {
      checking = false;
    }
  }

  function toggleAutoCheck(next: boolean) {
    autoCheck = next;
    try {
      localStorage.setItem('dshla.autoUpdate', next ? '1' : '0');
    } catch {
      /* 忽略 */
    }
    if (next) void checkUpdate();
  }

  function openRepo() {
    void api.openUrl(REPO_URL);
  }

  function openStar() {
    void api.openUrl(STAR_URL);
  }

  $effect(() => {
    if (autoCheck && !update && !checking) void checkUpdate();
  });
</script>

<div class="page">
  <div class="page-head">
    <h2>{t('settings.title')}</h2>
    <span class="field-hint">{t('settings.subtitle')}</span>
  </div>

  <!-- ① 常规 -->
  <section class="card">
    <div class="card-title">{t('settings.group.general')}</div>
    <div class="field">
      <div class="field-label">
        <span>{t('settings.language')}</span>
        <span class="field-hint">{t('settings.languageHint')}</span>
      </div>
      <div class="inline">
        <button class="btn ghost" class:active={lang === 'zh'} onclick={() => lang !== 'zh' && onToggleLang()}>
          中文
        </button>
        <button class="btn ghost" class:active={lang === 'en'} onclick={() => lang !== 'en' && onToggleLang()}>
          English
        </button>
      </div>
    </div>
  </section>

  <!-- ② 数据目录 -->
  <section class="card">
    <div class="card-title">{t('settings.group.paths')}</div>

    <div class="field">
      <div class="field-label">
        <span>{t('settings.homeRoot')}</span>
        <span class="field-hint">{t('settings.homeRootHint')}</span>
      </div>
      <input class="input" bind:value={homeRoot} placeholder={t('settings.rootFallback')} />
      <span class="field-hint">{t('settings.homeRootPreview', { sample: rootPreview })}</span>
    </div>

    <div class="field">
      <div class="field-label">
        <span>{t('settings.defaultHome')}</span>
        <span class="field-hint">{t('settings.defaultHomeHint')}</span>
      </div>
      <div class="inline">
        <input class="input" bind:value={defaultHome} />
        <button class="btn ghost" onclick={pickDefaultHome}>{t('settings.pick')}</button>
        <button class="btn ghost" onclick={() => (defaultHome = '')}>{t('settings.clear')}</button>
      </div>
      {#if !defaultHome.trim()}
        <span class="field-hint warn">{t('settings.defaultHomeEmpty')}</span>
      {/if}
    </div>
  </section>

  <!-- ③ 启动 -->
  <section class="card">
    <div class="card-title">{t('settings.group.launch')}</div>

    <div class="field">
      <div class="field-label">
        <span>{t('settings.timeout')}</span>
        <span class="field-hint">{t('settings.timeoutHint')}</span>
      </div>
      <input class="input narrow" type="number" min="10" max="600" bind:value={timeoutSec} />
    </div>

    <div class="field">
      <div class="field-label">
        <span>{t('settings.poll')}</span>
        <span class="field-hint">{t('settings.pollHint')}</span>
      </div>
      <input class="input narrow" type="number" min="100" max="5000" step="100" bind:value={pollMs} />
    </div>

    <div class="field">
      <label class="check">
        <input type="checkbox" bind:checked={restorePointer} />
        <span>{t('settings.restorePointer')}</span>
      </label>
      {#if !restorePointer}
        <span class="field-hint warn">{t('settings.restorePointerWarn')}</span>
      {/if}
    </div>

    <div class="field">
      <div class="field-label">
        <span>{t('settings.downloadMirror')}</span>
        <span class="field-hint">{t('settings.downloadMirrorHint')}</span>
      </div>
      <input class="input" bind:value={downloadMirror} placeholder="https://gh-proxy.com/" />
      <div class="inline wrap">
        {#each MIRROR_PRESETS as preset (preset)}
          <button class="btn tiny ghost" onclick={() => (downloadMirror = preset)}>{preset}</button>
        {/each}
        <button class="btn tiny ghost" onclick={() => (downloadMirror = '')}>{t('settings.clear')}</button>
      </div>
      <span class="field-hint">{t('settings.downloadMirrorNote')}</span>
    </div>
  </section>

  <!-- ④ 维护 · 关于 -->
  <section class="card">
    <div class="card-title">{t('settings.group.maintenance')}</div>

    <div class="inline wrap">
      <button class="btn ghost" onclick={() => onOpenPath(dataDir, true)}>{t('settings.openDataDir')}</button>
      <button class="btn ghost" onclick={() => onOpenPath(versionsDir, true)}>{t('settings.openVersionsDir')}</button>
      <button class="btn ghost" onclick={() => onOpenPath(logDir, true)}>{t('settings.openLogDir')}</button>
      <button class="btn ghost" onclick={openRepo}>{t('settings.openRepo')}</button>
      <button class="btn ghost" onclick={onRerunOnboarding}>{t('settings.rerunOnboarding')}</button>
    </div>
  </section>

  <section class="card">
    <div class="card-title">{t('settings.group.aboutUpdate')}</div>
    <div class="inline wrap">
      <span class="mono">{t('settings.currentVersion', { version: update?.current || '—' })}</span>
      <button class="btn ghost" onclick={checkUpdate} disabled={checking}>
        {checking ? t('settings.checking') : t('settings.checkUpdate')}
      </button>
      {#if checkedAt}<span class="field-hint">{t('settings.lastChecked', { time: checkedAt })}</span>{/if}
    </div>

    {#if update?.error}
      <div class="field-hint warn">{t('settings.checkFailed', { reason: update.error })}</div>
    {:else if update}
      {#if update.hasUpdate}
        <div class="inline">
          <span class="field-hint">
            {t('settings.updateAvailable', { latest: update.latest ?? '?' })}
          </span>
          {#if update.url}
            <button class="btn tiny primary" onclick={() => update?.url && api.openUrl(update.url)}>
              {t('settings.viewRelease')}
            </button>
          {/if}
        </div>
      {:else}
        <div class="field-hint">{t('settings.upToDate')}</div>
      {/if}
    {/if}

    <label class="check">
      <input type="checkbox" checked={autoCheck} onchange={(e) => toggleAutoCheck(e.currentTarget.checked)} />
      <span>{t('settings.autoCheck')}</span>
    </label>
    <span class="field-hint">{t('settings.autoCheckHint')}</span>
  </section>

  <section class="card star-card">
    <div class="card-title">{t('settings.group.aboutApp')}</div>
    <div class="star-body">
      <div>
        <div class="star-title">{t('settings.star.title')}</div>
        <div class="field-hint">{t('settings.star.body')}</div>
      </div>
      <button class="btn primary" onclick={openStar}>{t('settings.star.action')}</button>
    </div>
    <div class="inline wrap">
      <span class="field-hint">{t('settings.license')}</span>
      <button class="btn tiny ghost" onclick={openRepo}>GitHub</button>
    </div>
  </section>

  {#if error}
    <div class="error-text">{error}</div>
  {/if}

  {#if dirty}
    <div class="save-bar">
      <span class="dot"></span>
      <span>{t('settings.dirty')}</span>
      <span class="spacer"></span>
      <button class="btn ghost" onclick={reset} disabled={saving}>{t('settings.discard')}</button>
      <button class="btn primary" onclick={save} disabled={saving}>
        {saving ? t('settings.saving') : t('dialog.save')}
      </button>
    </div>
  {/if}
</div>

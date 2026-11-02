<script lang="ts">
  import { untrack } from 'svelte';
  import { api, errorText, type UiError } from './api';
  import type { OnboardingView } from './types';
  import { slugify } from './slug';
  import { shortPath } from './format';

  interface Props {
    view: OnboardingView;
    t: (key: string, vars?: Record<string, string | number>) => string;
    /** 关闭面板（完成 / 跳过之后） */
    onClose: () => void;
    /** 去版本页导入安装目录 */
    onOpenVersions: () => void;
    onNotify: (text: string, kind?: 'ok' | 'err') => void;
    onRefresh: () => Promise<void> | void;
  }

  let { view, t, onClose, onOpenVersions, onNotify, onRefresh }: Props = $props();

  const validVersions = $derived(view.versions.filter((v) => v.valid));

  let name = $state('日常');
  let id = $state(untrack(() => slugify('日常')));
  let idTouched = $state(false);
  let version = $state(untrack(() => validVersions[0]?.tag ?? ''));
  let home = $state('');
  let homeTouched = $state(false);

  let busy = $state(false);
  let error = $state<string | null>(null);
  let created = $state(untrack(() => view.hasInstance));
  let phase = $state<string>('');
  let importing = $state(false);

  const rootDir = $derived((view.homeRoot || view.userProfile || '').replace(/[\\/]+$/, ''));

  function defaultHomeFor(idValue: string): string {
    if (!idValue || idValue === 'default') return `${rootDir}\\.dsh`;
    return `${rootDir}\\.dsh-${idValue}`;
  }

  const effectiveHome = $derived(home.trim() || defaultHomeFor(id));
  const canFinish = $derived(Boolean(version) && Boolean(name.trim()) && /^[a-z0-9-]{1,32}$/.test(id));

  function autoId() {
    if (idTouched) return;
    const next = slugify(name);
    id = next;
    if (!homeTouched) home = defaultHomeFor(next);
  }

  $effect(() => {
    // 名称变化 → 自动转写 ID 与默认 home（用户手改过就不再覆盖）
    if (!idTouched) {
      const next = slugify(name);
      if (next !== id) id = next;
    }
  });

  async function ensureInstance(): Promise<string | null> {
    if (created && view.hasInstance) {
      // 已存在（首次运行会自动建 default 实例）
      const snap = await api.snapshot();
      const mine = snap.instances.find((v) => v.instance.id === id);
      if (mine) return mine.instance.id;
    }
    const inst = await api.saveInstance({
      id,
      name: name.trim(),
      version,
      home: effectiveHome,
      notes: '',
      isNew: true,
      archived: false
    });
    created = true;
    await onRefresh();
    return inst.id;
  }

  async function importDir() {
    importing = true;
    error = null;
    try {
      const dir = await api.pickDirectory();
      if (!dir) return;
      const info = await api.importVersion(dir);
      await onRefresh();
      version = info.tag;
      onNotify(t('onboarding.versionImported', { tag: info.tag }));
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      importing = false;
    }
  }

  async function tryLaunch() {
    busy = true;
    error = null;
    try {
      const instId = await ensureInstance();
      if (!instId) return;
      phase = 'preparing';
      await api.launch(instId);
      // 轮询到 running / failed（最多 ~90s），只更新状态文字
      for (let i = 0; i < 90; i++) {
        await new Promise((r) => setTimeout(r, 1000));
        const snap = await api.snapshot();
        const v = snap.instances.find((x) => x.instance.id === instId);
        phase = v?.running ? 'running' : (v?.launch.phase ?? 'idle');
        if (phase === 'running' || phase === 'failed') break;
      }
      if (phase === 'running') onNotify(t('onboarding.launchOk'));
      else if (phase === 'failed') error = t('onboarding.launchFailed');
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      busy = false;
    }
  }

  async function finish() {
    busy = true;
    error = null;
    try {
      if (validVersions.length > 0) await ensureInstance();
      await api.onboardingComplete(4);
      onClose();
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      busy = false;
    }
  }

  async function skip() {
    busy = true;
    try {
      await api.onboardingSkip();
      onClose();
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      busy = false;
    }
  }

  async function elevate() {
    try {
      const ok = await api.relaunchElevated();
      onNotify(ok ? t('onboarding.relaunched') : t('onboarding.relaunchFailed'), ok ? 'ok' : 'err');
    } catch (err) {
      error = errorText(err as UiError);
    }
  }

  const phaseText = $derived(
    phase === 'running'
      ? t('onboarding.phase.running')
      : phase === 'failed'
        ? t('onboarding.phase.failed')
        : phase
          ? t('onboarding.phase.pending')
          : ''
  );
</script>

<div class="backdrop">
  <div class="modal onboarding">
    <div class="onboarding-head">
      <span class="onboarding-title">{t('onboarding.title')}</span>
      <span class="field-hint">{t('onboarding.subtitle')}</span>
    </div>

    <!-- ① 环境体检 -->
    <section class="card">
      <div class="card-title">{t('onboarding.step.env')}</div>
      <div class="check-row">
        <span class:ok={view.environment.dataWritable} class:bad={!view.environment.dataWritable}>
          {view.environment.dataWritable ? '✓' : '⚠'}
        </span>
        <span>{t('onboarding.env.dataDir')}</span>
        <span class="mono" title={view.environment.dataDir}>
          {shortPath(view.environment.dataDir, 48)}
        </span>
      </div>
      <div class="check-row">
        <span class:ok={view.environment.pointerWritable} class:bad={!view.environment.pointerWritable}>
          {view.environment.pointerWritable ? '✓' : '⚠'}
        </span>
        <span>{t('onboarding.env.pointer')}</span>
        <span class="mono" title={view.environment.pointerPath}>
          {shortPath(view.environment.pointerPath, 48)}
        </span>
      </div>
      <div class="check-row">
        <span class="ok">✓</span>
        <span>{t('onboarding.env.webview')}</span>
        <span class="field-hint">{t('onboarding.env.webviewOk')}</span>
      </div>
      {#if !view.environment.pointerWritable}
        <div class="field-hint warn">{t('onboarding.env.pointerWarn')}</div>
        <div class="inline"><button class="btn tiny primary" onclick={elevate}>
          {t('onboarding.env.elevate')}
        </button></div>
      {/if}
    </section>

    <!-- ② 选择版本 -->
    <section class="card">
      <div class="card-title">{t('onboarding.step.version')}</div>
      {#if validVersions.length === 0}
        <div class="field-hint warn">{t('onboarding.version.none')}</div>
        <div class="inline wrap">
          <button class="btn ghost" onclick={importDir} disabled={importing}>
            {importing ? t('onboarding.version.importing') : t('onboarding.version.import')}
          </button>
          <button class="btn tiny ghost" onclick={onOpenVersions}>{t('onboarding.version.goto')}</button>
        </div>
      {:else}
        <select class="select" bind:value={version}>
          {#each validVersions as v (v.tag)}
            <option value={v.tag}>{v.tag} · {t('versions.kernel', { kernel: v.kernelVersion })}</option>
          {/each}
        </select>
        <div class="inline wrap">
          <button class="btn tiny ghost" onclick={importDir} disabled={importing}>
            {importing ? t('onboarding.version.importing') : t('onboarding.version.import')}
          </button>
        </div>
      {/if}
    </section>

    <!-- ③ 第一个实例 -->
    <section class="card">
      <div class="card-title">{t('onboarding.step.instance')}</div>
      <div class="field">
        <div class="field-label"><span>{t('dialog.name')}</span></div>
        <input class="input" bind:value={name} maxlength="40" oninput={autoId} />
      </div>
      <div class="field">
        <div class="field-label">
          <span>{t('dialog.id')}</span>
          <span class="field-hint">{t('onboarding.instance.idHint')}</span>
        </div>
        <input
          class="input"
          bind:value={id}
          oninput={() => {
            idTouched = true;
            if (!homeTouched) home = defaultHomeFor(id);
          }}
        />
      </div>
      <div class="field">
        <div class="field-label"><span>{t('dialog.home')}</span></div>
        <input
          class="input"
          bind:value={home}
          placeholder={defaultHomeFor(id)}
          oninput={() => (homeTouched = true)}
        />
        <span class="field-hint">{t('onboarding.instance.homeHint', { root: rootDir })}</span>
      </div>
    </section>

    <!-- ④ 试启动 -->
    <section class="card">
      <div class="card-title">{t('onboarding.step.try')}</div>
      <div class="inline wrap">
        <button class="btn ghost" onclick={tryLaunch} disabled={busy || !canFinish}>
          {t('onboarding.try.launch')}
        </button>
        {#if phaseText}
          <span class="field-hint">{phaseText}</span>
        {/if}
      </div>
      <div class="field-hint">{t('onboarding.try.hint')}</div>
    </section>

    {#if error}
      <div class="error-text">{error}</div>
    {/if}

    <div class="onboarding-foot">
      <button class="btn ghost" onclick={skip} disabled={busy}>{t('onboarding.skip')}</button>
      <span class="spacer"></span>
      <button class="btn primary" onclick={finish} disabled={busy || !canFinish}>
        {t('onboarding.finish')}
      </button>
    </div>
  </div>
</div>

<script lang="ts">
  import { untrack } from 'svelte';
  import { api, errorText, type UiError } from './api';
  import type { OnboardingView } from './types';
  import { slugify } from './slug';
  import { shortPath } from './format';
  import SelectMenu from './SelectMenu.svelte';
  import DirPickButton from './DirPickButton.svelte';
  import type { Lang } from './i18n';

  interface Props {
    view: OnboardingView;
    /** 当前界面语言（默认跟随系统，见 App 的初始化） */
    lang: Lang;
    t: (key: string, vars?: Record<string, string | number>) => string;
    onSetLang: (lang: Lang) => void;
    /** 应用「数据目录树根」 */
    onSaveRoot: (root: string) => Promise<void>;
    /** 关闭面板（完成 / 跳过之后） */
    onClose: () => void;
    /** 去版本页导入安装目录 */
    onOpenVersions: () => void;
    /** 去主界面的「新建实例」对话框 */
    onCreateInstance: () => void;
    onNotify: (text: string, kind?: 'ok' | 'err') => void;
    onRefresh: () => Promise<void> | void;
  }

  let { view, t, lang, onSetLang, onSaveRoot, onClose, onOpenVersions, onCreateInstance, onNotify, onRefresh }: Props =
    $props();

  const validVersions = $derived(view.versions.filter((v) => v.valid));
  /** 已经存在的实例（最常见是启动器自动建的「默认」） */
  const existing = $derived(view.instances ?? []);

  let name = $state('日常');
  let id = $state(untrack(() => slugify('日常')));
  let idTouched = $state(false);
  let version = $state(untrack(() => validVersions[0]?.tag ?? ''));
  let home = $state('');
  let homeTouched = $state(false);

  let busy = $state(false);
  let error = $state<string | null>(null);
  let phase = $state<string>('');
  let importing = $state(false);

  // ③ 默认数据目录树根：只做「选文件夹」，选完立即生效；留空 = 跟随系统默认
  let busyRoot = $state(false);
  const rootShown = $derived(view.homeRoot?.trim() || view.userProfile || '%USERPROFILE%');
  // 空值，或就是用户目录本身 → 视为「跟随默认」，此时不显示「恢复默认」
  const isDefaultRoot = $derived(
    !view.homeRoot?.trim() ||
      view.homeRoot.trim().toLowerCase() === (view.userProfile || '').toLowerCase()
  );
  const rootPreview = $derived(`${rootShown.replace(/[\\/]+$/, '')}\\.dsh-<ID>`);
  // 树根落在系统盘（与用户目录同盘）时，给一条「换到非系统盘」的建议
  const driveOf = (p: string) => ((p.match(/^[a-zA-Z]:/) || [''])[0] || '').toUpperCase();
  const onSystemDrive = $derived(
    Boolean(driveOf(rootShown)) && driveOf(rootShown) === driveOf(view.userProfile || '')
  );

  /** 选中目录后的落库动作（选目录本身交给共用的 DirPickButton） */
  async function applyRoot(dir: string) {
    busyRoot = true;
    error = null;
    try {
      await onSaveRoot(dir);
      onNotify(t('onboarding.root.saved', { path: shortPath(dir, 48) }));
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      busyRoot = false;
    }
  }

  /** 恢复默认：清空 homeRoot（后端把空串解释为 %USERPROFILE%） */
  async function resetRoot() {
    busyRoot = true;
    error = null;
    try {
      await onSaveRoot('');
      onNotify(t('onboarding.root.reset'));
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      busyRoot = false;
    }
  }
  const rootDir = $derived((view.homeRoot || view.userProfile || '').replace(/[\\/]+$/, ''));

  function defaultHomeFor(idValue: string): string {
    if (!idValue || idValue === 'default') return `${rootDir}\\.dsh`;
    return `${rootDir}\\.dsh-${idValue}`;
  }

  const effectiveHome = $derived(home.trim() || defaultHomeFor(id));
  // 已经实例了就不用再填名字/ID：这一屏只是让用户决定「用它」还是「去新建」
  const canFinish = $derived(
    existing.length > 0 ||
      (Boolean(version) && Boolean(name.trim()) && /^[a-z0-9-]{1,32}$/.test(id))
  );

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
    // 已经有实例（最常见是自动建的「默认」）：直接用它，不再造一条
    if (existing.length > 0) return existing[0].id;
    const inst = await api.saveInstance({
      id,
      name: name.trim(),
      version,
      home: effectiveHome,
      notes: '',
      isNew: true,
      archived: false
    });
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

  /** 去走普通新建流程：先记「已跳过」，否则下次启动又弹一遍向导 */
  async function createNew() {
    busy = true;
    error = null;
    try {
      await api.onboardingSkip();
      onCreateInstance();
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
  <!-- ① 语言（默认跟随系统，可在设置页随时改） -->
  <section class="card">
    <div class="card-title">{t('onboarding.step.lang')}</div>
    <div class="inline">
      <button class="btn ghost" class:active={lang === 'zh'} onclick={() => onSetLang('zh')}>中文</button>
      <button class="btn ghost" class:active={lang === 'en'} onclick={() => onSetLang('en')}>English</button>
    </div>
    <span class="field-hint">{t('onboarding.lang.hint')}</span>
  </section>

  <!-- ② 环境体检 -->
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

    <!-- ③ 默认数据目录树根：只做文件夹选择，选完立即生效 -->
    <section class="card">
      <div class="card-title">{t('onboarding.step.root')}</div>
      <div class="check-row">
        <span class="ok">✓</span>
        <span class="mono" title={view.homeRoot || view.userProfile}>{shortPath(rootShown, 52)}</span>
        {#if isDefaultRoot}
          <span class="field-hint">{t('onboarding.root.following')}</span>
        {/if}
      </div>
      <div class="inline wrap">
        <DirPickButton
          label={t('onboarding.root.pick')}
          busyLabel={t('onboarding.root.picking')}
          disabled={busyRoot}
          onpick={applyRoot}
          onerror={(err) => {
            error = errorText(err as UiError);
          }}
        />
        {#if !isDefaultRoot}
          <button class="btn tiny ghost" onclick={resetRoot} disabled={busyRoot}>
            {t('onboarding.root.reset')}
          </button>
        {/if}
      </div>
      <span class="field-hint">{t('onboarding.root.preview', { sample: rootPreview })}</span>
      {#if onSystemDrive}
        <span class="field-hint">{t('onboarding.root.recommend')}</span>
      {/if}
    </section>

    <!-- ④ 选择版本 -->
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
        <SelectMenu
          value={version}
          label={t('versions.title')}
          options={validVersions.map((v) => ({
            value: v.tag,
            label: `${v.tag} · ${t('versions.kernel', { kernel: v.kernelVersion })}`,
          }))}
          onchange={(next) => (version = next)}
        />
        <div class="inline wrap">
          <button class="btn tiny ghost" onclick={importDir} disabled={importing}>
            {importing ? t('onboarding.version.importing') : t('onboarding.version.import')}
          </button>
        </div>
      {/if}
    </section>

    <!-- ⑤ 实例 -->
    <section class="card">
      <div class="card-title">{t('onboarding.step.instance')}</div>

      {#if existing.length > 0}
        <!-- 已经有实例（启动器会自动建一条「默认」）：不再让用户重复填表，
             而是说清楚「已有」，并给两个出口 -->
        <div class="field-hint">{t('onboarding.instance.exists', { count: existing.length })}</div>
        {#each existing as inst (inst.id)}
          <div class="check-row">
            <span class="ok">✓</span>
            <span class="mono">{inst.name} · {inst.id}</span>
            <span class="mono" title={inst.home}>{shortPath(inst.home, 40)}</span>
          </div>
        {/each}
        <div class="inline wrap">
          <button class="btn ghost" onclick={createNew} disabled={busy}>
            {t('onboarding.instance.createNew')}
          </button>
        </div>
        <div class="field-hint">{t('onboarding.instance.createNewHint')}</div>
      {:else}
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
      {/if}

      <!-- 试启动：可选，失败不阻塞（原本是独立的第 4 块，按新流程并进实例块） -->
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
      {#if !canFinish}
        <span class="field-hint">{t('onboarding.finishBlocked')}</span>
      {/if}
      <span class="spacer"></span>
      <button class="btn primary" onclick={finish} disabled={busy || !canFinish}>
        {t('onboarding.finish')}
      </button>
    </div>
  </div>
</div>

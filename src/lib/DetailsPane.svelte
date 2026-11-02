<script lang="ts">
  import type { InstanceView, VersionInfo } from './types';
  import { hhmm, formatDate, uptime, elapsedSec } from './format';
  import Icon from './Icon.svelte';

  interface Props {
    view: InstanceView;
    versions: VersionInfo[];
    t: (key: string, vars?: Record<string, string | number>) => string;
    now: number;
    onSaved: () => void;
    onOpenPath: (path: string, create: boolean) => void;
    onCopy: (text: string) => void;
    onDelete: (id: string) => void;
    onLaunch: (id: string) => void;
    onStop: (id: string) => void;
    onRestart: (id: string, mode: 'normal' | 'safe' | 'exitSafe') => void;
    save: (payload: {
      id: string;
      name: string;
      version: string;
      home: string;
      notes: string;
      isNew: boolean;
      archived: boolean;
    }) => Promise<void>;
    logsDir: string;
  }

  let { view, versions, t, now, onSaved, onOpenPath, onCopy, onDelete, onLaunch, onStop, onRestart, save, logsDir }: Props =
    $props();

  const inst = $derived(view.instance);
  const running = $derived(Boolean(view.running));
  /** 当前是否安全模式：运行中用进程扫描结果，停止后用上次启动记录 */
  const inSafeMode = $derived(view.running?.safeMode ?? inst.safeMode);
  const pending = $derived(
    !running &&
      (view.launch.phase === 'queued' ||
        view.launch.phase === 'preparing' ||
        view.launch.phase === 'spawning' ||
        view.launch.phase === 'waiting')
  );

  let name = $state('');
  let notes = $state('');
  let version = $state('');
  let saving = $state(false);
  let error = $state<string | null>(null);

  // 选中实例或外部数据变化时同步表单（用户没有未保存修改才覆盖）
  let lastKey = $state('');
  $effect(() => {
    const key = `${inst.id}|${inst.name}|${inst.notes}|${inst.version}`;
    if (key !== lastKey) {
      lastKey = key;
      name = inst.name;
      notes = inst.notes;
      version = inst.version;
      error = null;
    }
  });

  const dirty = $derived(name !== inst.name || notes !== inst.notes || version !== inst.version);

  async function doSave() {
    saving = true;
    error = null;
    try {
      await save({
        id: inst.id,
        name: name.trim() || inst.name,
        version,
        home: inst.home,
        notes,
        isNew: false,
        archived: inst.archived
      });
      onSaved();
    } catch (err) {
      error = (err as { message?: string }).message ?? String(err);
    } finally {
      saving = false;
    }
  }

  const seconds = $derived.by(() => {
    void now;
    return elapsedSec(view.launch.startedAt);
  });
  const uptimeText = $derived.by(() => {
    void now;
    return view.running ? uptime(view.running.startedAt) : '';
  });
  const statusText = $derived.by(() => {
    if (running) return `${t('state.running')} · ${uptimeText}`;
    if (pending) return `${t('state.waiting')} ${seconds}s…`;
    if (view.launch.phase === 'failed') return t('state.failed');
    return t('state.idle');
  });
</script>

<aside class="details">
  <div class="details-title">{t('detail.title')}</div>

  <div class="field">
    <div class="field-label">
      <span>{t('detail.name')}</span>
    </div>
    <input class="input" bind:value={name} maxlength="40" disabled={saving} />
  </div>

  <div class="field">
    <div class="field-label">
      <span>{t('detail.id')}</span>
      <span class="field-hint">{t('detail.idReadonly')}</span>
    </div>
    <input class="input" value={inst.id} readonly />
  </div>

  <div class="field">
    <div class="field-label">
      <span>{t('detail.version')}</span>
    </div>
    <select class="select" bind:value={version} disabled={saving || running || pending}>
      {#if !versions.some((v) => v.tag === inst.version)}
        <option value={inst.version}>{inst.version}（缺失）</option>
      {/if}
      {#each versions as v (v.tag)}
        <option value={v.tag} disabled={!v.valid}>
          {v.tag} · 内核 {v.kernelVersion}{v.valid ? '' : '（无效）'}
        </option>
      {/each}
    </select>
    {#if view.version && !view.version.valid}
      <div class="field-hint">{t('detail.versionInvalid', { reason: view.version.invalidReason ?? '?' })}</div>
    {:else if view.version}
      <div class="field-hint">
        {t('detail.shell', { version: view.version.shellVersion })} ·
        {t('detail.kernel', { version: view.version.kernelVersion })}
      </div>
    {/if}
  </div>

  <div class="field">
    <div class="field-label">
      <span>{t('detail.home')}</span>
    </div>
    <div class="inline">
      <span class="mono" title={inst.home}>{inst.home}</span>
      <button class="btn tiny ghost" onclick={() => onOpenPath(inst.home, true)}>{t('detail.homeOpen')}</button>
      <button class="btn tiny ghost" onclick={() => onCopy(inst.home)}>{t('detail.homeCopy')}</button>
      <button class="btn tiny ghost" title={logsDir} onclick={() => onOpenPath(logsDir, false)}>
        {t('detail.openLogs')}
      </button>
    </div>
    {#if !view.homeExists}
      <div class="field-hint">{t('detail.homeMissing')}</div>
    {/if}
  </div>

  <div class="field">
    <div class="field-label">
      <span>{t('detail.status')}</span>
    </div>
    <div class="status-line">
      <span class:running class:failed={view.launch.phase === 'failed'}>{statusText}</span>
      {#if view.running}
        <span class="field-hint">{t('detail.pid', { pid: view.running.shellPid })}</span>
        {#if view.running.hostPid}
          <span class="field-hint">{t('detail.hostPid', { pid: view.running.hostPid })}</span>
        {/if}
      {/if}
    </div>
    {#if view.launch.error}
      <div class="error-text" title={view.launch.error.detail ?? ''}>
        {view.launch.error.code} · {view.launch.error.message}
      </div>
    {/if}
    <div class="field-hint">
      {inst.lastExitClean ? t('detail.exitClean') : t('detail.exitDirty')}
      {#if inst.lastLaunchAt}· {hhmm(inst.lastLaunchAt)} · {formatDate(inst.lastLaunchAt)}{/if}
    </div>
  </div>

  <div class="field">
    <div class="field-label"><span>{t('detail.notes')}</span></div>
    <textarea class="input" bind:value={notes} placeholder={t('detail.notesPlaceholder')} disabled={saving}
    ></textarea>
  </div>

  {#if error}
    <div class="error-text">{error}</div>
  {/if}

  <!-- 操作条固定在详情面板底部：窗口再矮也点得到「保存 / 启动」 -->
  <div class="details-actions">
    <button class="btn primary" onclick={doSave} disabled={saving || !dirty}>{t('detail.save')}</button>
    <button
      class="btn ghost tiny"
      style="anchor-name: --restart-detail"
      popovertarget="restart-detail"
      >{t('restart.label')}<Icon name="chevron-down" width={1.5} /></button
    >
    <div
      id="restart-detail"
      popover
      class="dropdown-menu"
      style="position-anchor: --restart-detail"
    >
      <button
        onclick={() => {
          document.getElementById('restart-detail')?.hidePopover();
          onRestart(inst.id, 'normal');
        }}>{t('restart.normal')}</button
      >
      {#if inSafeMode}
        <button
          onclick={() => {
            document.getElementById('restart-detail')?.hidePopover();
            onRestart(inst.id, 'exitSafe');
          }}>{t('restart.exitSafe')}</button
        >
      {:else}
        <button
          onclick={() => {
            document.getElementById('restart-detail')?.hidePopover();
            onRestart(inst.id, 'safe');
          }}>{t('restart.safe')}</button
        >
      {/if}
    </div>
    {#if running}
      <button class="btn ghost" onclick={() => onStop(inst.id)}>{t('list.stop')}</button>
    {:else if pending}
      <button class="btn ghost" onclick={() => onLaunch(inst.id)} disabled>{t('state.waiting')}</button>
    {:else}
      <button class="btn ghost" onclick={() => onLaunch(inst.id)}>{t('list.launch')}</button>
    {/if}
    <span class="spacer"></span>
    <button class="btn tiny danger" onclick={() => onDelete(inst.id)} disabled={running || pending}>
      {t('detail.delete')}
    </button>
  </div>
</aside>

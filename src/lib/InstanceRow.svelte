<script lang="ts">
  import type { InstanceView } from './types';
  import { shortPath, hhmm, uptime, elapsedSec } from './format';
  import Icon from './Icon.svelte';

  interface Props {
    view: InstanceView;
    t: (key: string, vars?: Record<string, string | number>) => string;
    selected: boolean;
    now: number;
    onSelect: (id: string) => void;
    onLaunch: (id: string) => void;
    onStop: (id: string) => void;
    onRestart: (id: string, mode: 'normal' | 'safe' | 'exitSafe') => void;
    onCancel: (id: string) => void;
    onArchive: (id: string, archived: boolean) => void;
    onDelete: (id: string) => void;
  }

  let { view, t, selected, now, onSelect, onLaunch, onStop, onRestart, onCancel, onArchive, onDelete }: Props = $props();

  const inst = $derived(view.instance);
  const running = $derived(Boolean(view.running));
  const launch = $derived(view.launch);
  const phase = $derived(running ? 'running' : launch.phase);
  const pending = $derived(phase === 'queued' || phase === 'preparing' || phase === 'spawning' || phase === 'waiting');
  const failed = $derived(phase === 'failed');

  // now 变化 → 重新计算启动秒数与运行时长
  const seconds = $derived.by(() => {
    void now;
    return elapsedSec(launch.startedAt);
  });
  const uptimeText = $derived.by(() => {
    void now;
    return view.running ? uptime(view.running.startedAt) : '';
  });

  const versionLine = $derived.by(() => {
    const v = view.version;
    if (!v || !v.valid) return t('detail.versionInvalid', { reason: v?.invalidReason ?? '?' });
    return `DSH-NEXT ${v.shellVersion} · ${t('detail.kernel', { version: v.kernelVersion })}`;
  });

  const stateText = $derived.by(() => {
    if (phase === 'queued') return t('list.queued', { position: launch.position ?? 0 });
    if (phase === 'waiting' || phase === 'spawning') return t('list.elapsed', { sec: seconds });
    if (phase === 'preparing') return t('state.preparing');
    if (phase === 'running') return `${t('list.running')} · ${uptimeText}`;
    if (phase === 'failed') return t('list.failed');
    return t('list.stopped');
  });

  const dotClass = $derived(pending ? 'pending' : failed ? 'failed' : running ? 'running' : '');
  /** 当前是否安全模式：运行中用进程扫描，停止后用上次启动记录 */
  const inSafeMode = $derived(view.running?.safeMode ?? inst.safeMode);

  function copyPath(event: MouseEvent) {
    event.stopPropagation();
    navigator.clipboard?.writeText(inst.home);
  }
</script>

<div
  class="row"
  class:active={selected}
  role="button"
  tabindex="0"
  onclick={() => onSelect(inst.id)}
  onkeydown={(e) => {
    if (e.key === 'Enter' || e.key === ' ') onSelect(inst.id);
  }}
>
  <div class="row-line1">
    <span class="status-dot {dotClass}" aria-hidden="true"></span>
    <span class="row-name">{inst.name}</span>
    <span class="row-id">- {inst.id}</span>
    <span class="row-path" title={inst.home + '\n' + t('list.copyPath')} role="presentation" onclick={copyPath}>
      {shortPath(inst.home, 34)}
    </span>
    {#if view.isDefault}
      <span class="badge default">{t('list.default')}</span>
    {/if}
    <div class="row-actions">
      <button
        class="btn tiny ghost"
        style="anchor-name: --restart-{inst.id}"
        popovertarget="restart-{inst.id}"
        onclick={(e) => e.stopPropagation()}
        >{t('restart.label')}<Icon name="chevron-down" width={1.5} /></button
      >
      <div
        id="restart-{inst.id}"
        popover
        class="dropdown-menu"
        style="position-anchor: --restart-{inst.id}"
      >
        <button
          onclick={() => {
            document.getElementById('restart-' + inst.id)?.hidePopover();
            onRestart(inst.id, 'normal');
          }}>{t('restart.normal')}</button
        >
        {#if inSafeMode}
          <button
            onclick={() => {
              document.getElementById('restart-' + inst.id)?.hidePopover();
              onRestart(inst.id, 'exitSafe');
            }}>{t('restart.exitSafe')}</button
          >
        {:else}
          <button
            onclick={() => {
              document.getElementById('restart-' + inst.id)?.hidePopover();
              onRestart(inst.id, 'safe');
            }}>{t('restart.safe')}</button
          >
        {/if}
      </div>
      {#if pending}
        <button class="btn tiny ghost" onclick={(e) => { e.stopPropagation(); onCancel(inst.id); }}>
          {t('list.cancel')}
        </button>
      {:else if running}
        <button class="btn tiny" onclick={(e) => { e.stopPropagation(); onStop(inst.id); }}>
          {t('list.stop')}
        </button>
      {:else}
        <button class="btn tiny primary" onclick={(e) => { e.stopPropagation(); onLaunch(inst.id); }}>
          {failed ? t('list.launch') : t('list.launch')}
        </button>
      {/if}
      <button
        class="btn tiny ghost"
        disabled={running || pending}
        onclick={(e) => { e.stopPropagation(); onArchive(inst.id, !inst.archived); }}
      >
        {inst.archived ? t('list.restore') : t('list.archive')}
      </button>
      <button
        class="btn tiny danger"
        disabled={running || pending}
        onclick={(e) => { e.stopPropagation(); onDelete(inst.id); }}
      >
        {t('list.delete')}
      </button>
    </div>
  </div>
  <div class="row-line2">
    <span>{versionLine}</span>
    <span class="sep">·</span>
    <span>{stateText}</span>
    {#if failed && launch.error}
      <span class="sep">·</span>
      <span class="row-failed" title={`${launch.error.code} ${launch.error.detail ?? ''}`}>
        {launch.error.code}
      </span>
    {:else if !running && inst.lastLaunchAt}
      <span class="sep">·</span>
      <span>{t('list.lastLaunch', { time: hhmm(inst.lastLaunchAt) })}</span>
    {/if}
  </div>
</div>

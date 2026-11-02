<script lang="ts">
  import Modal from './Modal.svelte';
  import type { Instance, VersionInfo } from './types';
  import { untrack } from 'svelte';
  import { errorText, type UiError } from './api';
  import { slugify } from './slug';
  import SelectMenu from './SelectMenu.svelte';

  interface Props {
    mode: 'new' | 'edit';
    instance?: Instance | null;
    versions: VersionInfo[];
    userProfile: string;
    /** 数据目录树根：新建实例 home 的默认值前缀 */
    homeRoot: string;
    defaultHome: string;
    initialId?: string;
    initialHome?: string;
    t: (key: string, vars?: Record<string, string | number>) => string;
    onClose: () => void;
    onSubmit: (payload: {
      id: string;
      name: string;
      version: string;
      home: string;
      notes: string;
      isNew: boolean;
      archived: boolean;
    }) => Promise<void>;
  }

  let { mode, instance, versions, userProfile, homeRoot, t, onClose, onSubmit, initialId, initialHome }: Props = $props();

  const validVersions = $derived(versions.filter((v) => v.valid));

  // 根目录只决定“默认值”：空串回落到 %USERPROFILE%，去掉结尾分隔符
  const rootDir = $derived((homeRoot?.trim() || userProfile || '').replace(/[\\/]+$/, ''));

  /** 默认 home：<root>\.dsh-<id>（default 实例用 <root>\.dsh） */
  function defaultHomeFor(idValue: string): string {
    if (!idValue || idValue === 'default') return `${rootDir}\\.dsh`;
    return `${rootDir}\\.dsh-${idValue}`;
  }

  let name = $state(untrack(() => instance?.name ?? ''));
  let id = $state(untrack(() => instance?.id ?? initialId ?? ''));
  let idTouched = $state(untrack(() => mode === 'edit'));
  let version = $state(untrack(() => instance?.version ?? validVersions[0]?.tag ?? ''));
  let home = $state(untrack(() => instance?.home ?? initialHome ?? ''));
  let notes = $state(untrack(() => instance?.notes ?? ''));
  let homeTouched = $state(untrack(() => mode === 'edit' || Boolean(initialHome)));
  let error = $state<string | null>(null);
  let submitting = $state(false);

  function autoId() {
    if (idTouched) return;
    const base = slugify(name);
    if (id !== base) id = base;
    if (!homeTouched) {
      const next = base ? defaultHomeFor(base) : '';
      if (home !== next) home = next;
    }
  }

  $effect(() => {
    // name 变化 → 自动转写 id 与 home（用户手动改过就不再覆盖）
    if (mode === 'new') {
      autoId();
    }
  });

  async function submit() {
    error = null;
    if (!name.trim()) {
      error = t('dialog.name') + ' ✕';
      return;
    }
    if (mode === 'new' && !/^[a-z0-9-]{1,32}$/.test(id)) {
      error = t('dialog.id') + ' ✕';
      return;
    }
    if (!version) {
      error = t('dialog.versionEmpty');
      return;
    }
    submitting = true;
    try {
      await onSubmit({
        id: mode === 'new' ? id : (instance?.id ?? id),
        name: name.trim(),
        version,
        home: home.trim() || `${userProfile}\\.dsh-${slugify(name)}`,
        notes,
        isNew: mode === 'new',
        archived: instance?.archived ?? false
      });
    } catch (err) {
      error = errorText(err as UiError);
    } finally {
      submitting = false;
    }
  }
</script>

<Modal
  title={mode === 'new' ? t('dialog.new.title') : t('dialog.edit.title')}
  {onClose}
>
  <div class="field">
    <div class="field-label"><span>{t('dialog.name')}</span></div>
    <input
      class="input"
      bind:value={name}
      placeholder={t('dialog.namePlaceholder')}
      maxlength="40"
      oninput={() => mode === 'new' && autoId()}
    />
  </div>

  <div class="field">
    <div class="field-label">
      <span>{t('dialog.id')}</span>
      <span class="field-hint">{mode === 'new' ? t('dialog.idAuto') : t('dialog.idLocked')}</span>
    </div>
    <input
      class="input"
      bind:value={id}
      readonly={mode === 'edit'}
      list="none"
      oninput={() => {
        idTouched = true;
        if (!homeTouched) home = defaultHomeFor(id);
      }}
    />
    {#if mode === 'new' && !idTouched && id.startsWith('inst-')}
      <span class="field-hint">{t('dialog.idHashHint')}</span>
    {/if}
  </div>

  <div class="field">
    <div class="field-label"><span>{t('dialog.version')}</span></div>
    <SelectMenu
      value={version}
      label={t('dialog.version')}
      options={[
        ...(validVersions.length === 0 ? [{ value: '', label: t('dialog.versionEmpty') }] : []),
        ...validVersions.map((v) => ({
          value: v.tag,
          label: `${v.tag} · ${t('versions.kernel', { kernel: v.kernelVersion })}`,
        })),
        ...(instance && !validVersions.some((v) => v.tag === instance?.version)
          ? [{ value: instance.version, label: `${instance.version}（缺失）` }]
          : []),
      ]}
      onchange={(next) => (version = next)}
    />
  </div>

  <div class="field">
    <div class="field-label">
      <span>{t('dialog.home')}</span>
      <button
        type="button"
        class="link-btn"
        onclick={() => {
          homeTouched = false;
          home = defaultHomeFor(id || slugify(name) || 'instance');
        }}>{t('dialog.homeDefault')}</button
      >
    </div>
    <input
      class="input"
      bind:value={home}
      placeholder={t('dialog.homePlaceholder')}
      oninput={() => (homeTouched = true)}
    />
    <span class="field-hint">{t('dialog.homeHint', { root: rootDir })}</span>
  </div>

  <div class="field">
    <div class="field-label"><span>{t('dialog.notes')}</span></div>
    <textarea class="input" bind:value={notes} placeholder={t('detail.notesPlaceholder')}></textarea>
  </div>

  {#if error}
    <div class="error-text">{error}</div>
  {/if}

  <div class="modal-foot">
    <button class="btn ghost" onclick={onClose} disabled={submitting}>{t('dialog.cancel')}</button>
    <button class="btn primary" onclick={submit} disabled={submitting}>
      {mode === 'new' ? t('dialog.create') : t('dialog.save')}
    </button>
  </div>
</Modal>

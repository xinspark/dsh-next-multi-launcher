import { api, errorText, type UiError } from './api';
import type { CatalogEntry, UpdateInfo } from './types';

/**
 * 在线状态（目录 + 更新检查）的唯一来源。
 *
 * 规则：**启动时拉一次，之后每 10 分钟自动拉一次**；此外只有用户点「刷新」才拉。
 * 放在模块级 `$state` 里，切视图不会丢、也不会因为组件重建而重复请求
 * （否则每切一次视图就打一遍 GitHub，60 次/小时/IP 很容易被限流）。
 */
export type CatalogChannel = 'next' | 'next-preview' | 'native';

interface OnlineState {
  /** 正在拉取的渠道（null = 空闲） */
  loadingChannel: CatalogChannel | null;
  loadingPair: boolean;
  catalogs: Record<string, CatalogEntry[]>;
  errors: Record<string, string>;
  /** DSH-NEXT 两条渠道上次成功拉取的时间 */
  loadedAt: string | null;
  update: UpdateInfo | null;
  updateError: string | null;
  /** 「关于更新」上次检查的时间 */
  updateCheckedAt: string | null;
  /** 最近一次整体刷新的时间 */
  refreshedAt: number | null;
}

export const online = $state<OnlineState>({
  loadingChannel: null,
  loadingPair: false,
  catalogs: {},
  errors: {},
  loadedAt: null,
  update: null,
  updateError: null,
  updateCheckedAt: null,
  refreshedAt: null
});

export async function loadCatalog(ch: CatalogChannel, force = false): Promise<void> {
  if (!force && online.catalogs[ch]) return;
  if (online.loadingChannel === ch) return;
  online.loadingChannel = ch;
  online.errors = { ...online.errors, [ch]: '' };
  try {
    const list = await api.catalogList(ch);
    online.catalogs = { ...online.catalogs, [ch]: list };
    if (ch !== 'native') online.loadedAt = new Date().toLocaleTimeString();
  } catch (err) {
    online.errors = { ...online.errors, [ch]: errorText(err as UiError) };
  } finally {
    if (online.loadingChannel === ch) online.loadingChannel = null;
  }
}

/** 两个 DSH-NEXT 渠道一次拉齐（界面切换 tab 不再等网络） */
export async function loadNextPair(force = false): Promise<void> {
  if (online.loadingPair) return;
  if (!force && online.catalogs.next && online.catalogs['next-preview']) return;
  online.loadingPair = true;
  await Promise.all([loadCatalog('next', force), loadCatalog('next-preview', force)]);
  online.loadingPair = false;
}

export async function checkUpdate(force = false): Promise<void> {
  if (!force && online.update) return;
  online.updateError = null;
  try {
    online.update = await api.updateCheck();
    online.updateCheckedAt = new Date().toLocaleTimeString();
  } catch (err) {
    online.updateError = errorText(err as UiError);
  }
}

/** 版本目录 + 更新检查一起拉（启动时一次，之后每 10 分钟一次） */
export async function refreshAll(force = false): Promise<void> {
  await Promise.all([loadNextPair(force), loadCatalog('native', force), checkUpdate(force)]);
  online.refreshedAt = Date.now();
}

/** 手动「刷新」：当前在原生渠道只拉它，否则拉 NEXT 两条 */
export async function refreshCurrent(ch: CatalogChannel): Promise<void> {
  if (ch === 'native') {
    await loadCatalog('native', true);
    await checkUpdate();
  } else {
    await Promise.all([loadNextPair(true), checkUpdate()]);
  }
  online.refreshedAt = Date.now();
}

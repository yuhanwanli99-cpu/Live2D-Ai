/**
 * toast.ts — 全局轻提示（断线 / LLM 失败 / 保存结果 / 外部接入事件）
 *
 * 背景：此前断线只在聊天面板顶部显示一行小字，LLM/TTS 失败在主画面完全没有
 * 用户可见提示（见 docs/research/neko-ui-alignment-and-gaps.md P1-6）。
 *
 * 设计：
 *   - 单一 host 容器（懒创建，挂 document.body），同类消息去重（避免重连风暴刷屏）；
 *   - `sticky` 消息不自动消失（用于"连接断开中"这类持续状态），恢复时用同 key 覆盖；
 *   - 纯 DOM，无框架依赖；node/vitest 环境（无 document）自动降级为 no-op。
 */
import { icon, type IconName } from './icons';

export type ToastKind = 'info' | 'ok' | 'warn' | 'error';

export interface ToastOptions {
  /** 同 key 的提示互相覆盖（用于状态类提示，如连接状态） */
  key?: string;
  /** 展示时长 ms；0 或 sticky=true 表示不自动消失 */
  durationMs?: number;
  sticky?: boolean;
}

const ICON_BY_KIND: Record<ToastKind, IconName> = {
  info: 'info',
  ok: 'check',
  warn: 'alert',
  error: 'alert',
};

const DEFAULT_DURATION_MS = 3600;

interface LiveToast {
  el: HTMLElement;
  timer: ReturnType<typeof setTimeout> | null;
}

let host: HTMLElement | null = null;
const byKey = new Map<string, LiveToast>();

function ensureHost(): HTMLElement | null {
  if (typeof document === 'undefined') return null;
  if (host && host.isConnected) return host;
  host = document.createElement('div');
  host.className = 'toast-host';
  host.setAttribute('role', 'status');
  host.setAttribute('aria-live', 'polite');
  document.body.appendChild(host);
  return host;
}

function dismiss(entry: LiveToast, key?: string): void {
  if (entry.timer) clearTimeout(entry.timer);
  entry.el.classList.add('leaving');
  const remove = (): void => {
    entry.el.remove();
    if (key && byKey.get(key) === entry) byKey.delete(key);
  };
  // 动画结束再移除；拿不到 animationend（reduced-motion / 测试环境）则兜底定时移除
  entry.el.addEventListener('animationend', remove, { once: true });
  setTimeout(remove, 400);
}

/**
 * 弹出一条提示。
 * @returns 手动关闭函数（sticky 提示由调用方负责收尾）
 */
export function toast(
  message: string,
  kind: ToastKind = 'info',
  opts: ToastOptions = {},
): () => void {
  const container = ensureHost();
  if (!container) return () => undefined;

  const { key, sticky = false } = opts;
  const durationMs = opts.durationMs ?? (sticky ? 0 : DEFAULT_DURATION_MS);

  if (key) {
    const existing = byKey.get(key);
    if (existing) dismiss(existing, key);
  }

  const el = document.createElement('div');
  el.className = `toast toast-${kind}`;
  el.innerHTML = `${icon(ICON_BY_KIND[kind])}<span></span>`;
  const textEl = el.querySelector('span');
  if (textEl) textEl.textContent = message;
  container.appendChild(el);

  const entry: LiveToast = { el, timer: null };
  if (key) byKey.set(key, entry);
  if (durationMs > 0) {
    entry.timer = setTimeout(() => dismiss(entry, key), durationMs);
  }
  // 点击即关（长提示不挡视线）
  el.addEventListener('click', () => dismiss(entry, key));
  return () => dismiss(entry, key);
}

/** 关闭指定 key 的提示（状态恢复时调用）。 */
export function clearToast(key: string): void {
  const existing = byKey.get(key);
  if (existing) dismiss(existing, key);
}

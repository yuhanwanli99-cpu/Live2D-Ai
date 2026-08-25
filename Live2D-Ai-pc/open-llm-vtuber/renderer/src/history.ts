/**
 * history.ts — 聊天历史回看（P0-3）
 *
 * 后端早就有持久化基础设施（`chat_history_manager.py` 写
 * `chat_history/<conf_uid>/<history_uid>.json`），但此前**没人绑定 history_uid**，
 * 所以一条都没落过盘；新 renderer 也从不请求历史。
 * 本模块补齐前端一侧：连接即恢复上文、抽屉里回看/切换/新建/删除会话。
 *
 * WS 协议（既有，未新增后端消息类型）：
 *   上行  fetch-history-list / fetch-and-set-history{history_uid} /
 *         create-new-history / delete-history{history_uid}
 *   下行  history-list{histories} / history-data{history_uid,messages} /
 *         new-history-created{history_uid} / history-deleted{success,history_uid}
 *
 * 纯逻辑（mapHistoryMessages / historyEntryTitle / formatHistoryTime / sortHistories）
 * 与 DOM 分离，便于 vitest 单测。
 */
import { icon } from './icons';
import type { ChatMessage } from './chat-ui';

/* ------------------------------------------------------------------ *
 * 类型（与后端 chat_history_manager 产出同构）
 * ------------------------------------------------------------------ */

/** 历史消息条目：后端角色是 human / ai（不是 user / assistant） */
export interface HistoryMessage {
  role: string;
  content: string;
  timestamp?: string;
  name?: string;
  avatar?: string | null;
}

export interface HistoryEntry {
  uid: string;
  latest_message?: HistoryMessage | null;
  timestamp?: string | null;
}

/* ------------------------------------------------------------------ *
 * 纯逻辑
 * ------------------------------------------------------------------ */

/**
 * 后端历史消息 → 聊天面板消息。
 * `human → user`、`ai → assistant`；其它角色（system/metadata 等）一律丢弃。
 */
export function mapHistoryMessages(messages: HistoryMessage[] | undefined): ChatMessage[] {
  if (!Array.isArray(messages)) return [];
  const out: ChatMessage[] = [];
  for (const msg of messages) {
    if (!msg || typeof msg.content !== 'string' || !msg.content.trim()) continue;
    if (msg.role === 'human') out.push({ role: 'user', content: msg.content });
    else if (msg.role === 'ai') out.push({ role: 'assistant', content: msg.content });
  }
  return out;
}

/** 会话标题：取最后一条消息的前 24 字（空会话给占位文案）。 */
export function historyEntryTitle(entry: HistoryEntry): string {
  const content = entry?.latest_message?.content?.trim() || '';
  if (!content) return '（空会话）';
  const oneLine = content.replace(/\s+/g, ' ');
  return oneLine.length > 24 ? `${oneLine.slice(0, 24)}…` : oneLine;
}

/**
 * 时间显示：今天 → `HH:MM`；昨天 → `昨天 HH:MM`；更早 → `M月D日`。
 * @param now 注入"当前时间"便于测试（缺省取系统时间）
 */
export function formatHistoryTime(timestamp?: string | null, now: Date = new Date()): string {
  if (!timestamp) return '';
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return '';
  const hhmm = `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
  const sameDay = (a: Date, b: Date): boolean =>
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate();
  if (sameDay(date, now)) return hhmm;
  const yesterday = new Date(now.getTime() - 86400000);
  if (sameDay(date, yesterday)) return `昨天 ${hhmm}`;
  return `${date.getMonth() + 1}月${date.getDate()}日`;
}

/** 按时间倒序（新会话在前）；缺时间戳的排最后。 */
export function sortHistories(histories: HistoryEntry[] | undefined): HistoryEntry[] {
  if (!Array.isArray(histories)) return [];
  return [...histories].sort((a, b) => (b.timestamp || '').localeCompare(a.timestamp || ''));
}

/* ------------------------------------------------------------------ *
 * 历史抽屉 UI
 * ------------------------------------------------------------------ */

export interface HistoryDrawerCallbacks {
  /** 请求历史列表（上行 fetch-history-list） */
  onRefresh(): void;
  /** 切换到某段历史（上行 fetch-and-set-history） */
  onSelect(uid: string): void;
  /** 开新会话（上行 create-new-history） */
  onCreate(): void;
  /** 删除某段历史（上行 delete-history） */
  onDelete(uid: string): void;
}

export interface HistoryDrawer {
  open(): void;
  close(): void;
  toggle(): void;
  isOpen(): boolean;
  /** 收到 history-list 后渲染 */
  setHistories(histories: HistoryEntry[]): void;
  /** 标记当前会话（高亮） */
  setCurrent(uid: string | null): void;
  destroy(): void;
}

function escapeHtml(text: string): string {
  return String(text ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/**
 * 在聊天面板内挂一个历史抽屉（覆盖消息区，不改变面板尺寸）。
 * @param host 聊天面板根元素（`.chat-panel`）
 */
export function initHistoryDrawer(
  host: HTMLElement,
  cb: HistoryDrawerCallbacks,
): HistoryDrawer {
  const drawer = document.createElement('div');
  drawer.className = 'chat-history-drawer';
  drawer.setAttribute('aria-label', '聊天历史');
  drawer.setAttribute('aria-hidden', 'true');
  drawer.innerHTML = `
    <div class="chat-history-head">
      <span class="chat-history-title">聊天历史</span>
      <div class="chat-history-actions">
        <button class="chat-icon-btn chat-history-new ui-lift" type="button" title="开始新会话" aria-label="开始新会话">${icon('sparkle')}</button>
        <button class="chat-icon-btn chat-history-close ui-lift" type="button" title="关闭" aria-label="关闭聊天历史">${icon('close')}</button>
      </div>
    </div>
    <div class="chat-history-list" data-historylist></div>
  `;
  host.appendChild(drawer);

  const listEl = drawer.querySelector<HTMLElement>('[data-historylist]')!;
  let currentUid: string | null = null;
  let entries: HistoryEntry[] = [];

  const render = (): void => {
    listEl.innerHTML = '';
    if (entries.length === 0) {
      const empty = document.createElement('div');
      empty.className = 'chat-history-empty';
      empty.textContent = '还没有历史会话——聊几句就会自动保存在本机。';
      listEl.appendChild(empty);
      return;
    }
    for (const entry of entries) {
      const row = document.createElement('div');
      row.className = `chat-history-item${entry.uid === currentUid ? ' current' : ''}`;
      row.innerHTML = `
        <button class="chat-history-pick" type="button">
          <span class="chat-history-item-title">${escapeHtml(historyEntryTitle(entry))}</span>
          <span class="chat-history-item-time">${escapeHtml(formatHistoryTime(entry.timestamp))}</span>
        </button>
        <button class="chat-history-del" type="button" title="删除这段会话" aria-label="删除这段会话">${icon('trash')}</button>
      `;
      row.querySelector<HTMLButtonElement>('.chat-history-pick')!.addEventListener('click', () => {
        if (entry.uid === currentUid) {
          close();
          return;
        }
        cb.onSelect(entry.uid);
        close();
      });
      row.querySelector<HTMLButtonElement>('.chat-history-del')!.addEventListener('click', (e) => {
        e.stopPropagation();
        cb.onDelete(entry.uid);
      });
      listEl.appendChild(row);
    }
  };

  const open = (): void => {
    drawer.classList.add('open');
    drawer.setAttribute('aria-hidden', 'false');
    cb.onRefresh(); // 每次打开都拉一次，避免看到过期列表
  };
  const close = (): void => {
    drawer.classList.remove('open');
    drawer.setAttribute('aria-hidden', 'true');
  };

  drawer.querySelector<HTMLButtonElement>('.chat-history-close')!.addEventListener('click', close);
  drawer.querySelector<HTMLButtonElement>('.chat-history-new')!.addEventListener('click', () => {
    cb.onCreate();
    close();
  });

  render();

  return {
    open,
    close,
    toggle() {
      if (drawer.classList.contains('open')) close();
      else open();
    },
    isOpen: () => drawer.classList.contains('open'),
    setHistories(histories: HistoryEntry[]) {
      entries = sortHistories(histories);
      render();
    },
    setCurrent(uid: string | null) {
      currentUid = uid;
      render();
    },
    destroy() {
      drawer.remove();
    },
  };
}

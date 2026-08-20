/**
 * history.test.ts — 聊天历史回看纯逻辑单测（P0-3）
 *
 * 被测：renderer/src/history.ts。重点锁定与后端 `chat_history_manager.py` 的契约：
 * 后端角色是 `human` / `ai`（**不是** user/assistant），metadata/system 必须被丢弃。
 */
import { describe, expect, it } from 'vitest';

import {
  formatHistoryTime,
  historyEntryTitle,
  mapHistoryMessages,
  sortHistories,
  type HistoryEntry,
} from '../src/history';

describe('mapHistoryMessages —— human/ai → user/assistant', () => {
  it('按后端角色正确映射', () => {
    const mapped = mapHistoryMessages([
      { role: 'human', content: '你好' },
      { role: 'ai', content: '你好呀' },
    ]);
    expect(mapped).toEqual([
      { role: 'user', content: '你好' },
      { role: 'assistant', content: '你好呀' },
    ]);
  });

  it('丢弃 metadata / system / 未知角色（否则面板会冒出奇怪气泡）', () => {
    const mapped = mapHistoryMessages([
      { role: 'metadata', content: '2026-08-20T00:00:00' },
      { role: 'system', content: '你是白' },
      { role: 'tool', content: '{}' },
      { role: 'human', content: '在吗' },
    ]);
    expect(mapped).toEqual([{ role: 'user', content: '在吗' }]);
  });

  it('丢弃空白内容', () => {
    expect(mapHistoryMessages([{ role: 'ai', content: '   ' }])).toEqual([]);
    expect(mapHistoryMessages([{ role: 'ai', content: '' }])).toEqual([]);
  });

  it('非数组/undefined 安全返回空', () => {
    expect(mapHistoryMessages(undefined)).toEqual([]);
    expect(mapHistoryMessages(null as never)).toEqual([]);
  });

  it('保留原始顺序（历史必须按时间正序回灌）', () => {
    const mapped = mapHistoryMessages([
      { role: 'human', content: '1' },
      { role: 'ai', content: '2' },
      { role: 'human', content: '3' },
    ]);
    expect(mapped.map((m) => m.content)).toEqual(['1', '2', '3']);
  });
});

describe('historyEntryTitle', () => {
  it('取最后一条消息作标题', () => {
    expect(historyEntryTitle({ uid: 'a', latest_message: { role: 'ai', content: '晚安' } })).toBe('晚安');
  });

  it('超长标题截断到 24 字 + 省略号', () => {
    const long = '一'.repeat(50);
    const title = historyEntryTitle({ uid: 'a', latest_message: { role: 'human', content: long } });
    expect(title.length).toBeLessThanOrEqual(25);
    expect(title.endsWith('…')).toBe(true);
  });

  it('多行折叠为单行', () => {
    const title = historyEntryTitle({
      uid: 'a',
      latest_message: { role: 'human', content: '第一行\n第二行' },
    });
    expect(title).toBe('第一行 第二行');
  });

  it('空会话给占位文案而不是空白', () => {
    expect(historyEntryTitle({ uid: 'a' })).toBe('（空会话）');
    expect(historyEntryTitle({ uid: 'a', latest_message: null })).toBe('（空会话）');
  });
});

describe('formatHistoryTime', () => {
  const now = new Date('2026-08-20T15:30:00');

  it('当天 → HH:MM', () => {
    expect(formatHistoryTime('2026-08-20T09:05:00', now)).toBe('09:05');
  });

  it('昨天 → 昨天 HH:MM', () => {
    expect(formatHistoryTime('2026-08-19T22:41:00', now)).toBe('昨天 22:41');
  });

  it('更早 → M月D日', () => {
    expect(formatHistoryTime('2026-08-02T10:00:00', now)).toBe('8月2日');
  });

  it('缺失/非法时间戳 → 空串（不显示 Invalid Date）', () => {
    expect(formatHistoryTime(null, now)).toBe('');
    expect(formatHistoryTime(undefined, now)).toBe('');
    expect(formatHistoryTime('not-a-date', now)).toBe('');
  });
});

describe('sortHistories', () => {
  it('按时间倒序（最新会话在最前）', () => {
    const entries: HistoryEntry[] = [
      { uid: 'old', timestamp: '2026-08-01T10:00:00' },
      { uid: 'new', timestamp: '2026-08-20T10:00:00' },
      { uid: 'mid', timestamp: '2026-08-10T10:00:00' },
    ];
    expect(sortHistories(entries).map((e) => e.uid)).toEqual(['new', 'mid', 'old']);
  });

  it('缺时间戳的排最后，且不修改入参', () => {
    const entries: HistoryEntry[] = [
      { uid: 'none' },
      { uid: 'dated', timestamp: '2026-08-20T10:00:00' },
    ];
    const sorted = sortHistories(entries);
    expect(sorted.map((e) => e.uid)).toEqual(['dated', 'none']);
    expect(entries[0].uid).toBe('none'); // 原数组未被就地排序
  });

  it('非数组安全返回空', () => {
    expect(sortHistories(undefined)).toEqual([]);
  });
});

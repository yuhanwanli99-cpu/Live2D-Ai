/**
 * background.test.ts — 角色背景图前端纯逻辑单测
 *
 * 被测：renderer/src/background.ts（formatBytes / validateLocalFile /
 * backgroundUrlFor / cssBackgroundValue / applyBackgroundLayer / API 客户端）。
 * 校验口径必须与后端 backgrounds.py 保持一致（扩展名白名单 + 12MB 上限）。
 */
import { describe, expect, it, vi, afterEach } from 'vitest';

import {
  ALLOWED_BACKGROUND_EXTENSIONS,
  MAX_BACKGROUND_BYTES,
  applyBackgroundLayer,
  backgroundUrlFor,
  cssBackgroundValue,
  escapeCssUrl,
  escapeHtml,
  fetchBackgroundState,
  formatBytes,
  selectBackground,
  validateLocalFile,
} from '../src/background';

describe('escapeHtml / escapeCssUrl —— backgrounds/ 里的文件名不一定经过上传接口清洗', () => {
  it('手工拷进去的恶意文件名不会变成可执行 HTML', () => {
    const evil = '<img src=x onerror="alert(1)">.png';
    const escaped = escapeHtml(evil);
    expect(escaped).not.toContain('<img');
    expect(escaped).toContain('&lt;img');
    expect(escaped).toContain('&quot;');
  });

  it('& 与单引号也被转义', () => {
    expect(escapeHtml("a&b'c")).toBe('a&amp;b&#39;c');
  });

  it('空值安全', () => {
    expect(escapeHtml('')).toBe('');
  });

  it('CSS url() 里的引号/括号/空格被转义，无法截断样式', () => {
    const escaped = escapeCssUrl("/bg/a'); background:red; x('.png");
    expect(escaped).not.toMatch(/[^\\]'/);
    expect(escaped).toContain("\\'");
    expect(escapeCssUrl('/bg/my file.png')).toContain('\\ ');
  });
});

describe('formatBytes', () => {
  it('B / KB / MB 三档', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(2048)).toBe('2 KB');
    expect(formatBytes(3 * 1024 * 1024)).toBe('3.0 MB');
  });

  it('非法输入不抛错', () => {
    expect(formatBytes(-1)).toBe('—');
    expect(formatBytes(NaN)).toBe('—');
  });
});

describe('validateLocalFile —— 与后端同一口径，先在前端挡一次', () => {
  it('白名单扩展名全部放行（含大写）', () => {
    for (const ext of ALLOWED_BACKGROUND_EXTENSIONS) {
      expect(validateLocalFile({ name: `bg${ext}`, size: 1024 })).toBeNull();
      expect(validateLocalFile({ name: `BG${ext.toUpperCase()}`, size: 1024 })).toBeNull();
    }
  });

  it('非图片扩展名被拒（含 svg / exe / 无扩展名）', () => {
    expect(validateLocalFile({ name: 'a.svg', size: 10 })).toContain('只支持');
    expect(validateLocalFile({ name: 'a.exe', size: 10 })).toContain('只支持');
    expect(validateLocalFile({ name: 'noext', size: 10 })).toContain('只支持');
  });

  it('空文件被拒', () => {
    expect(validateLocalFile({ name: 'a.png', size: 0 })).toBe('文件内容为空');
  });

  it('超过 12MB 被拒且提示实际大小', () => {
    const error = validateLocalFile({ name: 'a.png', size: MAX_BACKGROUND_BYTES + 1 });
    expect(error).toContain('过大');
    expect(error).toContain('12.0 MB');
  });

  it('恰好等于上限放行（边界）', () => {
    expect(validateLocalFile({ name: 'a.png', size: MAX_BACKGROUND_BYTES })).toBeNull();
  });
});

describe('backgroundUrlFor / cssBackgroundValue', () => {
  it('拼 /bg/ 前缀并对文件名转义（中文、空格安全）', () => {
    expect(backgroundUrlFor('room.png')).toBe('/bg/room.png');
    expect(backgroundUrlFor('我的 房间.png')).toBe(`/bg/${encodeURIComponent('我的 房间.png')}`);
  });

  it('空值 → null（回到透明）', () => {
    expect(backgroundUrlFor(null)).toBeNull();
    expect(backgroundUrlFor('')).toBeNull();
    expect(backgroundUrlFor(undefined)).toBeNull();
  });

  it('CSS 值：有图 url()，无图 none', () => {
    expect(cssBackgroundValue('/bg/a.png')).toBe('url("/bg/a.png")');
    expect(cssBackgroundValue(null)).toBe('none');
  });
});

describe('applyBackgroundLayer —— 直接作用于 #app-background', () => {
  /**
   * 极简元素替身：vitest 环境是 node（无 DOM，见 vitest.config.ts），
   * 而 applyBackgroundLayer 只用到 `style.backgroundImage` 与 `classList.toggle`，
   * 因此用替身即可真实覆盖逻辑，无需为此引入 jsdom 依赖。
   */
  const fakeEl = () => {
    const classes = new Set<string>();
    return {
      style: { backgroundImage: '' },
      classList: {
        toggle(name: string, on?: boolean) {
          const next = on ?? !classes.has(name);
          if (next) classes.add(name);
          else classes.delete(name);
        },
        contains: (name: string) => classes.has(name),
      },
    } as unknown as HTMLElement;
  };

  it('设置背景图并打上 has-image 标记（用于压暗渐变）', () => {
    const el = fakeEl();
    applyBackgroundLayer(el, '/bg/a.png');
    expect(el.style.backgroundImage).toContain('/bg/a.png');
    expect(el.classList.contains('has-image')).toBe(true);
  });

  it('传 null 时清空背景并移除标记', () => {
    const el = fakeEl();
    applyBackgroundLayer(el, '/bg/a.png');
    applyBackgroundLayer(el, null);
    expect(el.style.backgroundImage).toBe('none');
    expect(el.classList.contains('has-image')).toBe(false);
  });

  it('元素缺失（测试/无背景层环境）时安全 no-op', () => {
    expect(() => applyBackgroundLayer(null, '/bg/a.png')).not.toThrow();
  });
});

describe('API 客户端', () => {
  const originalFetch = globalThis.fetch;
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('fetchBackgroundState 解析 files/current/current_url', async () => {
    globalThis.fetch = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({
        ok: true,
        files: [{ name: 'a.png', url: '/bg/a.png', size: 100, mtime: 1 }],
        current: 'a.png',
        current_url: '/bg/a.png',
      }),
    })) as unknown as typeof fetch;

    const state = await fetchBackgroundState();
    expect(state.files).toHaveLength(1);
    expect(state.current).toBe('a.png');
    expect(state.current_url).toBe('/bg/a.png');
  });

  it('缺字段时给出安全缺省（不让 UI 崩）', async () => {
    globalThis.fetch = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({ ok: true }),
    })) as unknown as typeof fetch;

    const state = await fetchBackgroundState();
    expect(state.files).toEqual([]);
    expect(state.current).toBeNull();
  });

  it('selectBackground 把 null 作为"无背景"发出', async () => {
    const spy = vi.fn(async () => ({
      ok: true,
      status: 200,
      json: async () => ({ ok: true, files: [], current: null, current_url: null }),
    }));
    globalThis.fetch = spy as unknown as typeof fetch;

    await selectBackground(null);
    const [, init] = spy.mock.calls[0] as unknown as [string, RequestInit];
    expect(JSON.parse(String(init.body))).toEqual({ file: null });
  });

  it('404（背景不存在）→ 抛后端文案', async () => {
    globalThis.fetch = vi.fn(async () => ({
      ok: false,
      status: 404,
      json: async () => ({ ok: false, error: '背景图不存在: ghost.png' }),
    })) as unknown as typeof fetch;

    await expect(selectBackground('ghost.png')).rejects.toThrow('背景图不存在');
  });
});

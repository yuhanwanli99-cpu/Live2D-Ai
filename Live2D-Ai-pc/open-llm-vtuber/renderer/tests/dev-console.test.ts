/**
 * dev-console.test.ts — 开发者控制台（导演分析 / 动作触发测试台）
 *
 * 用注入的假 api 覆盖 DOM 行为：
 *  - open() 渲染覆盖层并自动拉取动作目录；
 *  - 目录刷新后动作列表渲染（含编排/手势标记）；
 *  - 触发动作按强度 1/2/3 + release 提交正确参数；
 *  - 导演分析成功渲染脱敏 JSON；
 *  - 403/错误在状态行可见（永不静默）。
 *
 * 需要 DOM：通过 @vitest-environment happy-dom 声明。
 */
// @vitest-environment happy-dom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createDevConsole, type DevApi } from '../src/dev-console/dev-console';
import { devActionRegistry } from '../src/dev-console/action-registry';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const CSS_PATH = join(
  dirname(fileURLToPath(import.meta.url)),
  '../src/settings.css',
);
const cssSource = readFileSync(CSS_PATH, 'utf8');
/** 折叠几何契约块：必须以紧凑浮条替换整条右侧占位。 */
const COLLAPSED_BLOCK = cssSource.match(/\.dev-dock\.collapsed\s*{([^}]*)}/);

function jsonResponse(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as Response;
}

function makeFakeApi(overrides: Partial<DevApi> = {}): DevApi & {
  calls: { catalog: number; trigger: Array<{ action: string; strength: number; release: boolean }>; analyze: string[] };
} {
  const calls = { catalog: 0, trigger: [] as Array<{ action: string; strength: number; release: boolean }>, analyze: [] as string[] };
  const api: DevApi = {
    catalog: async () => {
      calls.catalog += 1;
      return jsonResponse(200, {
        actions: ['nod', 'bow', 'look_around'],
        halfbody_channels: ['headX', 'headY'],
        capabilities: {},
      });
    },
    trigger: async (action, opts) => {
      calls.trigger.push({ action, strength: opts.strength ?? 2, release: opts.release ?? false });
      return jsonResponse(200, { ok: true, action, strength: opts.strength ?? 2 });
    },
    analyze: async (text) => {
      calls.analyze.push(text);
      return jsonResponse(200, { emotion: 'joy', strength: 0.8, motion: 'wave_big', motion_strength: 2, motion_timeline: [] });
    },
    ...overrides,
  };
  return { ...api, calls };
}

describe('createDevConsole — 生命周期与渲染', () => {
  let host: HTMLElement;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
  });

  afterEach(() => {
    host.remove();
    devActionRegistry.loadCatalog([]);
  });

  it('open() 挂载非模态停靠面板并自动拉取动作目录', async () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    // 目录渲染是异步的（fetch → loadCatalog → render），等待动作行出现再断言
    await vi.waitFor(() => expect(host.querySelectorAll('.dev-action-row').length).toBe(3));
    expect(fake.calls.catalog).toBe(1);
    const dock = host.querySelector('.dev-dock');
    expect(dock).not.toBeNull();
    const rows = host.querySelectorAll('.dev-action-row');
    expect(rows.length).toBe(3);
    const kinds = Array.from(host.querySelectorAll('.dev-action-kind')).map((k) => k.textContent);
    // 目录按字典序渲染：nod / bow / look_around 均为半身编排动作（CHOREOGRAPHY_LIBRARY）
    expect(kinds).toEqual(['choreo', 'choreo', 'choreo']);
    console_.close();
  });

  it('close() 移除停靠面板', async () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() => expect(host.querySelectorAll('.dev-action-row').length).toBe(3));
    expect(host.querySelector('.dev-dock')).not.toBeNull();
    console_.close();
    expect(host.querySelector('.dev-dock')).toBeNull();
  });

  it('非模态：菜单不应用全屏遮罩类，点击外部不关闭', async () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() => expect(host.querySelectorAll('.dev-action-row').length).toBe(3));
    const dock = host.querySelector<HTMLElement>('.dev-dock')!;
    // 不继承 settings-overlay（全屏 scrim）语义；aria-modal 必须为 false/absent
    expect(dock.classList.contains('settings-overlay')).toBe(false);
    expect(dock.classList.contains('dev-console-overlay')).toBe(false);
    expect(dock.getAttribute('aria-modal')).not.toBe('true');
    // 点击 dock 外部（body）不应关闭面板（非模态）
    document.body.click();
    expect(host.querySelector('.dev-dock')).not.toBeNull();
    console_.close();
  });

  it('折叠按钮隐藏主体、保留窄条；再次点击恢复', async () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() => expect(host.querySelectorAll('.dev-action-row').length).toBe(3));
    const dock = host.querySelector<HTMLElement>('.dev-dock')!;
    const collapse = dock.querySelector<HTMLButtonElement>('.dev-dock-collapse')!;
    collapse.click();
    expect(dock.classList.contains('collapsed')).toBe(true);
    // 折叠态仍保留 dock 根（窄条），不卸载
    expect(host.querySelector('.dev-dock')).not.toBeNull();
    collapse.click();
    expect(dock.classList.contains('collapsed')).toBe(false);
    console_.close();
  });

  it('折叠态容器不再占据整条右侧：几何 CSS 契约必须取消 top/bottom 全高占位', () => {
    // 根因防止：`.dev-dock` 基础是 fixed + top:0/bottom:0 铺满整条右侧；
    // 若 collapsed 只藏 body 而仍 top/bottom:0，会拦截 Live2D 右侧指针。
    expect(COLLAPSED_BLOCK).not.toBeNull();
    const body = COLLAPSED_BLOCK![1];
    expect(body).toContain('top: auto');
    expect(body).toContain('bottom: auto');
    expect(body).toContain('height: auto');
    expect(body).not.toContain('top: 0');
    // 展开态基础仍保持全高（恢复时顶边对齐）。
    const base = cssSource.match(/^\.dev-dock\s*{([^}]*)}/m);
    expect(base![1]).toMatch(/top:\s*0/);
    expect(base![1]).toMatch(/bottom:\s*0/);
  });

  it('折叠/展开同步 aria-expanded；重复 open 重置为展开且幂等（不重复挂载/监听）', async () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() => expect(host.querySelectorAll('.dev-action-row').length).toBe(3));
    const dock = host.querySelector<HTMLElement>('.dev-dock')!;
    const collapse = dock.querySelector<HTMLButtonElement>('.dev-dock-collapse')!;
    // 打开默认展开
    expect(collapse.getAttribute('aria-expanded')).toBe('true');
    // 折叠 → aria-expanded=false
    collapse.click();
    expect(collapse.getAttribute('aria-expanded')).toBe('false');
    // 重复 open 幂等：不重复 append（仍是同一个 dock 节点），并重置为展开
    console_.open();
    expect(host.querySelectorAll('.dev-dock').length).toBe(1);
    expect(dock.classList.contains('collapsed')).toBe(false);
    expect(collapse.getAttribute('aria-expanded')).toBe('true');
    console_.close();
    expect(host.querySelector('.dev-dock')).toBeNull();
    expect(collapse.getAttribute('aria-expanded')).toBe('true'); // 关闭不改变展开语义
  });
});

describe('createDevConsole — 动作触发', () => {
  let host: HTMLElement;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
  });

  afterEach(() => {
    host.remove();
    devActionRegistry.loadCatalog([]);
  });

  it('点击「触发」按选中强度提交', async () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() =>
      expect(host.querySelector<HTMLElement>('.dev-action-row[data-action="nod"]')).not.toBeNull(),
    );
    const row = host.querySelector<HTMLElement>('.dev-action-row[data-action="nod"]')!;
    const strength = row.querySelector<HTMLSelectElement>('.dev-strength')!;
    strength.value = '3';
    row.querySelector<HTMLButtonElement>('.dev-trigger-btn')!.click();
    await vi.waitFor(() => expect(fake.calls.trigger.length).toBe(1));
    expect(fake.calls.trigger[0]).toEqual({ action: 'nod', strength: 3, release: false });
    await vi.waitFor(() =>
      expect(host.querySelector<HTMLElement>('[data-status]')!.textContent).toContain('已触发 nod'),
    );
    console_.close();
  });

  it('点击「释放」提交 release=true（强度强制 2）', async () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() =>
      expect(host.querySelector<HTMLElement>('.dev-action-row[data-action="bow"]')).not.toBeNull(),
    );
    const row = host.querySelector<HTMLElement>('.dev-action-row[data-action="bow"]')!;
    row.querySelector<HTMLButtonElement>('.dev-release-btn')!.click();
    await vi.waitFor(() => expect(fake.calls.trigger.length).toBe(1));
    expect(fake.calls.trigger[0]).toEqual({ action: 'bow', strength: 2, release: true });
    await vi.waitFor(() =>
      expect(host.querySelector<HTMLElement>('[data-status]')!.textContent).toContain('已释放 bow'),
    );
    console_.close();
  });

  it('未知动作触发失败时错误可见', async () => {
    const fake = makeFakeApi({
      trigger: async () => jsonResponse(400, { error: 'unknown action', allowed: ['nod'] }),
    });
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() =>
      expect(host.querySelector<HTMLElement>('.dev-action-row[data-action="nod"]')).not.toBeNull(),
    );
    const row = host.querySelector<HTMLElement>('.dev-action-row[data-action="nod"]')!;
    row.querySelector<HTMLButtonElement>('.dev-trigger-btn')!.click();
    await vi.waitFor(() => {
      const status = host.querySelector<HTMLElement>('[data-status]')!.textContent ?? '';
      expect(status).toContain('失败: unknown action');
    });
    const cls = host.querySelector<HTMLElement>('[data-status]')!.className;
    expect(cls).toContain('is-off'); // 错误样式
    console_.close();
  });
});

describe('createDevConsole — 导演分析', () => {
  let host: HTMLElement;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
  });

  afterEach(() => {
    host.remove();
  });

  it('分析成功渲染脱敏 JSON；空文本被拒绝', async () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() =>
      expect(host.querySelector<HTMLElement>('.dev-analyze-input')).not.toBeNull(),
    );
    const input = host.querySelector<HTMLTextAreaElement>('.dev-analyze-input')!;
    input.value = '开心地挥手说你好';
    host.querySelector<HTMLButtonElement>('.dev-analyze-btn')!.click();
    // 结果渲染是异步的：等待 JSON 真正显示
    const result = host.querySelector<HTMLElement>('.dev-analyze-result')!;
    await vi.waitFor(() => expect(result.textContent).toContain('"emotion": "joy"'));
    expect(fake.calls.analyze).toEqual(['开心地挥手说你好']);
    expect(result.hidden).toBe(false);
    expect(result.textContent).toContain('"motion": "wave_big"');

    // 空文本：不发请求，状态显示错误
    input.value = '   ';
    host.querySelector<HTMLButtonElement>('.dev-analyze-btn')!.click();
    expect(fake.calls.analyze.length).toBe(1); // 未新增请求
    const status = host.querySelector<HTMLElement>('[data-status]')!.textContent ?? '';
    expect(status).toContain('请输入要分析的文本');
    console_.close();
  });

  it('后端禁用（403 developer mode disabled）错误可见', async () => {
    const fake = makeFakeApi({
      catalog: async () => jsonResponse(403, { error: 'developer mode disabled' }),
    });
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() => {
      const status = host.querySelector<HTMLElement>('[data-status]')!.textContent ?? '';
      expect(status).toContain('失败: developer mode disabled');
    });
    console_.close();
  });
});

describe('createDevConsole — 中文名与最近动作', () => {
  let host: HTMLElement;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
  });

  afterEach(() => {
    host.remove();
    devActionRegistry.loadCatalog([]);
  });

  it('动作行同时显示中文名与技术 ID', async () => {
    const fake = makeFakeApi(); // catalog: nod / bow / look_around
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    await vi.waitFor(() => expect(host.querySelectorAll('.dev-action-row').length).toBe(3));

    const nodRow = host.querySelector<HTMLElement>('.dev-action-row[data-action="nod"]')!;
    expect(nodRow.querySelector<HTMLElement>('.dev-action-cn')!.textContent).toBe('点头');
    expect(nodRow.querySelector<HTMLElement>('.dev-action-id')!.textContent).toBe('nod');
    expect(nodRow.querySelector<HTMLElement>('.dev-action-desc')!.textContent).toBe('轻轻点头示意');

    const bowRow = host.querySelector<HTMLElement>('.dev-action-row[data-action="bow"]')!;
    expect(bowRow.querySelector<HTMLElement>('.dev-action-cn')!.textContent).toBe('鞠躬');

    const aroundRow = host.querySelector<HTMLElement>('.dev-action-row[data-action="look_around"]')!;
    expect(aroundRow.querySelector<HTMLElement>('.dev-action-cn')!.textContent).toBe('左右张望');

    console_.close();
  });

  it('recordAction() 把收到/执行的动作写入「最近动作」并显示强度', () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    console_.recordAction('nod', 3, '对话');
    const recent = host.querySelector<HTMLElement>('[data-recent]')!;
    const text = recent.textContent ?? '';
    expect(text).toContain('点头');
    expect(text).toContain('nod');
    expect(text).toContain('强度 3');
    expect(text).toContain('对话');
    console_.close();
  });

  it('recordAction() 空动作也记录为占位且不抛错', () => {
    const fake = makeFakeApi();
    const console_ = createDevConsole(host, { api: fake });
    console_.open();
    expect(() => console_.recordAction(null, 2)).not.toThrow();
    const recent = host.querySelector<HTMLElement>('[data-recent]')!;
    expect((recent.textContent ?? '').length).toBeGreaterThan(0);
    console_.close();
  });
});
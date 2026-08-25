/**
 * dev-console.ts — 开发者控制台（本机导演分析 + 动作触发测试台）
 *
 * 由 main.ts 在「设置中心 → 系统/高级 → 开发者模式」开启后挂载。
 * 后端 /api/dev/* 仅 localhost + developer_mode 开启时可用；本模块把
 * 403/错误全部显示到状态行，永不静默失败。
 *
 * 依赖注入：createDevConsole(target, { api }) 可注入 api 适配器，便于
 * vitest 纯 DOM 测试（默认实现走 settings/api-client.ts）。
 */

import { getDevCatalog, triggerDevAction, analyzeDevDirector } from '../settings/api-client';
import { devActionRegistry } from './action-registry';
import { icon } from '../icons';

export interface DevCatalog {
  actions: string[];
  halfbody_channels: string[];
  capabilities: Record<string, boolean>;
}

export interface DirectorResult {
  emotion?: string | null;
  strength?: number | null;
  motion?: string | null;
  motion_strength?: number | null;
  motion_timeline?: unknown;
  error?: string;
}

/** 可注入的 API 适配器（测试用假实现；生产用默认实现）。 */
export interface DevApi {
  catalog(): Promise<Response>;
  trigger(action: string, opts: { strength?: 1 | 2 | 3; release?: boolean }): Promise<Response>;
  analyze(text: string): Promise<Response>;
}

export const defaultDevApi: DevApi = {
  catalog: () => getDevCatalog(),
  trigger: (action, opts = {}) => triggerDevAction(action, opts),
  analyze: (text) => analyzeDevDirector(text),
};

export interface DevConsole {
  open(): void;
  close(): void;
  destroy(): void;
  root(): HTMLElement | null;
}

const STRENGTHS: Array<1 | 2 | 3> = [1, 2, 3];

/** 轻量结构化诊断（不入历史，无敏感正文）：前端开发者动作触发/分析的可观测信息。 */
function devLog(event: string, data: Record<string, unknown>): void {
  // eslint-disable-next-line no-console
  console.info(`[dev] ${event}`, { ...data });
}

/** Choreography action ids that are timeline choreography (mirrors action-registry). */
const CHOREO_IDS = new Set([
  'wave_big',
  'bow',
  'bounce_happy',
  'shake_no',
  'lean_in',
  'look_around',
]);

/** 纯逻辑：目录 ids → 动作条目（id + kind），供 UI 渲染与 node 单测使用。 */
export function catalogEntries(ids: string[]): Array<{ id: string; kind: 'gesture' | 'choreo' }> {
  const seen = new Set<string>();
  const out: Array<{ id: string; kind: 'gesture' | 'choreo' }> = [];
  for (const raw of ids) {
    const id = raw.trim().toLowerCase();
    if (!id || seen.has(id)) continue;
    seen.add(id);
    out.push({ id, kind: CHOREO_IDS.has(id) ? 'choreo' : 'gesture' });
  }
  out.sort((a, b) => a.id.localeCompare(b.id));
  return out;
}

/** 纯逻辑：任意输入 → 强度 1|2|3（非法/缺失回落到 2）。 */
export function coerceStrength(v: unknown): 1 | 2 | 3 {
  const n = typeof v === 'number' ? v : Number(String(v ?? '2').trim());
  return (STRENGTHS.includes(n as 1 | 2 | 3) ? n : 2) as 1 | 2 | 3;
}

export function createDevConsole(
  target: HTMLElement,
  opts: { api?: DevApi } = {},
): DevConsole {
  const api = opts.api ?? defaultDevApi;

  const overlay = document.createElement('div');
  overlay.className = 'settings-overlay dev-console-overlay';
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-label', '开发者控制台');

  overlay.innerHTML = `
    <div class="settings-box dev-console-box">
      <div class="settings-header">
        <div class="settings-header-title">
          开发者控制台
          <small>本机导演 / 动作测试台</small>
        </div>
        <button class="settings-close ui-lift dev-console-close" type="button" title="关闭（Esc）" aria-label="关闭开发者控制台">${icon('close')}</button>
      </div>
      <div class="dev-console-body">
        <div class="settings-status-inline dev-console-status" data-status></div>

        <div class="settings-section-title">导演分析</div>
        <div class="settings-section-hint">输入任意文本，查看导演的情绪 / 强度 / 动作 / 时间线结果（已脱敏）。</div>
        <textarea class="settings-config-tree dev-analyze-input" rows="4" spellcheck="false" placeholder="例如：开心地挥手说你好" aria-label="导演分析文本"></textarea>
        <div class="settings-editor-actions">
          <button class="btn primary dev-analyze-btn" type="button">分析</button>
          <span class="settings-tip">本地仅调用导演逻辑，不发送到对话链。</span>
        </div>
        <pre class="dev-result dev-analyze-result" hidden></pre>

        <div class="settings-section-title">动作触发</div>
        <div class="settings-section-hint">目录来自后端白名单；不支持的动作为灰色不可触发。</div>
        <div class="settings-editor-actions">
          <button class="btn ghost dev-catalog-btn" type="button">刷新动作目录</button>
          <span class="dev-halfbody-tip settings-tip"></span>
        </div>
        <div class="dev-action-list" data-actionlist></div>
      </div>
    </div>
  `;

  const statusEl = overlay.querySelector<HTMLElement>('[data-status]')!;
  const analyzeInput = overlay.querySelector<HTMLTextAreaElement>('.dev-analyze-input')!;
  const analyzeBtn = overlay.querySelector<HTMLButtonElement>('.dev-analyze-btn')!;
  const analyzeResult = overlay.querySelector<HTMLElement>('.dev-analyze-result')!;
  const catalogBtn = overlay.querySelector<HTMLButtonElement>('.dev-catalog-btn')!;
  const halfbodyTip = overlay.querySelector<HTMLElement>('.dev-halfbody-tip')!;
  const actionList = overlay.querySelector<HTMLElement>('[data-actionlist]')!;

  const setStatus = (msg: string, kind: 'info' | 'ok' | 'error' = 'info') => {
    statusEl.textContent = msg;
    statusEl.className = `settings-status-inline dev-console-status ${
      kind === 'ok' ? 'is-on' : kind === 'error' ? 'is-off' : ''
    }`;
    statusEl.hidden = !msg;
  };

  const showError = (err: unknown) => {
    setStatus(`失败: ${err instanceof Error ? err.message : String(err)}`, 'error');
  };

  async function readJson<T>(res: Response): Promise<T> {
    let data: T;
    try {
      data = (await res.json()) as T;
    } catch {
      throw new Error(`HTTP ${res.status}（响应不可解析）`);
    }
    if (!res.ok) {
      const msg = (data as { error?: string }).error || `HTTP ${res.status}`;
      throw new Error(msg);
    }
    return data;
  }

  async function refreshCatalog(): Promise<DevCatalog> {
    const data = await readJson<DevCatalog>(await api.catalog());
    devActionRegistry.loadCatalog(data.actions);
    renderActionList();
    const channels = data.halfbody_channels ?? [];
    halfbodyTip.textContent = channels.length
      ? `HalfBody 通道: ${channels.join(' · ')}`
      : '当前模型未加载，无法探测 HalfBody 通道';
    return data;
  }

  function renderActionList(): void {
    const entries = devActionRegistry.list();
    actionList.innerHTML = '';
    for (const entry of entries) {
      const row = document.createElement('div');
      row.className = 'dev-action-row';
      row.dataset.action = entry.id;

      const strengthSelect = document.createElement('select');
      strengthSelect.className = 'dev-strength';
      strengthSelect.setAttribute('aria-label', `${entry.id} 强度`);
      for (const s of STRENGTHS) {
        const opt = document.createElement('option');
        opt.value = String(s);
        opt.textContent = s === 2 ? `${s}（默认）` : String(s);
        if (s === 2) opt.selected = true;
        strengthSelect.appendChild(opt);
      }

      const triggerBtn = document.createElement('button');
      triggerBtn.type = 'button';
      triggerBtn.className = 'btn ghost btn-xs dev-trigger-btn';
      triggerBtn.textContent = entry.kind === 'choreo' ? '触发编排' : '触发';

      const releaseBtn = document.createElement('button');
      releaseBtn.type = 'button';
      releaseBtn.className = 'btn ghost btn-xs dev-release-btn';
      releaseBtn.textContent = '释放';

      const meta = document.createElement('span');
      meta.className = 'settings-tip dev-action-kind';
      meta.textContent = entry.kind;

      row.append(meta, strengthSelect, triggerBtn, releaseBtn);
      actionList.appendChild(row);
    }
  }

  async function analyzeCurrent(): Promise<void> {
    const text = analyzeInput.value.trim();
    if (!text) {
      setStatus('请输入要分析的文本', 'error');
      return;
    }
    analyzeBtn.disabled = true;
    analyzeBtn.innerHTML = `<span class="spinner"></span>分析中…`;
    setStatus('分析中…', 'info');
    analyzeResult.hidden = true;
    try {
      const data = await readJson<DirectorResult>(await api.analyze(text));
      analyzeResult.textContent = JSON.stringify(data, null, 2);
      analyzeResult.hidden = false;
      setStatus('分析完成', 'ok');
      devLog('analyze:ok', {
        input_len: text.length,
        emotion: data.emotion ?? null,
        action: data.motion ?? null,
        strength: data.motion_strength ?? data.strength ?? null,
      });
    } catch (err) {
      showError(err);
      analyzeResult.hidden = true;
      devLog('analyze:error', { input_len: text.length });
    } finally {
      analyzeBtn.disabled = false;
      analyzeBtn.textContent = '分析';
    }
  }

  async function triggerCurrent(action: string, release: boolean): Promise<void> {
    const strengthRaw = release
      ? 2
      : Number(
          actionList.querySelector<HTMLSelectElement>(
            `.dev-action-row[data-action="${action}"] .dev-strength`,
          )?.value ?? 2,
        );
    const strength = (STRENGTHS.includes(strengthRaw as 1 | 2 | 3)
      ? strengthRaw
      : 2) as 1 | 2 | 3;
    setStatus(release ? `释放 ${action}…` : `触发 ${action} (强度 ${strength})…`, 'info');
    try {
      const data = await readJson<{ ok?: boolean; action?: string; strength?: number; error?: string }>(
        await api.trigger(action, { strength, release }),
      );
      setStatus(
        release ? `已释放 ${data.action ?? action}` : `已触发 ${data.action ?? action}（强度 ${data.strength ?? strength}）`,
        'ok',
      );
      devLog(release ? 'trigger:release' : 'trigger:fire', {
        action: data.action ?? action,
        strength: data.strength ?? strength,
      });
    } catch (err) {
      showError(err);
      devLog('trigger:error', { action, release });
    }
  }

  /* ---- 事件绑定（委托：动作列表是动态渲染的） ---- */
  actionList.addEventListener('click', (e) => {
    const btn = (e.target as HTMLElement).closest<HTMLButtonElement>('button');
    if (!btn) return;
    const row = btn.closest<HTMLElement>('.dev-action-row');
    const action = row?.dataset.action;
    if (!action) return;
    if (btn.classList.contains('dev-trigger-btn')) {
      void triggerCurrent(action, false);
    } else if (btn.classList.contains('dev-release-btn')) {
      void triggerCurrent(action, true);
    }
  });
  analyzeBtn.addEventListener('click', () => void analyzeCurrent());
  analyzeInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void analyzeCurrent();
    }
  });
  catalogBtn.addEventListener('click', () => {
    (async () => {
      catalogBtn.disabled = true;
      setStatus('刷新动作目录…', 'info');
      try {
        const data = await refreshCatalog();
        setStatus(`动作目录已刷新（${data.actions.length} 个动作）`, 'ok');
      } catch (err) {
        showError(err);
      } finally {
        catalogBtn.disabled = false;
      }
    })();
  });
  overlay.querySelector<HTMLButtonElement>('.dev-console-close')!.addEventListener('click', close);
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) close();
  });

  const escHandler = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && overlay.classList.contains('open')) close();
  };

  function open(): void {
    target.appendChild(overlay);
    document.addEventListener('keydown', escHandler);
    // 打开即拉一次目录；失败仅反映到状态行，不阻塞面板
    setStatus('', 'info');
    void (async () => {
      try {
        await refreshCatalog();
        setStatus(`动作目录已加载（${devActionRegistry.list().length} 个动作）`, 'ok');
      } catch (err) {
        showError(err);
      }
    })();
    // 触发重排后再加 .open，确保动画生效
    requestAnimationFrame(() => overlay.classList.add('open'));
  }

  function close(): void {
    overlay.classList.remove('open');
    document.removeEventListener('keydown', escHandler);
    // 动画结束后移除节点，避免残留阴影/键盘监听
    setTimeout(() => {
      if (overlay.parentNode === target) target.removeChild(overlay);
    }, 160);
  }

  function destroy(): void {
    close();
  }

  return { open, close, destroy, root: () => overlay };
}
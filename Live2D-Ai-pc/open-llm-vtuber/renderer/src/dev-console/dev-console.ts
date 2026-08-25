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
import { SUPPORTED_ACTIONS } from '../choreography';
import { icon } from '../icons';

export interface DevCatalog {
  actions: string[];
  halfbody_channels: string[];
  capabilities: Record<string, boolean>;
  /** 后端富元数据：中文 label / description（优先消费，取代前端硬编码表）。 */
  action_meta?: Record<string, { label?: string; description?: string }>;
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
  /** 记录一条「最近收到/执行的动作」，显示在停靠面板，便于观察动作是否真的下发。 */
  recordAction(action: string | null | undefined, strength?: number | null, source?: string): void;
}

const STRENGTHS: Array<1 | 2 | 3> = [1, 2, 3];

/** 轻量结构化诊断（不入历史，无敏感正文）：前端开发者动作触发/分析的可观测信息。 */
function devLog(event: string, data: Record<string, unknown>): void {
  // eslint-disable-next-line no-console
  console.info(`[dev] ${event}`, { ...data });
}

/** Choreography action ids that are timeline choreography（白名单动作：
 *  直接取自 choreography 的 SUPPORTED_ACTIONS 单一来源，新增动作自动进入
 *  choreo 分类；bow/lean_in/wave_big/bounce_happy 等身体类动作已删除，
 *  不属于任何 UI 分类）。 */
const CHOREO_IDS: ReadonlySet<string> = new Set<string>(SUPPORTED_ACTIONS);

/** 动作的中文显示名（优先消费后端 action_meta；此处为离线回退，
 *  仅含白名单动作 + release(idle)，不含 bow/happy/sway/wave/bounce）。 */
export function actionZhName(id: string): string {
  const known: Record<string, string> = {
    nod: '点头',
    shake_no: '摇头',
    tilt: '歪头',
    look_around: '左右张望',
    listen: '倾听',
    surprise: '惊讶',
    look_up: '向上看',
    look_down: '向下看',
    glance_left: '瞟一眼（一侧）',
    glance_right: '瞟一眼（另一侧）',
    wink_left: '眨左眼',
    wink_right: '眨右眼',
    smile: '微笑',
    frown: '皱眉',
    pout: '嘟嘴',
    wry_mouth: '歪嘴',
    deep_breath: '深呼吸',
    idle: '待机',
    release: '释放回默认',
  };
  return known[id] ?? id;
}

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
  overlay.className = 'dev-dock dev-dock-right';
  overlay.setAttribute('role', 'complementary');
  overlay.setAttribute('aria-label', '开发者控制台');

  overlay.innerHTML = `
    <div class="dev-dock-header">
      <div class="dev-dock-title">
        开发者控制台
        <small>本机导演 / 动作测试台</small>
      </div>
      <button class="dev-dock-collapse ui-lift" type="button" title="折叠" aria-label="折叠开发者控制台">${icon('chevron')}</button>
      <button class="settings-close ui-lift dev-console-close" type="button" title="关闭（Esc）" aria-label="关闭开发者控制台">${icon('close')}</button>
    </div>
    <div class="dev-dock-body dev-console-body">
      <div class="settings-status-inline dev-console-status" data-status></div>

      <div class="settings-section-title">最近动作</div>
      <div class="dev-recent-actions" data-recent>
        <span class="settings-tip">暂未收到动作</span>
      </div>

      <div class="settings-section-title">导演分析</div>
      <div class="settings-section-hint">输入任意文本，查看导演的情绪 / 强度 / 动作 / 时间线结果（已脱敏）。</div>
      <textarea class="settings-config-tree dev-analyze-input" rows="3" spellcheck="false" placeholder="例如：真的吗？（惊讶地）我不信" aria-label="导演分析文本"></textarea>
      <div class="settings-editor-actions">
        <button class="btn primary dev-analyze-btn" type="button">分析</button>
        <span class="settings-tip">本地仅调用导演逻辑，不发送到对话链。</span>
      </div>
      <pre class="dev-result dev-analyze-result" hidden></pre>

      <div class="settings-section-title">动作触发</div>
      <div class="settings-section-hint">目录来自后端白名单并按当前模型参数能力过滤（缺门槛参数的动作不显示）；动作只覆盖头/脸参数，身体通道已全面禁止。</div>
      <div class="settings-editor-actions">
        <button class="btn ghost dev-catalog-btn" type="button">刷新动作目录</button>
        <span class="dev-halfbody-tip settings-tip"></span>
      </div>
      <div class="dev-action-list" data-actionlist></div>
    </div>
  `;

  const statusEl = overlay.querySelector<HTMLElement>('[data-status]')!;
  const analyzeInput = overlay.querySelector<HTMLTextAreaElement>('.dev-analyze-input')!;
  const analyzeBtn = overlay.querySelector<HTMLButtonElement>('.dev-analyze-btn')!;
  const analyzeResult = overlay.querySelector<HTMLElement>('.dev-analyze-result')!;
  const catalogBtn = overlay.querySelector<HTMLButtonElement>('.dev-catalog-btn')!;
  const halfbodyTip = overlay.querySelector<HTMLElement>('.dev-halfbody-tip')!;
  const actionList = overlay.querySelector<HTMLElement>('[data-actionlist]')!;
  const collapseBtn = overlay.querySelector<HTMLButtonElement>('.dev-dock-collapse')!;
  const recentEl = overlay.querySelector<HTMLElement>('[data-recent]')!;
  const recentEntries: Array<{ action: string; strength: number; source: string; at: number }> = [];

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
    devActionRegistry.loadCatalog(data.actions, data.action_meta);
    renderActionList();
    const channels = data.halfbody_channels ?? [];
    halfbodyTip.textContent = channels.length
      ? `HalfBody 通道: ${channels.join(' · ')}`
      : '当前模型未加载，无法探测 HalfBody 通道';
    return data;
  }

  function renderRecentActions(): void {
    if (recentEntries.length === 0) {
      recentEl.innerHTML = '<span class="settings-tip">暂未收到动作</span>';
      return;
    }
    recentEl.innerHTML = '';
    const frag = document.createDocumentFragment();
    for (const rec of recentEntries.slice(0, 6)) {
      const item = document.createElement('div');
      item.className = 'dev-recent-item';
      const t = new Date(rec.at).toLocaleTimeString([], { hour12: false });
      const name = actionZhName(rec.action);
      const label = rec.action === name ? rec.action : `${name}（${rec.action}）`;
      item.textContent = `${t} | ${label} · 强度 ${rec.strength} · ${rec.source}`;
      frag.appendChild(item);
    }
    recentEl.appendChild(frag);
  }

  function recordAction(
    action: string | null | undefined,
    strength?: number | null,
    source = '对话',
  ): void {
    const id = (action ?? '').trim().toLowerCase();
    const s = coerceStrength(strength ?? 2);
    recentEntries.unshift({ action: id || '-', strength: s, source, at: Date.now() });
    if (recentEntries.length > 12) recentEntries.pop();
    renderRecentActions();
    devLog('motion:recorded', { action: id || null, strength: s, source });
  }

  function renderActionList(): void {
    const entries = devActionRegistry.list();
    actionList.innerHTML = '';
    for (const entry of entries) {
      const row = document.createElement('div');
      row.className = 'dev-action-row';
      row.dataset.action = entry.id;

      const nameWrap = document.createElement('div');
      nameWrap.className = 'dev-action-name';
      nameWrap.innerHTML = `<span class="dev-action-cn">${entry.name}</span>
        <code class="dev-action-id">${entry.id}</code>`;

      const desc = document.createElement('div');
      desc.className = 'dev-action-desc';
      desc.textContent = entry.description;

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

      const controls = document.createElement('div');
      controls.className = 'dev-action-controls';
      controls.append(strengthSelect, triggerBtn, releaseBtn);

      row.append(nameWrap, desc, controls, meta);
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
  const closeBtn = overlay.querySelector<HTMLButtonElement>('.dev-console-close')!;
  closeBtn.addEventListener('click', close);

  // 折叠：仅隐藏面板主体（保留顶部窄条），不销毁实例，方便随时展开对照模型。
  // collapsed 时容器不应再占用整条右侧（见 settings.css），并为辅助技术同步展开状态。
  function setCollapsed(collapsed: boolean): void {
    overlay.classList.toggle('collapsed', collapsed);
    const label = collapsed ? '展开开发者控制台' : '折叠开发者控制台';
    collapseBtn.setAttribute('aria-label', label);
    collapseBtn.title = label;
    collapseBtn.setAttribute('aria-expanded', String(!collapsed));
  }

  collapseBtn.addEventListener('click', () => {
    setCollapsed(!overlay.classList.contains('collapsed'));
  });

  const escHandler = (e: KeyboardEvent) => {
    if (e.key === 'Escape' && overlay.parentNode === target) close();
  };

  function open(): void {
    // 幂等：已在 DOM 中则不重复 append，避免重复拉取目录/重复监听
    if (overlay.parentNode !== target) {
      target.appendChild(overlay);
      document.addEventListener('keydown', escHandler);
    }
    // 打开时总是展开（显示主体），与 collapsed 窄条状态保持一致且辅助技术同步。
    setCollapsed(false);
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
  }

  function close(): void {
    document.removeEventListener('keydown', escHandler);
    if (overlay.parentNode === target) target.removeChild(overlay);
  }

  function destroy(): void {
    close();
  }

  return { open, close, destroy, root: () => overlay, recordAction };
}
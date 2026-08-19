/**
 * settings-ui.ts — PC 端设置中心 UI（DSH / Codex 深色精练风）
 *
 * 布局：模态覆盖层 + 左侧导航 rail（6 分区）+ 右侧内容区（一次一个 section）+ 底部条。
 *   分区：对话/LLM · 语音/TTS · 形象/Live2D · 密钥 · 人设/提示词 · 系统/高级
 *
 * 职责：只做 DOM 装配 + 事件接线；纯逻辑（collectPayload / 校验 / 选项构建 / touched）
 * 抽到 settings-logic.ts 以便 vitest 直接单测。
 *
 * 对外契约：`initSettingsPanel(container, opts): SettingsPanel {open, close, destroy}`，
 * 与 main.ts 的用法保持不变。
 */

import {
  buildModelOptions,
  buildSelectOptions,
  checkPort,
  collectPayload,
  type SettingsUiState,
  type LlmProviderUi,
  type TtsProviderUi,
} from './settings-logic';
import './settings.css';

/* ------------------------------------------------------------------ *
 * 数据契约（与后端 GET /api/config 同构；以 spec 1.2–1.5 为准）
 * ------------------------------------------------------------------ */

interface LlmProviderCfg {
  id: string;
  name?: string;
  model: string;
  models?: string[];
  base_url: string;
  temperature: number;
  max_tokens?: number | null;
  key_env?: string | null;
  has_key: boolean;
  configured?: boolean;
}

interface TtsProviderCfg {
  id: string;
  name?: string;
  voice: string;
  model: string;
  ws_url?: string;
  voices?: string[];
  models?: string[];
  key_env?: string | null;
  has_key: boolean;
  configured?: boolean;
}

interface PersonaCfg {
  name: string;
  role: string;
  model: string;
  temperature: number;
  max_tokens?: number | null;
  vision_model: string;
  system_prompt: string;
}

interface AppConfig {
  llm_provider: string;
  llm_providers: string[];
  llm_configs: LlmProviderCfg[];
  tts_model: string;
  viseme_mode: string;
  tts_providers: string[];
  tts_configs: TtsProviderCfg[];
  asr_model: string;
  asr_providers: string[];
  character_name: string;
  port: number;
  live2d_fps: number;
  log_level: string;
  persona: PersonaCfg;
}

interface SaveResponse {
  ok?: boolean;
  config?: AppConfig;
  applied?: boolean;
  requires_restart?: boolean;
  message?: string;
  error?: string;
  field_errors?: Record<string, string>;
}

interface Live2DModel {
  id: string;
  name: string;
  description?: string;
  default?: boolean;
}

export interface SettingsPanel {
  open(): void;
  close(): void;
  destroy(): void;
}

const LLM_PROVIDER_LABELS: Record<string, string> = {
  deepseek_llm: 'DeepSeek',
  zhipu_llm: '智谱 GLM',
  zhipu_free_llm: '免费 GLM',
  qwen_llm: 'Qwen',
  openrouter: 'OpenRouter',
  openai: 'OpenAI',
  anthropic: 'Anthropic',
  moonshot: 'Moonshot',
  groq: 'Groq',
  mistral: 'Mistral',
  together: 'Together AI',
  fireworks: 'Fireworks AI',
  siliconflow: 'SiliconFlow',
  opencode: 'OpenCode',
  opencode_go: 'OpenCode Go',
  openai_compatible_llm: 'OpenAI 兼容',
  openai_compatible: 'OpenAI 兼容（旧）',
};

const TTS_PROVIDER_LABELS: Record<string, string> = {
  cosyvoice_cloud: 'CosyVoice',
  qwen_tts: 'Qwen TTS',
  edge_tts: 'Edge TTS',
  openai_tts: 'OpenAI TTS',
  azure_tts: 'Azure TTS',
  minimax_tts: 'MiniMax',
  siliconflow_tts: 'SiliconFlow TTS',
  fish_api_tts: 'Fish Audio',
  spark_tts: 'Spark TTS',
  gpt_sovits_tts: 'GPT-SoVITS',
  piper_tts: 'Piper',
  sherpa_onnx_tts: 'Sherpa-ONNX',
  pyttsx3_tts: 'pyttsx3',
  gpt_sovits: 'GPT-SoVITS（旧）',
  piper: 'Piper（旧）',
  sherpa_onnx: 'Sherpa-ONNX（旧）',
  pyttsx3: 'pyttsx3（旧）',
};

const LLM_CATEGORIES = [
  { id: 'official', label: '官方直连', providers: ['deepseek_llm', 'zhipu_llm', 'qwen_llm', 'openai', 'anthropic'] },
  { id: 'aggregator', label: '聚合平台', providers: ['openrouter', 'together', 'fireworks', 'groq', 'moonshot', 'mistral', 'siliconflow'] },
  { id: 'custom', label: '其他 / 自定义', providers: ['opencode', 'opencode_go', 'openai_compatible_llm'] },
];

const TTS_CATEGORIES = [
  { id: 'cloud', label: '云端 TTS', providers: ['cosyvoice_cloud', 'qwen_tts', 'openai_tts', 'azure_tts', 'minimax_tts', 'siliconflow_tts', 'fish_api_tts', 'spark_tts'] },
  { id: 'local', label: '本地 / 免费', providers: ['edge_tts', 'gpt_sovits_tts', 'piper_tts', 'sherpa_onnx_tts', 'pyttsx3_tts'] },
];

const EMPTY_CONFIG: AppConfig = {
  llm_provider: '',
  llm_providers: [],
  llm_configs: [],
  tts_model: '',
  viseme_mode: 'rms',
  tts_providers: [],
  tts_configs: [],
  asr_model: 'disabled',
  asr_providers: [],
  character_name: '白',
  port: 12393,
  live2d_fps: 120,
  log_level: 'INFO',
  persona: {
    name: '',
    role: '',
    model: '',
    temperature: 0.8,
    max_tokens: null,
    vision_model: '',
    system_prompt: '',
  },
};

function esc(text: string): string {
  return String(text ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

const providerLabel = (id: string, map: Record<string, string>): string => map[id] || id;

/* ================================================================== *
 * 面板
 * ================================================================== */

export function initSettingsPanel(
  container: HTMLElement,
  opts: { onChanged?: () => void } = {},
): SettingsPanel {
  const overlay = document.createElement('div');
  overlay.className = 'settings-overlay';
  overlay.innerHTML = `
    <div class="settings-box">
      <div class="settings-header">
        <div class="settings-header-title">
          设置中心
          <small>Live2D-Ai PC</small>
        </div>
        <button class="settings-close" type="button" title="关闭">✕</button>
      </div>

      <div class="settings-layout">
        <nav class="settings-nav">
          <div class="settings-nav-group">配置</div>
          <button class="settings-tab active" type="button" data-tab="llm"><span class="tab-dot"></span>对话 / LLM</button>
          <button class="settings-tab" type="button" data-tab="tts"><span class="tab-dot"></span>语音 / TTS</button>
          <button class="settings-tab" type="button" data-tab="live2d"><span class="tab-dot"></span>形象 / Live2D</button>
          <button class="settings-tab" type="button" data-tab="keys"><span class="tab-dot"></span>密钥</button>
          <button class="settings-tab" type="button" data-tab="persona"><span class="tab-dot"></span>人设 / 提示词</button>
          <button class="settings-tab" type="button" data-tab="system"><span class="tab-dot"></span>系统 / 高级</button>
          <div class="settings-nav-status">
            <div class="sv-label">状态简报</div>
            <div class="sv-line"><span class="status-dot" data-nav="llm"></span><span data-navtext="llm">LLM –</span></div>
            <div class="sv-line"><span class="status-dot" data-nav="tts"></span><span data-navtext="tts">TTS –</span></div>
            <div class="sv-line"><span class="status-dot" data-nav="sys"></span><span data-navtext="sys">系统 –</span></div>
          </div>
        </nav>

        <div class="settings-content">
          <div class="settings-error-bar" data-errorbar></div>

          <section class="settings-tab-panel active" data-panel="llm">
            <div class="settings-section-title">对话 / LLM</div>
            <label class="settings-row"><span class="lbl">LLM 提供商</span><select class="settings-llm"></select></label>
            <div class="settings-editor-actions">
              <button class="btn ghost settings-check-llm" type="button">全部检查可用性</button>
              <span class="settings-tip">保存后生效；打开面板不会自动探测模型。</span>
            </div>
            <div class="settings-group-title">Provider 参数</div>
            <div class="settings-llm-cards"></div>
          </section>

          <section class="settings-tab-panel" data-panel="tts">
            <div class="settings-section-title">语音 / TTS</div>
            <label class="settings-row"><span class="lbl">TTS 提供商</span><select class="settings-tts"></select></label>
            <label class="settings-row"><span class="lbl">ASR 引擎</span><select class="settings-asr"></select></label>
            <label class="settings-row"><span class="lbl">口型模式</span>
              <select class="settings-viseme-mode">
                <option value="rms">RMS 快速（推荐）</option>
                <option value="off">关闭 viseme（最快）</option>
                <option value="pymouth">Pymouth 精细（慢）</option>
              </select>
            </label>
            <div class="settings-group-title">TTS 参数</div>
            <div class="settings-tip">「测试/试听」仅检查 Key 是否已配置，不执行真实合成。</div>
            <div class="settings-tts-cards"></div>
          </section>

          <section class="settings-tab-panel" data-panel="live2d">
            <div class="settings-section-title">形象 / Live2D</div>
            <div class="settings-section-hint">选择当前使用的 Live2D 模型；扫描可导入本地模型。</div>
            <div class="settings-editor-actions">
              <button class="btn ghost settings-scan-models" type="button">扫描并导入本地模型</button>
            </div>
            <div class="settings-status-inline" data-modelstatus></div>
            <div class="settings-model-list" data-modellist></div>
          </section>

          <section class="settings-tab-panel" data-panel="keys">
            <div class="settings-section-title">密钥（写入 .env）</div>
            <div class="settings-section-hint">留空表示不修改该 Key；「清除」会显式删除已保存的 Key（置 null 发送）。</div>
            <div class="settings-key-cards"></div>
          </section>

          <section class="settings-tab-panel" data-panel="persona">
            <div class="settings-section-title">人设 / 提示词</div>
            <label class="settings-row"><span class="lbl">System Prompt</span>
              <textarea class="settings-persona-prompt" rows="12" spellcheck="false"></textarea>
            </label>
            <div class="settings-editor-actions">
              <button class="btn danger settings-persona-clear-all" type="button">清除全部人设字段</button>
            </div>
            <div class="settings-group-title">人设参数</div>
            <div class="settings-edit-stack">
              <div class="settings-row"><span class="lbl">角色名</span><input class="settings-persona-name" type="text" /><button class="btn ghost btn-xs persona-clear" data-field="name" type="button">清除</button></div>
              <div class="settings-row"><span class="lbl">角色定位</span><input class="settings-persona-role" type="text" /><button class="btn ghost btn-xs persona-clear" data-field="role" type="button">清除</button></div>
              <div class="settings-row"><span class="lbl">模型</span><input class="settings-persona-model" type="text" data-mono /><button class="btn ghost btn-xs persona-clear" data-field="model" type="button">清除</button></div>
              <div class="settings-row"><span class="lbl">Temperature</span><input class="settings-persona-temperature" type="number" step="0.1" min="0" max="2" /></div>
              <div class="settings-row"><span class="lbl">Max Tokens</span><input class="settings-persona-max-tokens" type="number" min="1" /></div>
              <div class="settings-row"><span class="lbl">视觉模型</span><input class="settings-persona-vision-model" type="text" data-mono /><button class="btn ghost btn-xs persona-clear" data-field="vision_model" type="button">清除</button></div>
            </div>
          </section>

          <section class="settings-tab-panel" data-panel="system">
            <div class="settings-section-title">系统 / 高级</div>
            <div class="settings-edit-stack">
              <div class="settings-row"><span class="lbl">角色名（conf.yaml）</span><input class="settings-character-name" type="text" /></div>
              <div class="settings-row" data-portrow><span class="lbl">端口</span><input class="settings-port" type="number" min="1" max="65535" data-mono /><div class="field-error-msg"></div></div>
              <div class="settings-row"><span class="lbl">Live2D FPS</span><input class="settings-fps" type="number" min="1" max="240" /></div>
              <div class="settings-row"><span class="lbl">日志级别</span>
                <select class="settings-log-level">
                  <option value="INFO">INFO（推荐）</option>
                  <option value="DEBUG">DEBUG（调试）</option>
                  <option value="WARNING">WARNING（安静）</option>
                </select>
              </div>
            </div>
            <div class="settings-tip">端口与日志在保存并重启后端后生效。</div>
            <div class="settings-group-title">高级：conf.yaml 设定树</div>
            <label class="settings-row"><span class="lbl">原始 YAML</span>
              <textarea class="settings-config-tree" rows="14" spellcheck="false"></textarea>
            </label>
            <div class="settings-editor-actions">
              <button class="btn settings-save-tree" type="button">保存设定树</button>
              <span class="settings-tip">保存前会做 schema 校验，非法则不会写盘。</span>
            </div>
          </section>
        </div>
      </div>

      <div class="settings-footer">
        <div class="settings-status" data-status></div>
        <div class="settings-footer-actions">
          <button class="btn ghost settings-apply" type="button">保存并应用</button>
          <button class="btn primary settings-save" type="button">保存全部</button>
          <button class="btn settings-close2" type="button">关闭</button>
        </div>
      </div>
    </div>
  `;
  container.appendChild(overlay);

  /* ---- 元素句柄 ---- */
  const statusEl = overlay.querySelector<HTMLElement>('[data-status]')!;
  const errorBarEl = overlay.querySelector<HTMLElement>('[data-errorbar]')!;
  const tabs = Array.from(overlay.querySelectorAll<HTMLButtonElement>('.settings-tab'));
  const panels = Array.from(overlay.querySelectorAll<HTMLElement>('.settings-tab-panel'));
  const llmCardsEl = overlay.querySelector<HTMLElement>('.settings-llm-cards')!;
  const ttsCardsEl = overlay.querySelector<HTMLElement>('.settings-tts-cards')!;
  const keyCardsEl = overlay.querySelector<HTMLElement>('.settings-key-cards')!;
  const modelListEl = overlay.querySelector<HTMLElement>('[data-modellist]')!;
  const modelStatusEl = overlay.querySelector<HTMLElement>('[data-modelstatus]')!;
  const configTreeEl = overlay.querySelector<HTMLTextAreaElement>('.settings-config-tree')!;
  const saveBtn = overlay.querySelector<HTMLButtonElement>('.settings-save')!;
  const applyBtn = overlay.querySelector<HTMLButtonElement>('.settings-apply')!;

  /* ---- 运行态 ---- */
  let currentConfig: AppConfig = EMPTY_CONFIG;
  let loaded = false;
  let logLevelChanged = false;
  let requestToken = 0;
  // 模型发现结果缓存（P2-1：手动获取 + 结果缓存，避免重复拉取）
  const discoverCache = new Map<string, string[]>();

  /* ---- 状态/错误呈现 ---- */
  const setStatus = (msg: string, kind: 'info' | 'ok' | 'error' = 'info'): void => {
    statusEl.textContent = msg;
    statusEl.className = `settings-status ${kind === 'ok' ? 'ok' : kind === 'error' ? 'error' : ''}`;
  };
  const setErrorBar = (msg: string): void => {
    errorBarEl.textContent = msg;
    errorBarEl.classList.toggle('show', !!msg);
  };
  const setSaveButtonsEnabled = (en: boolean): void => {
    saveBtn.disabled = !en;
    applyBtn.disabled = !en;
  };

  /* ---- busy + 最后点击胜出 ---- */
  const withBusy = (
    btn: HTMLButtonElement | null,
    fn: (token: number) => Promise<void>,
  ): void => {
    const token = ++requestToken;
    const prevLabel = btn ? btn.dataset.label || btn.textContent || '' : '';
    if (btn) {
      btn.dataset.label = prevLabel;
      btn.disabled = true;
      btn.innerHTML = `<span class="spinner"></span>${prevLabel}`;
    }
    fn(token)
      .catch((err) => {
        // 仅在仍是最新 token 时才更新状态
        if (token === requestToken) setStatus(`操作失败: ${(err as Error).message}`, 'error');
      })
      .finally(() => {
        if (btn && token === requestToken) {
          btn.disabled = false;
          btn.innerHTML = prevLabel;
        }
      });
  };

  /* ---- tab 切换 ---- */
  const switchTab = (name: string): void => {
    tabs.forEach((t) => t.classList.toggle('active', t.dataset.tab === name));
    panels.forEach((p) => p.classList.toggle('active', p.dataset.panel === name));
  };
  tabs.forEach((tab) => {
    tab.addEventListener('click', () => switchTab(tab.dataset.tab || 'llm'));
  });

  const close = (): void => {
    overlay.style.display = 'none';
  };

  /* ---- 下拉填充（占位逻辑在 settings-logic.buildSelectOptions） ---- */
  const fillSelect = (
    sel: HTMLSelectElement,
    values: string[],
    current: string,
    labelMap?: Record<string, string>,
  ): void => {
    sel.innerHTML = '';
    for (const o of buildSelectOptions(values, current, labelMap)) {
      const opt = document.createElement('option');
      opt.value = o.value;
      opt.textContent = o.label;
      opt.selected = o.selected;
      sel.appendChild(opt);
    }
  };

  const fillModelSelect = (sel: HTMLSelectElement, current: string, models?: string[]): void => {
    sel.innerHTML = '';
    for (const m of buildModelOptions(current, models)) {
      const opt = document.createElement('option');
      opt.value = m;
      opt.textContent = m;
      opt.selected = m === current;
      sel.appendChild(opt);
    }
    if (sel.options.length === 0) {
      const opt = document.createElement('option');
      opt.value = '';
      opt.textContent = '未选择';
      sel.appendChild(opt);
    }
  };

  /* ---- nav 状态简报 ---- */
  const updateNavStatus = (): void => {
    const llmTotal = currentConfig.llm_configs.length;
    const llmOk = currentConfig.llm_configs.filter((c) => c.configured && c.has_key).length;
    const ttsTotal = currentConfig.tts_configs.length;
    const ttsOk = currentConfig.tts_configs.filter((c) => c.configured).length;
    const navLlmDot = overlay.querySelector<HTMLElement>('[data-nav="llm"]');
    const navTtsDot = overlay.querySelector<HTMLElement>('[data-nav="tts"]');
    const navSysDot = overlay.querySelector<HTMLElement>('[data-nav="sys"]');
    const navLlm = overlay.querySelector<HTMLElement>('[data-navtext="llm"]');
    const navTts = overlay.querySelector<HTMLElement>('[data-navtext="tts"]');
    const navSys = overlay.querySelector<HTMLElement>('[data-navtext="sys"]');
    if (navLlmDot) navLlmDot.className = `status-dot ${llmTotal > 0 ? (llmOk > 0 ? 'ok' : 'warn') : 'bad'}`;
    if (navTtsDot) navTtsDot.className = `status-dot ${ttsTotal > 0 ? (ttsOk > 0 ? 'ok' : 'warn') : ''}`;
    if (navSysDot) navSysDot.className = `status-dot ${currentConfig.port ? 'ok' : 'bad'}`;
    if (navLlm) navLlm.textContent = `LLM ${llmOk}/${llmTotal} 可用`;
    if (navTts) navTts.textContent = `TTS ${ttsOk}/${ttsTotal} 已配置`;
    if (navSys) navSys.textContent = `端口 ${currentConfig.port || '—'}`;
  };

  /* ================================================================== *
   * Provider 卡渲染
   * ================================================================== */

  const renderLlmCards = (): void => {
    llmCardsEl.innerHTML = '';
    const groups = LLM_CATEGORIES.map((c) => ({ id: c.id, label: c.label, ids: c.providers, items: [] as LlmProviderCfg[] }));
    const other = { id: 'other', label: '其他', ids: [] as string[], items: [] as LlmProviderCfg[] };
    for (const p of currentConfig.llm_configs || []) {
      const g = groups.find((c) => c.ids.includes(p.id));
      if (g) g.items.push(p);
      else other.items.push(p);
    }
    const allGroups = [...groups.filter((g) => g.items.length > 0), ...(other.items.length ? [other] : [])];
    if (allGroups.length === 0) {
      llmCardsEl.innerHTML = '<div class="settings-tip">暂无 LLM Provider 配置。</div>';
    }
    for (const group of allGroups) {
      const title = document.createElement('div');
      title.className = 'settings-group-title';
      title.textContent = `${group.label}（${group.items.length}）`;
      llmCardsEl.appendChild(title);
      for (const p of group.items) {
        llmCardsEl.appendChild(buildLlmProviderCard(p));
      }
    }
  };

  const buildLlmProviderCard = (p: LlmProviderCfg): HTMLElement => {
    const row = document.createElement('div');
    row.className = 'settings-provider-row';
    const badge =
      p.has_key
        ? '<span class="settings-badge ok"><span class="status-dot"></span>已配置</span>'
        : '<span class="settings-badge warn"><span class="status-dot"></span>未配置 Key</span>';
    row.innerHTML = `
      <div class="settings-provider-row-title">
        <span>${esc(p.name || providerLabel(p.id, LLM_PROVIDER_LABELS))}</span>
        <span class="settings-mono settings-provider-id">${esc(p.id)}</span>
        <span class="settings-badge-wrap">${badge}</span>
      </div>
      <button class="settings-provider-toggle" type="button">编辑</button>
    `;
    const editor = document.createElement('div');
    editor.className = 'settings-provider-editor';
    editor.dataset.provider = p.id;
    editor.dataset.configured = p.configured ? '1' : '0';
    editor.dataset.keyEnv = p.key_env || '';
    editor.dataset.type = 'llm';
    editor.innerHTML = `
      <div class="settings-edit-stack">
        <div class="settings-row"><span class="lbl">模型</span>
          <select class="llm-model" data-mono></select>
        </div>
        <div class="settings-row"><span class="lbl">Base URL</span><input class="llm-base-url" type="text" data-mono /></div>
        <div class="settings-row"><span class="lbl">Temperature</span><input class="llm-temperature" type="number" step="0.1" min="0" max="2" /></div>
        <div class="settings-row"><span class="lbl">Max Tokens</span><input class="llm-max-tokens" type="number" min="1" /></div>
      </div>
      <div class="settings-editor-actions">
        <button class="btn ghost settings-discover-models" type="button" data-provider="${esc(p.id)}" data-key-env="${esc(p.key_env || '')}">获取可用模型</button>
        <button class="btn ghost settings-test-llm" type="button" data-provider="${esc(p.id)}">测试连接</button>
        <span class="settings-tip settings-editor-help" data-llmhelp></span>
      </div>
    `;
    fillModelSelect(editor.querySelector<HTMLSelectElement>('.llm-model')!, p.model, p.models);
    editor.querySelector<HTMLInputElement>('.llm-base-url')!.value = p.base_url;
    editor.querySelector<HTMLInputElement>('.llm-temperature')!.value = String(p.temperature ?? 0.8);
    editor.querySelector<HTMLInputElement>('.llm-max-tokens')!.value = p.max_tokens ? String(p.max_tokens) : '';

    const toggle = row.querySelector<HTMLButtonElement>('.settings-provider-toggle')!;
    const openEditor = (): void => {
      const show = !editor.classList.contains('open');
      editor.classList.toggle('open', show);
      toggle.textContent = show ? '收起' : '编辑';
      if (show && !p.configured) editor.dataset.touched = '1';
    };
    toggle.addEventListener('click', openEditor);

    editor.querySelector<HTMLButtonElement>('.settings-discover-models')!.addEventListener('click', () => {
      void discoverModels(p.id, editor);
    });
    editor.querySelector<HTMLButtonElement>('.settings-test-llm')!.addEventListener('click', () => {
      void testLlm(p.id, undefined, editor);
    });

    const wrapper = document.createElement('div');
    wrapper.appendChild(row);
    wrapper.appendChild(editor);
    return wrapper;
  };

  const renderTtsCards = (): void => {
    ttsCardsEl.innerHTML = '';
    const groups = TTS_CATEGORIES.map((c) => ({ id: c.id, label: c.label, ids: c.providers, items: [] as TtsProviderCfg[] }));
    const other = { id: 'other', label: '其他', ids: [] as string[], items: [] as TtsProviderCfg[] };
    for (const p of currentConfig.tts_configs || []) {
      const g = groups.find((c) => c.ids.includes(p.id));
      if (g) g.items.push(p);
      else other.items.push(p);
    }
    const allGroups = [...groups.filter((g) => g.items.length > 0), ...(other.items.length ? [other] : [])];
    if (allGroups.length === 0) {
      ttsCardsEl.innerHTML = '<div class="settings-tip">暂无 TTS Provider 配置。</div>';
    }
    for (const group of allGroups) {
      const title = document.createElement('div');
      title.className = 'settings-group-title';
      title.textContent = `${group.label}（${group.items.length}）`;
      ttsCardsEl.appendChild(title);
      for (const p of group.items) {
        ttsCardsEl.appendChild(buildTtsProviderCard(p));
      }
    }
  };

  const buildTtsProviderCard = (p: TtsProviderCfg): HTMLElement => {
    const row = document.createElement('div');
    row.className = 'settings-provider-row';
    const badge =
      p.has_key
        ? '<span class="settings-badge ok"><span class="status-dot"></span>已配置</span>'
        : '<span class="settings-badge warn"><span class="status-dot"></span>未配置 Key</span>';
    row.innerHTML = `
      <div class="settings-provider-row-title">
        <span>${esc(p.name || providerLabel(p.id, TTS_PROVIDER_LABELS))}</span>
        <span class="settings-mono settings-provider-id">${esc(p.id)}</span>
        ${badge}
      </div>
      <button class="settings-provider-toggle" type="button">编辑</button>
    `;
    const editor = document.createElement('div');
    editor.className = 'settings-provider-editor';
    editor.dataset.provider = p.id;
    editor.dataset.configured = p.configured ? '1' : '0';
    editor.dataset.keyEnv = p.key_env || '';
    editor.dataset.type = 'tts';
    editor.innerHTML = `
      <div class="settings-edit-stack">
        <div class="settings-row"><span class="lbl">音色 Voice</span><select class="tts-voice" data-mono></select></div>
        <div class="settings-row"><span class="lbl">模型</span><select class="tts-model" data-mono></select></div>
        <div class="settings-row"><span class="lbl">WebSocket URL</span><input class="tts-ws-url" type="text" data-mono /></div>
      </div>
      <div class="settings-editor-actions">
        <button class="btn ghost settings-test-tts" type="button">试听/检查</button>
        <span class="settings-tip settings-editor-help">仅检查 Key 是否已配置，不真实合成。</span>
      </div>
    `;
    fillModelSelect(editor.querySelector<HTMLSelectElement>('.tts-voice')!, p.voice, p.voices);
    fillModelSelect(editor.querySelector<HTMLSelectElement>('.tts-model')!, p.model, p.models);
    editor.querySelector<HTMLInputElement>('.tts-ws-url')!.value = p.ws_url || '';

    const toggle = row.querySelector<HTMLButtonElement>('.settings-provider-toggle')!;
    toggle.addEventListener('click', () => {
      const show = !editor.classList.contains('open');
      editor.classList.toggle('open', show);
      toggle.textContent = show ? '收起' : '编辑';
      if (show && !p.configured) editor.dataset.touched = '1';
    });
    editor.querySelector<HTMLButtonElement>('.settings-test-tts')!.addEventListener('click', () => {
      void testTts(editor);
    });

    const wrapper = document.createElement('div');
    wrapper.appendChild(row);
    wrapper.appendChild(editor);
    return wrapper;
  };

  /* ================================================================== *
   * Key 卡渲染
   * ================================================================== */

  const KEY_GROUPS: Array<{
    label: string;
    items: Array<{ env: string; label: string; provider?: string }>;
  }> = [
    {
      label: 'LLM 密钥',
      items: [
        { env: 'DEEPSEEK_API_KEY', label: 'DeepSeek', provider: 'deepseek_llm' },
        { env: 'ZHIPU_API_KEY', label: '智谱 GLM', provider: 'zhipu_llm' },
        { env: 'DASHSCOPE_API_KEY', label: 'Qwen / DashScope', provider: 'qwen_llm' },
        { env: 'OPENROUTER_API_KEY', label: 'OpenRouter', provider: 'openrouter' },
        { env: 'OPENAI_API_KEY', label: 'OpenAI / 兼容', provider: 'openai' },
        { env: 'ANTHROPIC_API_KEY', label: 'Anthropic', provider: 'anthropic' },
        { env: 'MOONSHOT_API_KEY', label: 'Moonshot', provider: 'moonshot' },
        { env: 'GROQ_API_KEY', label: 'Groq', provider: 'groq' },
        { env: 'MISTRAL_API_KEY', label: 'Mistral', provider: 'mistral' },
        { env: 'TOGETHER_API_KEY', label: 'Together AI', provider: 'together' },
        { env: 'FIREWORKS_API_KEY', label: 'Fireworks AI', provider: 'fireworks' },
        { env: 'SILICONFLOW_API_KEY', label: 'SiliconFlow', provider: 'siliconflow' },
        { env: 'OPENCODE_API_KEY', label: 'OpenCode', provider: 'opencode' },
        { env: 'OPENCODE_GO_API_KEY', label: 'OpenCode Go', provider: 'opencode_go' },
      ],
    },
    {
      label: 'TTS / 语音密钥',
      items: [
        { env: 'ALIYUN_API_KEY', label: 'CosyVoice / 阿里云' },
        { env: 'AZURE_SPEECH_KEY', label: 'Azure TTS' },
        { env: 'MINIMAX_API_KEY', label: 'MiniMax' },
        { env: 'SILICONFLOW_API_KEY', label: 'SiliconFlow TTS' },
        { env: 'FISH_API_KEY', label: 'Fish Audio' },
        { env: 'SPARK_API_KEY', label: 'Spark TTS' },
      ],
    },
  ];

  const renderKeyCards = (): void => {
    keyCardsEl.innerHTML = '';
    for (const group of KEY_GROUPS) {
      const title = document.createElement('div');
      title.className = 'settings-group-title';
      title.textContent = group.label;
      keyCardsEl.appendChild(title);
      for (const item of group.items) {
        const card = document.createElement('div');
        card.className = 'settings-card settings-key-card';
        card.innerHTML = `
          <div class="settings-key-head">
            <div class="settings-key-label">
              <span>${esc(item.label)}</span>
              <span class="settings-mono settings-provider-id">${esc(item.env)}</span>
              <span class="key-clear-state" data-keystate="${esc(item.env)}"></span>
            </div>
            <div class="settings-editor-actions">
              ${item.provider ? `<button class="btn ghost settings-key-test" type="button" data-provider="${esc(item.provider)}" data-env="${esc(item.env)}">测试连接</button>` : ''}
              <button class="btn danger btn-sm settings-key-clear" type="button" data-env="${esc(item.env)}">清除</button>
            </div>
          </div>
          <div class="settings-key-row">
            <input class="settings-key-input" type="password" data-env="${esc(item.env)}" placeholder="留空不修改" autocomplete="off" />
            <button class="btn ghost settings-key-toggle" type="button" data-env="${esc(item.env)}">显示</button>
          </div>
        `;
        keyCardsEl.appendChild(card);
      }
    }

    keyCardsEl.querySelectorAll<HTMLButtonElement>('.settings-key-toggle').forEach((btn) => {
      btn.addEventListener('click', () => {
        const input = keyCardsEl.querySelector<HTMLInputElement>(
          `.settings-key-input[data-env="${btn.dataset.env}"]`,
        );
        if (!input) return;
        const show = input.type === 'password';
        input.type = show ? 'text' : 'password';
        btn.textContent = show ? '隐藏' : '显示';
      });
    });
    keyCardsEl.querySelectorAll<HTMLButtonElement>('.settings-key-test').forEach((btn) => {
      btn.addEventListener('click', () => {
        const key = keyCardsEl.querySelector<HTMLInputElement>(
          `.settings-key-input[data-env="${btn.dataset.env}"]`,
        )?.value.trim() || '';
        void testLlm(btn.dataset.provider || '', key || undefined, undefined);
      });
    });
    keyCardsEl.querySelectorAll<HTMLButtonElement>('.settings-key-clear').forEach((btn) => {
      btn.addEventListener('click', () => {
        const env = btn.dataset.env || '';
        const input = keyCardsEl.querySelector<HTMLInputElement>(`.settings-key-input[data-env="${env}"]`);
        if (input) {
          input.value = '';
          input.dataset.clear = '1';
        }
        const state = keyCardsEl.querySelector<HTMLElement>(`[data-keystate="${env}"]`);
        if (state) state.textContent = '将清除';
        setStatus(`「${env}」将在保存时清除`, 'info');
      });
    });
  };

  /* ================================================================== *
   * 形象模型区
   * ================================================================== */

  const renderModels = async (): Promise<void> => {
    modelListEl.innerHTML = '<div class="settings-tip">加载模型列表…</div>';
    try {
      const res = await fetch('/api/models');
      const data = (await res.json()) as { models?: Live2DModel[]; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '加载失败');
      modelListEl.innerHTML = '';
      for (const m of data.models || []) {
        const card = document.createElement('div');
        card.className = `settings-card settings-model-card${m.default ? ' selected' : ''}`;
        card.dataset.id = m.id;
        card.innerHTML = `
          <div class="settings-model-title">
            <span>${esc(m.name || m.id)}</span>
            ${m.default ? '<span class="settings-badge ok">当前</span>' : ''}
          </div>
          <div class="settings-model-desc">${esc(m.description || m.id)}</div>
        `;
        card.addEventListener('click', () => {
          void selectModel(m.id);
        });
        modelListEl.appendChild(card);
      }
    } catch (err) {
      modelListEl.innerHTML = `<div class="settings-tip">加载模型失败: ${esc((err as Error).message)}</div>`;
    }
  };

  /* ================================================================== *
   * populate（从 config 填充所有表单）
   * ================================================================== */

  const renderPersona = (): void => {
    const p = currentConfig.persona || EMPTY_CONFIG.persona;
    overlay.querySelector<HTMLTextAreaElement>('.settings-persona-prompt')!.value = p.system_prompt || '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-name')!.value = p.name || '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-role')!.value = p.role || '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-model')!.value = p.model || '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-temperature')!.value = String(p.temperature ?? 0.8);
    overlay.querySelector<HTMLInputElement>('.settings-persona-max-tokens')!.value = p.max_tokens ? String(p.max_tokens) : '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-vision-model')!.value = p.vision_model || '';
    // 清空标记重置
    overlay.querySelectorAll<HTMLButtonElement>('.persona-clear').forEach((b) => {
      delete b.dataset.cleared;
      b.textContent = '清除';
    });
    const clearAllBtn = overlay.querySelector<HTMLButtonElement>('.settings-persona-clear-all');
    if (clearAllBtn) {
      delete clearAllBtn.dataset.cleared;
      clearAllBtn.textContent = '清除全部人设字段';
    }
  };

  const renderSystem = (): void => {
    overlay.querySelector<HTMLInputElement>('.settings-character-name')!.value = currentConfig.character_name || '';
    overlay.querySelector<HTMLInputElement>('.settings-port')!.value = String(currentConfig.port ?? 12393);
    overlay.querySelector<HTMLInputElement>('.settings-fps')!.value = String(currentConfig.live2d_fps ?? 120);
    const logSel = overlay.querySelector<HTMLSelectElement>('.settings-log-level')!;
    const levels = ['INFO', 'DEBUG', 'WARNING', 'ERROR', 'CRITICAL'];
    if (!levels.includes(currentConfig.log_level || 'INFO')) levels.push(currentConfig.log_level || 'INFO');
    fillSelect(logSel, levels, currentConfig.log_level || 'INFO');
    logLevelChanged = false; // A1：重渲染后视为未改动
  };

  const populate = (cfg: AppConfig): void => {
    currentConfig = cfg;
    fillSelect(
      overlay.querySelector<HTMLSelectElement>('.settings-llm')!,
      cfg.llm_providers || [],
      cfg.llm_provider || '',
      LLM_PROVIDER_LABELS,
    );
    fillSelect(
      overlay.querySelector<HTMLSelectElement>('.settings-tts')!,
      cfg.tts_providers || [],
      cfg.tts_model || '',
      TTS_PROVIDER_LABELS,
    );
    const asrOptions =
      cfg.asr_providers && cfg.asr_providers.length
        ? cfg.asr_providers
        : ['faster_whisper', 'disabled'];
    fillSelect(overlay.querySelector<HTMLSelectElement>('.settings-asr')!, asrOptions, cfg.asr_model || 'disabled');
    overlay.querySelector<HTMLSelectElement>('.settings-viseme-mode')!.value = cfg.viseme_mode || 'rms';
    renderLlmCards();
    renderTtsCards();
    renderKeyCards();
    renderPersona();
    renderSystem();
    updateNavStatus();
    setErrorBar('');
  };

  const open = async (): Promise<void> => {
    overlay.style.display = 'flex';
    setStatus('加载中…', 'info');
    setSaveButtonsEnabled(false);
    loaded = false;
    try {
      const res = await fetch('/api/config');
      const data = (await res.json()) as AppConfig & { error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '加载失败');
      populate(data);
      const treeRes = await fetch('/api/config/tree');
      const treeData = (await treeRes.json()) as { yaml?: string; error?: string };
      if (treeRes.ok && treeData.yaml) {
        configTreeEl.value = treeData.yaml;
      }
      loaded = true;
      setSaveButtonsEnabled(true);
      // 打开面板异步加载「形象/Live2D」模型列表（不阻塞其它 tab；失败仅反映到列表区域）
      void renderModels();
      setStatus(`已加载 ${data.llm_providers?.length || 0} 个 LLM、${data.tts_providers?.length || 0} 个 TTS`, 'ok');
    } catch (err) {
      loaded = false;
      setSaveButtonsEnabled(false);
      setStatus(`加载失败: ${(err as Error).message}`, 'error');
    }
  };

  /* ================================================================== *
   * 读取 UI 状态快照 → 纯逻辑构建 payload
   * ================================================================== */

  const readUiState = (): SettingsUiState => {
    const llmProvider = overlay.querySelector<HTMLSelectElement>('.settings-llm')!.value;
    const ttsModel = overlay.querySelector<HTMLSelectElement>('.settings-tts')!.value;

    const llmConfigs: LlmProviderUi[] = [];
    llmCardsEl.querySelectorAll<HTMLElement>('.settings-provider-editor[data-type="llm"]').forEach((card) => {
      const id = card.dataset.provider || '';
      const cfg = currentConfig.llm_configs.find((c) => c.id === id);
      const temperature = Number(card.querySelector<HTMLInputElement>('.llm-temperature')?.value || '');
      const maxTokensRaw = card.querySelector<HTMLInputElement>('.llm-max-tokens')?.value || '';
      llmConfigs.push({
        id,
        name: cfg?.name,
        model: card.querySelector<HTMLSelectElement>('.llm-model')?.value || '',
        models: Array.from(card.querySelectorAll<HTMLOptionElement>('.llm-model option')).map((o) => o.value).filter(Boolean),
        base_url: card.querySelector<HTMLInputElement>('.llm-base-url')?.value || '',
        temperature: temperature && Number.isFinite(temperature) ? temperature : null,
        max_tokens: maxTokensRaw ? Number(maxTokensRaw) || null : null,
        key_env: card.dataset.keyEnv || cfg?.key_env || null,
        has_key: !!cfg?.has_key,
        configured: card.dataset.configured === '1',
        touched: card.dataset.touched === '1',
      });
    });

    const ttsConfigs: TtsProviderUi[] = [];
    ttsCardsEl.querySelectorAll<HTMLElement>('.settings-provider-editor[data-type="tts"]').forEach((card) => {
      const id = card.dataset.provider || '';
      const cfg = currentConfig.tts_configs.find((c) => c.id === id);
      const voices = Array.from(card.querySelectorAll<HTMLOptionElement>('.tts-voice option')).map((o) => o.value).filter(Boolean);
      const models = Array.from(card.querySelectorAll<HTMLOptionElement>('.tts-model option')).map((o) => o.value).filter(Boolean);
      ttsConfigs.push({
        id,
        name: cfg?.name,
        voice: card.querySelector<HTMLSelectElement>('.tts-voice')?.value || '',
        model: card.querySelector<HTMLSelectElement>('.tts-model')?.value || '',
        ws_url: card.querySelector<HTMLInputElement>('.tts-ws-url')?.value || '',
        voices,
        models,
        key_env: card.dataset.keyEnv || cfg?.key_env || null,
        has_key: !!cfg?.has_key,
        configured: card.dataset.configured === '1',
        touched: card.dataset.touched === '1',
      });
    });

    const apiKeys: SettingsUiState['api_keys'] = [];
    keyCardsEl.querySelectorAll<HTMLInputElement>('.settings-key-input').forEach((input) => {
      const env = input.dataset.env || '';
      const v = input.value.trim();
      const clear = input.dataset.clear === '1';
      if (v || clear) apiKeys.push({ env, value: v, clear });
    });

    const temperature = Number(overlay.querySelector<HTMLInputElement>('.settings-persona-temperature')?.value || '');
    const maxTokensRaw = overlay.querySelector<HTMLInputElement>('.settings-persona-max-tokens')?.value || '';
    const clearMap: Record<string, boolean> = {};
    overlay.querySelectorAll<HTMLButtonElement>('.persona-clear').forEach((b) => {
      if (b.dataset.cleared === '1') clearMap[b.dataset.field || ''] = true;
    });
    const clearAll = overlay.querySelector<HTMLButtonElement>('.settings-persona-clear-all');
    if (clearAll?.dataset.cleared === '1') {
      for (const f of ['name', 'role', 'model', 'vision_model', 'system_prompt']) clearMap[f] = true;
    }

    return {
      llm_provider: llmProvider,
      tts_model: ttsModel,
      viseme_mode: overlay.querySelector<HTMLSelectElement>('.settings-viseme-mode')!.value,
      asr_model: overlay.querySelector<HTMLSelectElement>('.settings-asr')!.value,
      llm_configs: llmConfigs,
      tts_configs: ttsConfigs,
      api_keys: apiKeys,
      persona: {
        name: overlay.querySelector<HTMLInputElement>('.settings-persona-name')!.value.trim(),
        role: overlay.querySelector<HTMLInputElement>('.settings-persona-role')!.value.trim(),
        model: overlay.querySelector<HTMLInputElement>('.settings-persona-model')!.value.trim(),
        temperature: temperature && Number.isFinite(temperature) ? temperature : null,
        max_tokens: maxTokensRaw ? Number(maxTokensRaw) || null : null,
        vision_model: overlay.querySelector<HTMLInputElement>('.settings-persona-vision-model')!.value.trim(),
        system_prompt: overlay.querySelector<HTMLTextAreaElement>('.settings-persona-prompt')!.value,
        clear: clearMap,
      },
      system: {
        character_name: overlay.querySelector<HTMLInputElement>('.settings-character-name')!.value.trim(),
        port: Number(overlay.querySelector<HTMLInputElement>('.settings-port')!.value),
        live2d_fps: Number(overlay.querySelector<HTMLInputElement>('.settings-fps')!.value),
        log_level: overlay.querySelector<HTMLSelectElement>('.settings-log-level')!.value,
        log_level_changed: logLevelChanged,
      },
    };
  };

  /* ================================================================== *
   * Reveal previous persona values from saved config for test (saved∪UI)
   * ================================================================== */

  const llmProviderConfig = (id: string): LlmProviderCfg | undefined =>
    currentConfig.llm_configs.find((c) => c.id === id);

  /* ================================================================== *
   * 保存（POST /api/config）
   * ================================================================== */

  const collectAndValidate = (): { payload?: Record<string, unknown>; error?: string } => {
    // 端口校验（A2/A7：POST 前 checkValidity，挡 port:0）
    const portRaw = overlay.querySelector<HTMLInputElement>('.settings-port')?.value ?? '';
    const portCheck = checkPort(portRaw);
    const portRow = overlay.querySelector<HTMLElement>('[data-portrow]');
    const portMsg = portRow?.querySelector<HTMLElement>('.field-error-msg');
    if (!portCheck.valid) {
      if (portRow) portRow.classList.add('invalid');
      if (portMsg && portCheck.message) portMsg.textContent = portCheck.message;
      return { error: `端口无效: ${portCheck.message}` };
    }
    if (portRow) portRow.classList.remove('invalid');
    return { payload: collectPayload(readUiState()) };
  };

  const save = (apply: boolean): void => {
    setErrorBar('');
    const validated = collectAndValidate();
    if (validated.error) {
      setStatus(validated.error, 'error');
      return;
    }
    const payload = validated.payload || {};
    (payload as Record<string, unknown>).apply = apply;
    // busy + 请求 token 竞态防护（A2/A6）：在途禁用两个保存按钮，只发一次 POST；
    // requestToken 保证「最后一次点击胜出」。
    const token = ++requestToken;
    const prevLabel = saveBtn.dataset.label || saveBtn.textContent || '';
    saveBtn.dataset.label = prevLabel;
    saveBtn.disabled = true;
    applyBtn.disabled = true;
    saveBtn.innerHTML = `<span class="spinner"></span>${apply ? '保存并应用中…' : '保存中…'}`;
    if (token === requestToken) setStatus(apply ? '保存并应用中…' : '保存中…', 'info');

    (async () => {
      try {
        const res = await fetch('/api/config', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(payload),
        });
        const data = (await res.json()) as SaveResponse;
        if (!res.ok) {
          // 校验失败：field_errors → 字段级红框 + 顶部错误条；否则状态条
          const fieldErrors = data.field_errors;
          if (fieldErrors && Object.keys(fieldErrors).length) {
            applyFieldErrors(fieldErrors);
            if (token === requestToken) setErrorBar(data.error || '配置校验失败');
          } else {
            // 400 也可能是显式校验错误无 field_errors
            if (data.error) {
              if (token === requestToken) setStatus(data.error, 'error');
            } else {
              if (token === requestToken) setStatus('保存失败（HTTP ' + res.status + '）', 'error');
            }
          }
          return;
        }
        if (data.error) throw new Error(data.error);
        // 成功 → re-populate（P1-1：UI = 磁盘 = 运行时）
        if (data.config) populate(data.config);
        if (token === requestToken) setErrorBar('');
        if (token === requestToken) {
          if (data.requires_restart) {
            setStatus('已保存，重启/重连后生效', 'ok');
          } else if (data.applied) {
            setStatus('已保存并应用', 'ok');
          } else {
            setStatus('已保存', 'ok');
          }
        }
        opts.onChanged?.();
      } catch (err) {
        if (token === requestToken) setStatus(`保存失败: ${(err as Error).message}`, 'error');
      } finally {
        // 无条件恢复按钮（不依赖 token），避免保存按钮卡死
        saveBtn.disabled = !loaded;
        applyBtn.disabled = !loaded;
        saveBtn.innerHTML = prevLabel;
      }
    })();
  };

  /* 把后端 field_errors 映射到字段级红框 */
  const applyFieldErrors = (errors: Record<string, string>): void => {
    overlay.querySelectorAll('.settings-row.invalid').forEach((r) => r.classList.remove('invalid'));
    const map: Record<string, string> = {
      port: '[data-portrow]',
      character_name: '.settings-character-name',
      'persona.name': '.settings-persona-name',
      'persona.role': '.settings-persona-role',
      'persona.model': '.settings-persona-model',
      'persona.system_prompt': '.settings-persona-prompt',
      'persona.vision_model': '.settings-persona-vision-model',
    };
    for (const [field, msg] of Object.entries(errors)) {
      const sel = map[field] || (map[field.split('.')[0]] ?? '');
      const target = sel ? overlay.querySelector<HTMLElement>(sel) : null;
      if (target) {
        let row = target.closest<HTMLElement>('.settings-row');
        if (!row) row = target;
        row.classList.add('invalid');
        const msgEl = row.querySelector<HTMLElement>('.field-error-msg');
        if (msgEl) msgEl.textContent = msg;
      }
    }
  };

  /* ---- 设定树保存 ---- */
  const saveTree = async (): Promise<void> => {
    withBusy(overlay.querySelector<HTMLButtonElement>('.settings-save-tree'), async (token) => {
      setStatus('保存设定树中…', 'info');
      const res = await fetch('/api/config/tree', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ yaml: configTreeEl.value }),
      });
      const data = (await res.json()) as { ok?: boolean; error?: string };
      if (!res.ok || data.error) {
        if (token === requestToken) setStatus(`保存失败: ${data.error || 'HTTP ' + res.status}`, 'error');
        return;
      }
      if (token === requestToken) setStatus('设定树已保存，重启后生效', 'ok');
    });
  };

  /* ================================================================== *
   * discover / test / check / scan / select（带 busy + last-click-wins）
   * ================================================================== */

  const discoverModels = async (provider: string, card: HTMLElement): Promise<void> => {
    const btn = card.querySelector<HTMLButtonElement>('.settings-discover-models');
    const help = card.querySelector<HTMLElement>('[data-llmhelp]');
    const cfg = llmProviderConfig(provider);
    const baseUrl = card.querySelector<HTMLInputElement>('.llm-base-url')?.value.trim() || '';
    const keyEnv = card.dataset.keyEnv || '';
    const keyInput = keyEnv ? keyCardsEl.querySelector<HTMLInputElement>(`.settings-key-input[data-env="${keyEnv}"]`) : null;
    const apiKey = keyInput?.value.trim() || '';
    // 结果缓存：相同 provider+base_url+api_key 命中则直接回填
    // （键含 api_key：key 更换后不命中旧缓存，避免缓存污染）
    const cacheKey = `${provider}|${baseUrl}|${apiKey}`;
    const cached = discoverCache.get(cacheKey);
    const useCache = cached && cached.length > 0;
    const requestFn = async (token: number): Promise<void> => {
      if (help) help.textContent = '获取模型…';
      let models: string[] = [];
      if (useCache) {
        models = cached;
      } else {
        const res = await fetch('/api/llm/discover-models', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ provider, base_url: baseUrl, api_key: apiKey, model: cfg?.model || '' }),
        });
        const data = (await res.json()) as { ok?: boolean; models?: string[]; error?: string };
        if (!res.ok || data.error) throw new Error(data.error || '获取失败');
        models = data.models || [];
        discoverCache.set(cacheKey, models);
      }
      if (token !== requestToken) return;
      fillModelSelect(card.querySelector<HTMLSelectElement>('.llm-model')!, card.querySelector<HTMLSelectElement>('.llm-model')?.value || (models[0] || ''), models);
      if (help) help.textContent = `已获取 ${models.length} 个模型（已缓存）`;
      setStatus(`「${provider}」获取到 ${models.length} 个模型`, 'ok');
      if (!cfg?.configured) card.dataset.touched = '1';
    };
    withBusy(btn, requestFn);
  };

  const testLlm = async (provider: string, explicitKey?: string, card?: HTMLElement): Promise<void> => {
    const btn = card
      ? card.querySelector<HTMLButtonElement>('.settings-test-llm')
      : keyCardsEl.querySelector<HTMLButtonElement>(`.settings-key-test[data-provider="${provider}"]`);
    const cfg = llmProviderConfig(provider);
    const requestFn = async (token: number): Promise<void> => {
      const body: Record<string, unknown> = { provider };
      // saved ∪ UI（P1-3）：带上当前 UI 未保存值
      if (explicitKey) body.api_key = explicitKey;
      if (card) {
        const baseUrl = card.querySelector<HTMLInputElement>('.llm-base-url')?.value.trim() || '';
        const model = card.querySelector<HTMLSelectElement>('.llm-model')?.value || '';
        const maxTokens = Number(card.querySelector<HTMLInputElement>('.llm-max-tokens')?.value || '') || undefined;
        const keyEnv = card.dataset.keyEnv || '';
        const keyInput = keyEnv ? keyCardsEl.querySelector<HTMLInputElement>(`.settings-key-input[data-env="${keyEnv}"]`) : null;
        const cardKey = keyInput?.value.trim() || '';
        if (baseUrl) body.base_url = baseUrl;
        if (model) body.model = model;
        if (maxTokens) body.max_tokens = maxTokens;
        if (cardKey && !explicitKey) body.api_key = cardKey;
      } else {
        // 来自 Keys 页：带上 saved 的 base_url/model（saved ∪ UI）
        if (cfg?.base_url) body.base_url = cfg.base_url;
        if (cfg?.model) body.model = cfg.model;
      }
      const res = await fetch('/api/test-llm', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      const data = (await res.json()) as { ok?: boolean; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '测试失败');
      if (token === requestToken) setStatus(`「${provider}」连接正常`, 'ok');
    };
    withBusy(btn, requestFn);
  };

  const testTts = async (card: HTMLElement): Promise<void> => {
    const btn = card.querySelector<HTMLButtonElement>('.settings-test-tts');
    const requestFn = async (token: number): Promise<void> => {
      const provider = card.dataset.provider || '';
      const body: Record<string, unknown> = {};
      if (provider) body.provider = provider;
      // saved ∪ UI
      const voice = card.querySelector<HTMLSelectElement>('.tts-voice')?.value || '';
      const model = card.querySelector<HTMLSelectElement>('.tts-model')?.value || '';
      const wsUrl = card.querySelector<HTMLInputElement>('.tts-ws-url')?.value.trim() || '';
      const keyEnv = card.dataset.keyEnv || '';
      const keyInput = keyEnv ? keyCardsEl.querySelector<HTMLInputElement>(`.settings-key-input[data-env="${keyEnv}"]`) : null;
      const apiKey = keyInput?.value.trim() || '';
      if (voice) body.voice = voice;
      if (model) body.model = model;
      if (wsUrl) body.ws_url = wsUrl;
      if (apiKey) body.api_key = apiKey;
      const res = await fetch('/api/test-tts', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      const data = (await res.json()) as { ok?: boolean; audio_path?: string; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '试听失败');
      if (token !== requestToken) return;
      if (data.audio_path) {
        playAudio(data.audio_path);
        setStatus('🔊 正在播放试听…', 'ok');
      } else {
        // 诚实提示：仅检查 Key 是否已配置
        setStatus('已确认 Key 已配置（此接口仅检查 Key，不真实合成）', 'ok');
      }
    };
    withBusy(btn, requestFn);
  };

  const playAudio = (audioPath: string): void => {
    const clean = audioPath.replace(/\\/g, '/');
    // 统一后端相对路径约定（A11）
    const audioUrl = /^https?:\/\//.test(clean)
      ? clean
      : clean.startsWith('/')
        ? clean
        : `/${clean}`;
    const audio = new Audio(audioUrl);
    audio.play().catch((err) => {
      setStatus(`无法播放试听（浏览器已拦截自动播放）: ${(err as Error)?.message || 'Audio.play rejected'}`, 'error');
    });
  };

  const checkAll = async (): Promise<void> => {
    const btn = overlay.querySelector<HTMLButtonElement>('.settings-check-llm');
    const requestFn = async (token: number): Promise<void> => {
      setStatus('检查所有 Provider 可用性…', 'info');
      const res = await fetch('/api/providers/check', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(collectPayload(readUiState())),
      });
      const data = (await res.json()) as {
        llm?: Array<{ provider: string; ok: boolean; status: string; models?: string[] }>;
        tts?: Array<{ provider: string; ok: boolean; status: string }>;
        error?: string;
      };
      if (!res.ok || data.error) throw new Error(data.error || '检查失败');
      if (token !== requestToken) return;
      const llm = data.llm || [];
      const tts = data.tts || [];
      const llmOk = llm.filter((x) => x.ok).length;
      const ttsOk = tts.filter((x) => x.ok).length;
      const lines: string[] = [
        `LLM ${llmOk}/${llm.length} 可用`,
        `TTS ${ttsOk}/${tts.length} 已配置（仅查 Key）`,
      ];
      const failed = llm.filter((x) => !x.ok);
      if (failed.length > 0) lines.push('未通过: ' + failed.map((x) => `${x.provider}(${x.status})`).join(', '));
      setStatus(lines.join(' · '), llmOk === llm.length && ttsOk === tts.length ? 'ok' : 'info');
    };
    withBusy(btn, requestFn);
  };

  const scanModels = async (): Promise<void> => {
    const btn = overlay.querySelector<HTMLButtonElement>('.settings-scan-models');
    const requestFn = async (token: number): Promise<void> => {
      modelStatusEl.textContent = '扫描中…';
      const res = await fetch('/api/models/scan', { method: 'POST' });
      const data = (await res.json()) as { ok?: boolean; added?: string[]; models?: Live2DModel[]; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '扫描失败');
      if (token !== requestToken) return;
      const added = data.added || [];
      modelStatusEl.textContent = added.length > 0 ? `已导入: ${added.join(', ')}` : '扫描完成，没有发现新模型';
      await renderModels();
    };
    withBusy(btn, requestFn);
  };

  const selectModel = async (modelId: string): Promise<void> => {
    modelStatusEl.textContent = '切换中…';
    try {
      const res = await fetch('/api/models/select', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ model_id: modelId }),
      });
      const data = (await res.json()) as { ok?: boolean; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '切换失败');
      modelStatusEl.textContent = '已切换，重启后生效';
      await renderModels(); // 刷新选中态（A11/A9）
    } catch (err) {
      modelStatusEl.textContent = `切换失败: ${(err as Error).message}`;
    }
  };

  /* ================================================================== *
   * 事件接线
   * ================================================================== */

  overlay.querySelector<HTMLButtonElement>('.settings-close')!.addEventListener('click', close);
  overlay.querySelector<HTMLButtonElement>('.settings-close2')!.addEventListener('click', close);
  saveBtn.addEventListener('click', () => { void save(false); });
  applyBtn.addEventListener('click', () => { void save(true); });
  overlay.querySelector<HTMLButtonElement>('.settings-save-tree')!.addEventListener('click', () => { void saveTree(); });
  overlay.querySelector<HTMLButtonElement>('.settings-check-llm')!.addEventListener('click', () => { void checkAll(); });
  overlay.querySelector<HTMLButtonElement>('.settings-scan-models')!.addEventListener('click', () => { void scanModels(); });

  overlay.querySelector<HTMLSelectElement>('.settings-log-level')!.addEventListener('change', () => {
    logLevelChanged = true; // A1：仅用户改动才提交
  });

  // 人设字段「清除」按钮
  overlay.querySelectorAll<HTMLButtonElement>('.persona-clear').forEach((btn) => {
    btn.addEventListener('click', () => {
      const field = btn.dataset.field || '';
      const input = overlay.querySelector<HTMLInputElement>(`.settings-persona-${field}`);
      if (input) input.value = '';
      btn.dataset.cleared = '1';
      btn.textContent = '已标记清除';
    });
  });
  // 清除全部人设字段
  const clearAllBtn = overlay.querySelector<HTMLButtonElement>('.settings-persona-clear-all')!;
  clearAllBtn.addEventListener('click', () => {
    overlay.querySelector<HTMLTextAreaElement>('.settings-persona-prompt')!.value = '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-name')!.value = '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-role')!.value = '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-model')!.value = '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-vision-model')!.value = '';
    clearAllBtn.dataset.cleared = '1';
    setStatus('已标记清除全部人设字段，保存时生效', 'info');
  });

  // LLM/TTS 提供商下拉变化 → 标记对应 provider touched
  overlay.querySelector<HTMLSelectElement>('.settings-llm')!.addEventListener('change', (e) => {
    const id = (e.target as HTMLSelectElement).value;
    if (!id) return;
    const editor = llmCardsEl.querySelector<HTMLElement>(`.settings-provider-editor[data-provider="${id}"]`);
    if (editor && editor.dataset.configured !== '1') editor.dataset.touched = '1';
  });
  overlay.querySelector<HTMLSelectElement>('.settings-tts')!.addEventListener('change', (e) => {
    const id = (e.target as HTMLSelectElement).value;
    if (!id) return;
    const editor = ttsCardsEl.querySelector<HTMLElement>(`.settings-provider-editor[data-provider="${id}"]`);
    if (editor && editor.dataset.configured !== '1') editor.dataset.touched = '1';
  });

  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) close();
  });

  // 首次打开时预加载（main.ts 点击按钮再 open 也会触发 open；此处不自动打开）
  return {
    open: () => {
      void open();
    },
    close,
    destroy() {
      overlay.remove();
    },
  };
}

/**
 * settings-ui.ts — PC 端设置中心 UI（与聊天面板共用 theme.css 令牌的深色精练风）
 *
 * 布局：模态覆盖层 + 左侧导航 rail（7 分区）+ 右侧内容区（一次一个 section）+ 底部条。
 *   分区：对话/LLM · 语音/TTS · 形象/Live2D（含角色背景图） · 外部接入 · 密钥 · 人设/提示词 · 系统/高级
 *
 * 职责：只做 DOM 装配 + 事件接线；纯逻辑（collectPayload / 校验 / 选项构建 / touched）
 * 抽到 settings-logic.ts；背景图与外部接入两块自成模块
 * （background.ts / external-input.ts），本文件只负责挂载与回调转发。
 *
 * 对外契约：`initSettingsPanel(container, opts): SettingsPanel {open, close, destroy}`，
 * 与 main.ts 的用法保持不变（opts 新增两个可选回调）。
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
import { uploadModelZip } from './model-upload';
import { exportConfig, importConfigFile } from './config-transfer';
import {
  listVoices,
  uploadVoice,
  selectVoice,
  applyVoice,
  deleteVoice,
} from './voice-library';
import { initBackgroundSection, type BackgroundSection, type BackgroundState } from './background';
import {
  initExternalInputSection,
  type ExternalInputSection,
  type ExternalStatus,
} from './external-input';
import { icon } from './icons';
import { listPlugins, enablePlugin, disablePlugin, type PluginInfo } from './plugin-manager';
import {
  listMemoryFacts,
  addMemoryFact,
  deleteMemoryFact,
  clearMemoryFacts,
  listKnowledgeDocs,
  addKnowledgeDoc,
  deleteKnowledgeDoc,
  webSearch,
} from './memory-api';
import { toast } from './toast';
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
  top_p?: number | null;
  thinking_enabled?: boolean | null;
  thinking_effort?: string | null;
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
  /** Melo TTS（本地）专用字段。 */
  language?: string;
  device?: string;
  speaker?: string;
  speed?: number | null;
  sdp_ratio?: number | null;
  noise_scale?: number | null;
  noise_scale_w?: number | null;
  format?: string;
}

interface PersonaCfg {
  // 人设收敛：仅提示词（模型/温度等参数在 llm_configs 内）
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
  /** 打开面板；可指定直接落在哪个分区（首启引导跳转用） */
  open(tab?: string): void;
  close(): void;
  destroy(): void;
}

const LLM_PROVIDER_LABELS: Record<string, string> = {
  deepseek_llm: 'DeepSeek',
  zhipu_llm: '智谱 GLM',
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
  melo_tts: 'Melo TTS（本地）',
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
  { id: 'local', label: '本地 / 免费', providers: ['edge_tts', 'melo_tts', 'gpt_sovits_tts', 'piper_tts', 'sherpa_onnx_tts', 'pyttsx3_tts'] },
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
  opts: {
    onChanged?: () => void;
    /** 背景图选择变化 → 主画面立即换背景 */
    onBackgroundChanged?: (state: BackgroundState) => void;
    /** 外部接入开关变化 → 聊天面板头部胶囊同步 */
    onExternalGateChange?: (status: ExternalStatus) => void;
  } = {},
): SettingsPanel {
  const overlay = document.createElement('div');
  overlay.className = 'settings-overlay';
  overlay.setAttribute('role', 'dialog');
  overlay.setAttribute('aria-modal', 'true');
  overlay.setAttribute('aria-label', '设置中心');
  overlay.innerHTML = `
    <div class="settings-box">
      <div class="settings-header">
        <div class="settings-header-title">
          设置中心
          <small>Live2D-Ai PC</small>
        </div>
        <button class="settings-close ui-lift" type="button" title="关闭（Esc）" aria-label="关闭设置中心">${icon('close')}</button>
      </div>

      <div class="settings-layout">
        <nav class="settings-nav">
          <div class="settings-nav-group">配置</div>
          <button class="settings-tab active" type="button" data-tab="llm"><span class="tab-dot"></span>对话 / LLM</button>
          <button class="settings-tab" type="button" data-tab="tts"><span class="tab-dot"></span>语音 / TTS</button>
          <button class="settings-tab" type="button" data-tab="live2d"><span class="tab-dot"></span>形象 / 背景</button>
          <button class="settings-tab" type="button" data-tab="external"><span class="tab-dot"></span>外部接入</button>
          <button class="settings-tab" type="button" data-tab="keys"><span class="tab-dot"></span>密钥</button>
          <button class="settings-tab" type="button" data-tab="persona"><span class="tab-dot"></span>人设 / 提示词</button>
          <button class="settings-tab" type="button" data-tab="system"><span class="tab-dot"></span>系统 / 高级</button>
          <button class="settings-tab" type="button" data-tab="memory"><span class="tab-dot"></span>记忆</button>
          <button class="settings-tab" type="button" data-tab="plugins"><span class="tab-dot"></span>插件</button>
          <div class="settings-nav-status">
            <div class="sv-label">状态简报</div>
            <div class="sv-line"><span class="status-dot" data-nav="llm"></span><span data-navtext="llm">LLM –</span></div>
            <div class="sv-line"><span class="status-dot" data-nav="tts"></span><span data-navtext="tts">TTS –</span></div>
            <div class="sv-line"><span class="status-dot" data-nav="ext"></span><span data-navtext="ext">外部接入 –</span></div>
            <div class="sv-line"><span class="status-dot" data-nav="sys"></span><span data-navtext="sys">系统 –</span></div>
          </div>
        </nav>

        <div class="settings-content">
          <div class="settings-error-bar" data-errorbar></div>

          <section class="settings-tab-panel active" data-panel="llm">
            <div class="settings-section-title">对话 / LLM</div>
            <label class="settings-row"><span class="lbl">LLM 提供商</span><select class="settings-llm"></select></label>
            <label class="settings-row" title="开启后思考模式输出思维链；关闭则直接回答（延迟更低）">
              <span class="lbl">思考模式</span>
              <input class="settings-thinking-toggle" type="checkbox" />
            </label>
            <label class="settings-row" data-thinking-effort-row hidden><span class="lbl">思考强度</span>
              <select class="settings-thinking-effort">
                <option value="low">low（轻度）</option>
                <option value="high">high（增强）</option>
                <option value="max">max（深度）</option>
              </select>
            </label>
            <label class="settings-row" title="仅影响本机界面：是否在聊天气泡上方展示模型的思维链过程">
              <span class="lbl">对话内展示思考</span>
              <input class="settings-show-thinking" type="checkbox" />
            </label>
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

            <div class="settings-group-title">声音克隆库</div>
            <div class="settings-editor-actions">
              <button class="btn ghost voice-upload-btn" type="button">上传参考音频</button>
              <input class="voice-upload-input" type="file" accept="audio/*,.wav,.mp3,.m4a,.flac,.ogg" hidden />
              <span class="settings-tip">支持 wav / mp3 / m4a / flac / ogg</span>
            </div>
            <div class="settings-status-inline voice-status"></div>
            <div class="voice-list"></div>
          </section>

          <section class="settings-tab-panel" data-panel="live2d">
            <div class="settings-section-title">形象 / Live2D</div>
            <div class="settings-section-hint">选择当前使用的 Live2D 模型；扫描可导入本地模型。</div>
            <div class="settings-editor-actions">
              <button class="btn ghost settings-scan-models" type="button">扫描并导入本地模型</button>
              <button class="btn ghost settings-model-upload-btn" type="button">上传 Live2D 模型 (.zip)</button>
              <input class="settings-model-upload-input" type="file" accept=".zip" hidden />
            </div>
            <div class="settings-status-inline" data-modeluploadstatus></div>
            <div class="settings-status-inline" data-modelstatus></div>
            <div class="settings-model-list" data-modellist></div>

            <div class="settings-section-title">角色背景图</div>
            <div class="settings-section-hint">
              上传一张本地图片作为角色背景（铺在 Live2D 画布之下，立即生效）；选「无背景」回到透明桌宠形态。
            </div>
            <div data-bgsection></div>
          </section>

          <section class="settings-tab-panel" data-panel="external">
            <div class="settings-section-title">外部接入（仅电脑端）</div>
            <div class="settings-section-hint">
              开启后，外部程序可以把一段文字交给角色，走的是与聊天框输入**完全相同**的内部链路
              （同一人设、同一记忆、同一 TTS 与口型）。默认关闭，且默认只允许本机调用；随时可以「立即断开」。
            </div>
            <div data-externalsection></div>
          </section>

          <section class="settings-tab-panel" data-panel="keys">
            <div class="settings-section-title">密钥（写入 .env）</div>
            <div class="settings-section-hint">留空表示不修改该 Key；「清除」会显式删除已保存的 Key（置 null 发送）。</div>
            <div class="settings-key-cards"></div>
          </section>

          <section class="settings-tab-panel" data-panel="persona">
            <div class="settings-section-title">人设 / 提示词</div>
            <div class="settings-section-hint">
              人设只承载提示词本身；模型 / 温度 / top_p / 思考等参数在「LLM 设置」页调整。
            </div>
            <label class="settings-row"><span class="lbl">System Prompt</span>
              <textarea class="settings-persona-prompt" rows="14" spellcheck="false"></textarea>
            </label>
            <div class="settings-editor-actions">
              <button class="btn danger settings-persona-clear-all" type="button">清除 System Prompt</button>
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
              <div class="settings-row"><span class="lbl">前端版本</span><span class="settings-frontend-version">未知版本</span></div>
            </div>
            <div class="settings-tip">端口与日志在保存并重启后端后生效。</div>
            <div class="settings-group-title">配置导入 / 导出</div>
            <div class="settings-editor-actions">
              <button class="btn ghost settings-config-export-btn" type="button">导出配置 JSON</button>
              <button class="btn ghost settings-config-import-btn" type="button">导入配置 JSON</button>
              <input class="settings-config-import-input" type="file" accept="application/json,.json" hidden />
            </div>
            <div class="settings-status-inline" data-configtransferstatus></div>
            <div class="settings-group-title">高级：conf.yaml 设定树</div>
            <label class="settings-row"><span class="lbl">原始 YAML</span>
              <textarea class="settings-config-tree" rows="14" spellcheck="false"></textarea>
            </label>
            <div class="settings-editor-actions">
              <button class="btn settings-save-tree" type="button">保存设定树</button>
              <span class="settings-tip">保存前会做 schema 校验，非法则不会写盘。</span>
            </div>
          </section>

          <section class="settings-tab-panel" data-panel="memory">
            <div class="settings-section-title">记忆 / 知识库</div>
            <div class="settings-section-hint">「长期记忆」插件在对话中自动记录事实（称呼、偏好、约定）；知识库文档按相关性检索注入并标注来源；联网查询为真实搜索（无 Key 走 DuckDuckGo，配 TAVILY_API_KEY 走 Tavily）。改动即时生效。</div>
            <div class="settings-status-inline memory-status"></div>
            <div class="settings-group-title">长期记忆事实</div>
            <div class="settings-editor-actions memory-add-row">
              <input class="settings-input memory-fact-input" type="text" placeholder="手动记一条，例如：喜欢被叫小白" aria-label="新增记忆" />
              <button class="btn primary btn-xs memory-add" type="button">添加</button>
              <button class="btn ghost btn-xs memory-clear" type="button">清空全部</button>
            </div>
            <div class="memory-list"></div>
            <div class="settings-group-title">知识库文档（.md / .txt）</div>
            <div class="settings-editor-actions kb-add-row">
              <input class="settings-input kb-name-input" type="text" placeholder="文档名，例如：世界观设定" aria-label="知识文档名" />
              <button class="btn primary btn-xs kb-add" type="button">添加文档</button>
            </div>
            <textarea class="settings-config-tree kb-content-input" rows="4" spellcheck="false" placeholder="文档内容（Markdown 纯文本），对话中相关内容会被检索注入" aria-label="知识文档内容"></textarea>
            <div class="settings-status-inline kb-status"></div>
            <div class="kb-list"></div>
            <div class="settings-group-title">联网查询试一试</div>
            <div class="settings-editor-actions search-row">
              <input class="settings-input search-query-input" type="text" placeholder="输入关键词，验证联网检索是否可用" aria-label="联网查询关键词" />
              <button class="btn primary btn-xs search-run" type="button">搜索</button>
            </div>
            <div class="search-results"></div>
          </section>
          <section class="settings-tab-panel" data-panel="plugins">
            <div class="settings-section-title">插件管理</div>
            <div class="settings-section-hint">原生插件随应用内置，外部插件由用户安装；启用 / 停用即时生效，部分插件需重启后端。</div>
            <div class="settings-status-inline plugin-status"></div>
            <div class="plugin-list"></div>
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
  const modelUploadStatusEl = overlay.querySelector<HTMLElement>('[data-modeluploadstatus]')!;
  const configTransferStatusEl = overlay.querySelector<HTMLElement>('[data-configtransferstatus]')!;
  const configTreeEl = overlay.querySelector<HTMLTextAreaElement>('.settings-config-tree')!;
  const saveBtn = overlay.querySelector<HTMLButtonElement>('.settings-save')!;
  const applyBtn = overlay.querySelector<HTMLButtonElement>('.settings-apply')!;
  const voiceListEl = overlay.querySelector<HTMLElement>('.voice-list')!;
  const voiceStatusEl = overlay.querySelector<HTMLElement>('.voice-status')!;
  const voiceUploadInput = overlay.querySelector<HTMLInputElement>('.voice-upload-input')!;
  const voiceUploadBtn = overlay.querySelector<HTMLButtonElement>('.voice-upload-btn')!;
  // 作用域限定到插件面板：记忆/知识库列表曾误挂 .plugin-list 类，
  // querySelector 命中第一个导致插件卡片渲染进「记忆」页、插件页空白
  const pluginListEl = overlay.querySelector<HTMLElement>(
    '[data-panel="plugins"] .plugin-list',
  )!;
  const pluginStatusEl = overlay.querySelector<HTMLElement>('.plugin-status')!;
  const memoryListEl = overlay.querySelector<HTMLElement>('.memory-list')!;
  const memoryStatusEl = overlay.querySelector<HTMLElement>('.memory-status')!;
  const memoryInputEl = overlay.querySelector<HTMLInputElement>('.memory-fact-input')!;
  const kbListEl = overlay.querySelector<HTMLElement>('.kb-list')!;
  const kbStatusEl = overlay.querySelector<HTMLElement>('.kb-status')!;
  const kbNameEl = overlay.querySelector<HTMLInputElement>('.kb-name-input')!;
  const kbContentEl = overlay.querySelector<HTMLTextAreaElement>('.kb-content-input')!;
  const searchQueryEl = overlay.querySelector<HTMLInputElement>('.search-query-input')!;
  const frontendVersionEl = overlay.querySelector<HTMLElement>('.settings-frontend-version');

  /* ---- 运行态 ---- */
  let currentConfig: AppConfig = EMPTY_CONFIG;
  let loaded = false;
  let logLevelChanged = false;
  let requestToken = 0;
  let externalStatus: ExternalStatus | null = null;
  let pluginsLoaded = false;
  let memoryLoaded = false;
  // 子区块（背景图 / 外部接入）在下方 setStatus 就绪后装配
  let backgroundSection: BackgroundSection | null = null;
  let externalSection: ExternalInputSection | null = null;
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
    tab.addEventListener('click', () => {
      switchTab(tab.dataset.tab || 'llm');
      if (tab.dataset.tab === 'plugins' && !pluginsLoaded) void renderPlugins();
    if (tab.dataset.tab === 'memory' && !memoryLoaded) {
      void renderMemory();
      void renderKnowledge();
    }
    });
  });

  const close = (): void => {
    overlay.classList.remove('open');
    externalSection?.stop(); // 关闭面板即停掉外部接入状态轮询
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

  /* ---- 子区块装配（背景图 / 外部接入；各自独立模块，只回传状态） ---- */
  backgroundSection = initBackgroundSection(
    overlay.querySelector<HTMLElement>('[data-bgsection]')!,
    {
      onChanged: (state) => opts.onBackgroundChanged?.(state),
      onStatus: (msg, kind) => setStatus(msg, kind),
    },
  );
  externalSection = initExternalInputSection(
    overlay.querySelector<HTMLElement>('[data-externalsection]')!,
    {
      onStatus: (msg, kind) => setStatus(msg, kind),
      onGateChange: (status) => {
        externalStatus = status;
        updateNavStatus();
        opts.onExternalGateChange?.(status);
      },
    },
  );

  /* ---- nav 状态简报 ---- */
  const updateNavStatus = (): void => {
    const llmTotal = currentConfig.llm_configs.length;
    const llmOk = currentConfig.llm_configs.filter((c) => c.configured && c.has_key).length;
    const ttsTotal = currentConfig.tts_configs.length;
    const ttsOk = currentConfig.tts_configs.filter((c) => c.configured).length;
    const navLlmDot = overlay.querySelector<HTMLElement>('[data-nav="llm"]');
    const navTtsDot = overlay.querySelector<HTMLElement>('[data-nav="tts"]');
    const navExtDot = overlay.querySelector<HTMLElement>('[data-nav="ext"]');
    const navSysDot = overlay.querySelector<HTMLElement>('[data-nav="sys"]');
    const navLlm = overlay.querySelector<HTMLElement>('[data-navtext="llm"]');
    const navTts = overlay.querySelector<HTMLElement>('[data-navtext="tts"]');
    const navExt = overlay.querySelector<HTMLElement>('[data-navtext="ext"]');
    const navSys = overlay.querySelector<HTMLElement>('[data-navtext="sys"]');
    if (navLlmDot) navLlmDot.className = `status-dot ${llmTotal > 0 ? (llmOk > 0 ? 'ok' : 'warn') : 'bad'}`;
    if (navTtsDot) navTtsDot.className = `status-dot ${ttsTotal > 0 ? (ttsOk > 0 ? 'ok' : 'warn') : ''}`;
    if (navExtDot) {
      navExtDot.className = `status-dot ${externalStatus?.enabled ? 'ok' : ''}`;
    }
    if (navSysDot) navSysDot.className = `status-dot ${currentConfig.port ? 'ok' : 'bad'}`;
    if (navLlm) navLlm.textContent = `LLM ${llmOk}/${llmTotal} 可用`;
    if (navTts) navTts.textContent = `TTS ${ttsOk}/${ttsTotal} 已配置`;
    if (navExt) {
      navExt.textContent = externalStatus
        ? externalStatus.enabled
          ? `外部接入 开（${externalStatus.allow_remote ? '含局域网' : '仅本机'}）`
          : '外部接入 关'
        : '外部接入 –';
    }
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
    syncThinkingControls();
  };

  /* 全局思考开关 ↔ 当前选中 provider 配置；「对话内展示思考」为纯前端偏好 */
  const syncThinkingControls = (): void => {
    const activeId = overlay.querySelector<HTMLSelectElement>('.settings-llm')!.value;
    const cfg = currentConfig.llm_configs.find((c) => c.id === activeId);
    const toggle = overlay.querySelector<HTMLInputElement>('.settings-thinking-toggle')!;
    const effortSel = overlay.querySelector<HTMLSelectElement>('.settings-thinking-effort')!;
    const effortRow = overlay.querySelector<HTMLElement>('[data-thinking-effort-row]')!;
    toggle.checked = !!cfg?.thinking_enabled;
    effortSel.value = cfg?.thinking_effort || 'high';
    effortRow.hidden = !toggle.checked;
    const show = overlay.querySelector<HTMLInputElement>('.settings-show-thinking')!;
    show.checked = localStorage.getItem('showThinking') !== '0'; // 默认展示
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
        <div class="settings-row"><span class="lbl">Top P</span><input class="llm-top-p" type="number" step="0.05" min="0" max="1" placeholder="留空 = 提供商默认" /></div>
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
    editor.querySelector<HTMLInputElement>('.llm-top-p')!.value =
      p.top_p === null || p.top_p === undefined ? '' : String(p.top_p);

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
    const isMelo = p.id === 'melo_tts';
    const voiceLabel = isMelo ? '语言 Language' : '音色 Voice';
    const modelLabel = isMelo ? '设备 Device' : '模型';
    const wsRow = isMelo
      ? ''
      : '<div class="settings-row"><span class="lbl">WebSocket URL</span><input class="tts-ws-url" type="text" data-mono /></div>';
    const meloRows = isMelo
      ? `
        <div class="settings-row"><span class="lbl">说话人 Speaker（可选）</span><input class="tts-speaker" type="text" data-mono /></div>
        <div class="settings-row"><span class="lbl">语速 Speed（0.5~2.0）</span><input class="tts-speed" type="number" min="0.5" max="2" step="0.05" data-mono /></div>
        <div class="settings-row"><span class="lbl">韵律 sdp_ratio（0~1）</span><input class="tts-sdp" type="number" min="0" max="1" step="0.05" data-mono /></div>
        <div class="settings-row"><span class="lbl">音高随机 noise_scale（0~1）</span><input class="tts-noise" type="number" min="0" max="1" step="0.05" data-mono /></div>
        <div class="settings-row"><span class="lbl">时长随机 noise_scale_w（0~1）</span><input class="tts-noise-w" type="number" min="0" max="1" step="0.05" data-mono /></div>
      `
      : '';
    const helpText = isMelo
      ? '将用本地 Melo 真实合成试听，首次使用会下载模型。'
      : '仅检查 Key 是否已配置，不真实合成。';
    editor.innerHTML = `
      <div class="settings-edit-stack">
        <div class="settings-row"><span class="lbl">${voiceLabel}</span><select class="tts-voice" data-mono></select></div>
        <div class="settings-row"><span class="lbl">${modelLabel}</span><select class="tts-model" data-mono></select></div>
        ${wsRow}
        ${meloRows}
      </div>
      <div class="settings-editor-actions">
        <button class="btn ghost settings-test-tts" type="button">试听/检查</button>
        <span class="settings-tip settings-editor-help">${helpText}</span>
      </div>
    `;
    fillModelSelect(editor.querySelector<HTMLSelectElement>('.tts-voice')!, p.voice, p.voices);
    fillModelSelect(editor.querySelector<HTMLSelectElement>('.tts-model')!, p.model, p.models);
    if (!isMelo) {
      editor.querySelector<HTMLInputElement>('.tts-ws-url')!.value = p.ws_url || '';
    } else {
      editor.querySelector<HTMLInputElement>('.tts-speaker')!.value = p.speaker || '';
      editor.querySelector<HTMLInputElement>('.tts-speed')!.value = String(p.speed ?? 1.0);
      editor.querySelector<HTMLInputElement>('.tts-sdp')!.value = String(p.sdp_ratio ?? 0.2);
      editor.querySelector<HTMLInputElement>('.tts-noise')!.value = String(p.noise_scale ?? 0.6);
      editor.querySelector<HTMLInputElement>('.tts-noise-w')!.value = String(p.noise_scale_w ?? 0.8);
    }

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
    // 人设收敛：只渲染 System Prompt（其余参数归 LLM 设置）
    overlay.querySelector<HTMLTextAreaElement>('.settings-persona-prompt')!.value = p.system_prompt || '';
    const clearAllBtn = overlay.querySelector<HTMLButtonElement>('.settings-persona-clear-all');
    if (clearAllBtn) {
      delete clearAllBtn.dataset.cleared;
      clearAllBtn.textContent = '清除 System Prompt';
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
    overlay.classList.add('open');
    setStatus('加载中…', 'info');
    setSaveButtonsEnabled(false);
    loaded = false;
    // 与配置无关的两块（背景图 / 外部接入）独立加载，任一失败不影响其余分区
    void backgroundSection?.refresh();
    void externalSection?.refresh();
    pluginsLoaded = false;
    void renderPlugins(); // 插件列表独立加载，失败仅反映到插件分区 + toast
    void loadFrontendVersion(); // 前端版本独立加载，失败显示「未知版本」不影响其余 UI
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
    // 全局思考开关：作用于当前选中 provider（写入其配置项）
    const thinkingOn = overlay.querySelector<HTMLInputElement>('.settings-thinking-toggle')!.checked;
    const thinkingEffort = thinkingOn
      ? overlay.querySelector<HTMLSelectElement>('.settings-thinking-effort')!.value
      : null;
    llmCardsEl.querySelectorAll<HTMLElement>('.settings-provider-editor[data-type="llm"]').forEach((card) => {
      const id = card.dataset.provider || '';
      const cfg = currentConfig.llm_configs.find((c) => c.id === id);
      const temperature = Number(card.querySelector<HTMLInputElement>('.llm-temperature')?.value || '');
      const topPRaw = card.querySelector<HTMLInputElement>('.llm-top-p')?.value ?? '';
      const is_active = id === llmProvider;
      llmConfigs.push({
        id,
        name: cfg?.name,
        model: card.querySelector<HTMLSelectElement>('.llm-model')?.value || '',
        models: Array.from(card.querySelectorAll<HTMLOptionElement>('.llm-model option')).map((o) => o.value).filter(Boolean),
        base_url: card.querySelector<HTMLInputElement>('.llm-base-url')?.value || '',
        temperature: temperature && Number.isFinite(temperature) ? temperature : null,
        top_p: topPRaw && Number.isFinite(Number(topPRaw)) ? Number(topPRaw) : null,
        thinking_enabled: is_active ? thinkingOn : (cfg?.thinking_enabled ?? false),
        thinking_effort: is_active ? thinkingEffort : (cfg?.thinking_effort ?? null),
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
      const ttsEntry: TtsProviderUi = {
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
      };
      if (id === 'melo_tts') {
        const num = (selector: string, fallback: number | null | undefined): number | null => {
          const raw = card.querySelector<HTMLInputElement>(selector)?.value || '';
          const n = Number(raw);
          return raw && Number.isFinite(n) ? n : (fallback ?? null);
        };
        ttsEntry.language = ttsEntry.voice;
        ttsEntry.device = ttsEntry.model;
        ttsEntry.speaker = card.querySelector<HTMLInputElement>('.tts-speaker')?.value || '';
        ttsEntry.speed = num('.tts-speed', cfg?.speed ?? 1.0);
        ttsEntry.sdp_ratio = num('.tts-sdp', cfg?.sdp_ratio ?? 0.2);
        ttsEntry.noise_scale = num('.tts-noise', cfg?.noise_scale ?? 0.6);
        ttsEntry.noise_scale_w = num('.tts-noise-w', cfg?.noise_scale_w ?? 0.8);
        ttsEntry.format = 'wav';
      }
      ttsConfigs.push(ttsEntry);
    });

    const apiKeys: SettingsUiState['api_keys'] = [];
    keyCardsEl.querySelectorAll<HTMLInputElement>('.settings-key-input').forEach((input) => {
      const env = input.dataset.env || '';
      const v = input.value.trim();
      const clear = input.dataset.clear === '1';
      if (v || clear) apiKeys.push({ env, value: v, clear });
    });

    const clearMap: Record<string, boolean> = {};
    const clearAll = overlay.querySelector<HTMLButtonElement>('.settings-persona-clear-all');
    if (clearAll?.dataset.cleared === '1') {
      clearMap['system_prompt'] = true;
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
      'persona.system_prompt': '.settings-persona-prompt',
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
      if (provider === 'melo_tts') {
        const numberValue = (selector: string): number | null => {
          const raw = card.querySelector<HTMLInputElement>(selector)?.value || '';
          const n = Number(raw);
          return raw && Number.isFinite(n) ? n : null;
        };
        body.language = voice || 'ZH';
        body.device = model || 'cpu';
        const speaker = card.querySelector<HTMLInputElement>('.tts-speaker')?.value.trim() || '';
        if (speaker) body.speaker = speaker;
        const speed = numberValue('.tts-speed');
        if (speed !== null) body.speed = speed;
        const sdpRatio = numberValue('.tts-sdp');
        if (sdpRatio !== null) body.sdp_ratio = sdpRatio;
        const noiseScale = numberValue('.tts-noise');
        if (noiseScale !== null) body.noise_scale = noiseScale;
        const noiseScaleW = numberValue('.tts-noise-w');
        if (noiseScaleW !== null) body.noise_scale_w = noiseScaleW;
        body.format = 'wav';
      }
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
      const data = (await res.json()) as {
        ok?: boolean;
        applied?: boolean;
        requires_restart?: boolean;
        message?: string;
        error?: string;
      };
      if (!res.ok || data.error) throw new Error(data.error || '切换失败');
      // P1-4：后端已热推 set-model-and-conf → 主画面即时换形象，不再说"重启后生效"
      modelStatusEl.textContent =
        data.message || (data.requires_restart ? '已切换，重启后生效' : '已切换并即时生效');
      await renderModels(); // 刷新选中态（A11/A9）
    } catch (err) {
      modelStatusEl.textContent = `切换失败: ${(err as Error).message}`;
    }
  };

  /* ---- Live2D 模型 zip 上传（对接 POST /api/models/upload） ---- */
  const uploadModelHandler = (file: File): void => {
    const btn = overlay.querySelector<HTMLButtonElement>('.settings-model-upload-btn');
    const requestFn = async (token: number): Promise<void> => {
      modelUploadStatusEl.textContent = '上传中…';
      try {
        const result = await uploadModelZip(file);
        if (token !== requestToken) return;
        modelUploadStatusEl.textContent = `已导入 ${result.model_id}`;
        await renderModels(); // 复用现有模型列表刷新
      } catch (err) {
        if (token === requestToken) {
          modelUploadStatusEl.textContent = `上传失败: ${(err as Error).message}`;
        }
        throw err;
      }
    };
    withBusy(btn, requestFn);
  };

  /* ---- 配置导出 / 导入（对接 GET/POST /api/config/export · /api/config/import） ---- */
  const exportConfigHandler = (): void => {
    const btn = overlay.querySelector<HTMLButtonElement>('.settings-config-export-btn');
    const requestFn = async (token: number): Promise<void> => {
      configTransferStatusEl.textContent = '导出中…';
      try {
        const cfg = await exportConfig();
        if (token !== requestToken) return;
        const payload = JSON.stringify(
          { format: cfg.format, version: cfg.version, conf_yaml: cfg.conf_yaml, persona: cfg.persona },
          null,
          2,
        );
        const blob = new Blob([payload], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'live2dai-config.json';
        document.body.appendChild(a);
        a.click();
        a.remove();
        URL.revokeObjectURL(url);
        configTransferStatusEl.textContent = '已导出 live2dai-config.json';
      } catch (err) {
        if (token === requestToken) {
          configTransferStatusEl.textContent = `导出失败: ${(err as Error).message}`;
        }
        throw err;
      }
    };
    withBusy(btn, requestFn);
  };

  const importConfigHandler = (file: File): void => {
    const btn = overlay.querySelector<HTMLButtonElement>('.settings-config-import-btn');
    const requestFn = async (token: number): Promise<void> => {
      configTransferStatusEl.textContent = '导入中…';
      try {
        const result = await importConfigFile(file);
        if (token !== requestToken) return;
        configTransferStatusEl.textContent = `${result.message}（请重启后端以生效）`;
      } catch (err) {
        if (token === requestToken) {
          configTransferStatusEl.textContent = `导入失败: ${(err as Error).message}`;
        }
        throw err;
      }
    };
    withBusy(btn, requestFn);
  };

  /* ---- 插件管理（对接 /api/plugins） ---- */
  const STATUS_BADGES: Record<string, { label: string; cls: string }> = {
    ok: { label: '正常', cls: 'ok' },
    disabled: { label: '停用', cls: '' },
    error: { label: '异常', cls: 'warn' },
    gated: { label: '核心门控', cls: 'warn' },
    core: { label: '核心提供', cls: 'ok' },
    stub: { label: '未实现', cls: '' },
  };

  const statusBadgeHtml = (status: string): string => {
    const s = STATUS_BADGES[status] || { label: status || '未知', cls: '' };
    return `<span class="settings-badge ${s.cls}"><span class="status-dot"></span>${esc(s.label)}</span>`;
  };

  const buildPluginCard = (p: PluginInfo): HTMLElement => {
    const card = document.createElement('div');
    card.className = 'settings-card plugin-card';
    const originBadge =
      p.origin === 'native'
        ? '<span class="settings-badge">原生</span>'
        : '<span class="settings-badge testing">外部</span>';
    const implLabel: Record<string, string> = {
      runtime: '运行时',
      gated: '核心门控',
      core: '核心提供',
      stub: '未实现',
    };
    const implBadge = p.implementation
      ? `<span class="settings-badge">${esc(implLabel[p.implementation] || p.implementation)}</span>`
      : '';
    const enabledBadge = p.enabled
      ? '<span class="settings-badge ok"><span class="status-dot"></span>已启用</span>'
      : '<span class="settings-badge warn"><span class="status-dot"></span>已停用</span>';
    const hookPoints = (p.hook_points || []).map((h) => `<code>${esc(h)}</code>`).join(' ') || '—';
    const capabilities = (p.capabilities || []).map((c) => `<code>${esc(c)}</code>`).join(' ') || '—';
    const nameText = p.name_zh ? `${esc(p.name_zh)} · ${esc(p.name)}` : esc(p.name || p.id);
    const descText = p.description_zh
      ? `${esc(p.description_zh)}<br><span class="settings-mono">${esc(p.description || '')}</span>`
      : esc(p.description || '');
    card.innerHTML = `
      <div class="plugin-card-head">
        <div class="plugin-card-title">
          <span>${nameText}</span>
          <span class="settings-mono settings-provider-id">${esc(p.id)}</span>
          <span class="settings-mono">v${esc(p.version || '')}</span>
        </div>
        <div class="plugin-badges">${originBadge}${implBadge}${enabledBadge}${statusBadgeHtml(p.status)}</div>
      </div>
      <div class="plugin-card-desc">${descText}</div>
      <div class="plugin-card-meta"><span class="lbl">钩子</span><span>${hookPoints}</span></div>
      <div class="plugin-card-meta"><span class="lbl">能力</span><span>${capabilities}</span></div>
      <div class="settings-editor-actions">
        ${p.implementation === 'stub'
          ? '<span class="settings-mono">未实现，不可启用</span>'
          : (p.enabled
            ? `<button class="btn ghost btn-xs plugin-disable" type="button" data-id="${esc(p.id)}">停用</button>`
            : `<button class="btn primary btn-xs plugin-enable" type="button" data-id="${esc(p.id)}">启用</button>`)}
      </div>
    `;
    return card;
  };

  const togglePlugin = (id: string, enabled: boolean, btn: HTMLButtonElement): void => {
    const prevLabel = btn.textContent || '';
    btn.disabled = true;
    btn.innerHTML = `<span class="spinner"></span>${enabled ? '启用中…' : '停用中…'}`;
    (async () => {
      try {
        const plugin = enabled ? await enablePlugin(id) : await disablePlugin(id);
        await renderPlugins();
        pluginStatusEl.textContent = `已${enabled ? '启用' : '停用'}: ${plugin.name || plugin.id}`;
      } catch (err) {
        const msg = (err as Error).message;
        pluginStatusEl.textContent = `${enabled ? '启用' : '停用'}失败: ${msg}`;
        toast(`插件${enabled ? '启用' : '停用'}失败: ${msg}`, 'error');
      } finally {
        btn.disabled = false;
        btn.innerHTML = prevLabel;
      }
    })();
  };

  const loadFrontendVersion = async (): Promise<void> => {
    if (!frontendVersionEl) return;
    try {
      const res = await fetch('/api/frontend-version');
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = (await res.json()) as { ok?: boolean; version?: string; error?: string };
      if (data.error) throw new Error(String(data.error));
      frontendVersionEl.textContent = data.version || '未知版本';
    } catch {
      frontendVersionEl.textContent = '未知版本';
    }
  };

  /* ---- 记忆管理（对接 /api/memory/*，可视化长期记忆事实） ---- */
  const renderMemory = async (): Promise<void> => {
    memoryLoaded = true;
    memoryListEl.innerHTML = '<div class="settings-tip">加载记忆…</div>';
    memoryStatusEl.textContent = '';
    try {
      const facts = await listMemoryFacts();
      if (!facts.length) {
        memoryListEl.innerHTML =
          '<div class="settings-tip">还没有长期记忆——聊天时说「我叫… / 我喜欢… / 记住：…」就会自动记录。</div>';
        return;
      }
      memoryListEl.innerHTML = '';
      facts.forEach((fact, idx) => {
        const row = document.createElement('div');
        row.className = 'memory-row';
        row.innerHTML = `
          <span class="memory-idx settings-mono">#${idx + 1}</span>
          <span class="memory-fact">${esc(fact)}</span>
          <button class="btn ghost btn-xs memory-del" type="button" data-idx="${idx}" aria-label="删除这条记忆">删除</button>
        `;
        memoryListEl.appendChild(row);
      });
      memoryListEl.querySelectorAll<HTMLButtonElement>('.memory-del').forEach((btn) => {
        btn.addEventListener('click', () => {
          const idx = Number(btn.dataset.idx);
          btn.disabled = true;
          void (async () => {
            try {
              await deleteMemoryFact(idx);
              await renderMemory();
              memoryStatusEl.textContent = '已删除';
            } catch (err) {
              memoryStatusEl.textContent = `删除失败: ${(err as Error).message}`;
              toast(`记忆删除失败: ${(err as Error).message}`, 'error');
              btn.disabled = false;
            }
          })();
        });
      });
    } catch (err) {
      memoryLoaded = false; // 失败允许下次点 tab 重试（与插件分区同策略）
      const msg = (err as Error).message;
      memoryListEl.innerHTML = `<div class="settings-tip">加载记忆失败: ${esc(msg)}</div>`;
      toast(`记忆加载失败: ${msg}`, 'error');
    }
  };

  const addMemoryFromInput = (): void => {
    const fact = (memoryInputEl.value || '').trim();
    if (!fact) return;
    if (fact.length > 200) {
      toast('记忆过长（不超过 200 字）', 'error');
      return;
    }
    void (async () => {
      try {
        await addMemoryFact(fact);
        memoryInputEl.value = '';
        await renderMemory();
        memoryStatusEl.textContent = '已添加';
      } catch (err) {
        memoryStatusEl.textContent = `添加失败: ${(err as Error).message}`;
        toast(`记忆添加失败: ${(err as Error).message}`, 'error');
      }
    })();
  };
  overlay.querySelector<HTMLButtonElement>('.memory-add')?.addEventListener('click', addMemoryFromInput);
  memoryInputEl.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') addMemoryFromInput();
  });

  const renderKnowledge = async (): Promise<void> => {
    kbListEl.innerHTML = '<div class="settings-tip">加载知识文档…</div>';
    kbStatusEl.textContent = '';
    try {
      const docs = await listKnowledgeDocs();
      if (!docs.length) {
        kbListEl.innerHTML =
          '<div class="settings-tip">还没有知识文档——添加后，对话中相关内容会自动检索注入并标注来源。</div>';
        return;
      }
      kbListEl.innerHTML = '';
      for (const d of docs) {
        const row = document.createElement('div');
        row.className = 'memory-row';
        const sizeText = typeof d.size === 'number' ? ` · ${d.size} 字` : '';
        row.innerHTML = `
          <span class="memory-idx settings-mono">📄</span>
          <span class="memory-fact">${esc(d.name)}<span class="settings-mono">${esc(sizeText)}</span></span>
          <button class="btn ghost btn-xs kb-del" type="button" data-name="${esc(d.name)}" aria-label="删除知识文档">删除</button>
        `;
        kbListEl.appendChild(row);
      }
      kbListEl.querySelectorAll<HTMLButtonElement>('.kb-del').forEach((btn) => {
        btn.addEventListener('click', () => {
          const name = btn.dataset.name || '';
          btn.disabled = true;
          void (async () => {
            try {
              await deleteKnowledgeDoc(name);
              await renderKnowledge();
              kbStatusEl.textContent = '已删除';
            } catch (err) {
              kbStatusEl.textContent = `删除失败: ${(err as Error).message}`;
              toast(`知识文档删除失败: ${(err as Error).message}`, 'error');
              btn.disabled = false;
            }
          })();
        });
      });
    } catch (err) {
      const msg = (err as Error).message;
      kbListEl.innerHTML = `<div class="settings-tip">加载知识文档失败: ${esc(msg)}</div>`;
    }
  };

  const addKnowledgeFromInputs = (): void => {
    const name = (kbNameEl.value || '').trim();
    const content = (kbContentEl.value || '').trim();
    if (!name || !content) {
      toast('文档名和内容都要填', 'error');
      return;
    }
    void (async () => {
      try {
        const saved = await addKnowledgeDoc(name, content);
        kbNameEl.value = '';
        kbContentEl.value = '';
        await renderKnowledge();
        kbStatusEl.textContent = `已保存: ${saved}`;
      } catch (err) {
        kbStatusEl.textContent = `保存失败: ${(err as Error).message}`;
        toast(`知识文档保存失败: ${(err as Error).message}`, 'error');
      }
    })();
  };
  overlay.querySelector<HTMLButtonElement>('.kb-add')?.addEventListener('click', addKnowledgeFromInputs);

  const runSearch = (): void => {
    const query = (searchQueryEl.value || '').trim();
    if (!query) return;
    const resultsEl = overlay.querySelector<HTMLElement>('.search-results')!;
    resultsEl.innerHTML = '<div class="settings-tip">搜索中…</div>';
    void (async () => {
      try {
        const { results, provider, error } = await webSearch(query);
        if (error) throw new Error(error);
        if (!results.length) {
          resultsEl.innerHTML = '<div class="settings-tip">没有结果（或网络不可达）</div>';
          return;
        }
        resultsEl.innerHTML = '';
        for (const r of results.slice(0, 5)) {
          const row = document.createElement('div');
          row.className = 'memory-row';
          row.innerHTML = `
            <span class="memory-idx settings-mono">🔗</span>
            <span class="memory-fact"><a href="${esc(r.url || '#')}" target="_blank" rel="noreferrer">${esc(r.title || r.url || '结果')}</a><br><span class="settings-mono">${esc((r.snippet || '').slice(0, 140))}</span></span>
          `;
          resultsEl.appendChild(row);
        }
        if (provider) {
          const tag = document.createElement('div');
          tag.className = 'settings-tip';
          tag.textContent = `来源: ${provider}`;
          resultsEl.appendChild(tag);
        }
      } catch (err) {
        resultsEl.innerHTML = `<div class="settings-tip">搜索失败: ${esc((err as Error).message)}</div>`;
      }
    })();
  };
  overlay.querySelector<HTMLButtonElement>('.search-run')?.addEventListener('click', runSearch);
  searchQueryEl.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') runSearch();
  });
  overlay.querySelector<HTMLButtonElement>('.memory-clear')?.addEventListener('click', () => {
    void (async () => {
      try {
        const n = await clearMemoryFacts();
        await renderMemory();
        memoryStatusEl.textContent = n > 0 ? `已清空 ${n} 条` : '当前没有记忆';
      } catch (err) {
        memoryStatusEl.textContent = `清空失败: ${(err as Error).message}`;
        toast(`记忆清空失败: ${(err as Error).message}`, 'error');
      }
    })();
  });
  const renderPlugins = async (): Promise<void> => {
    pluginsLoaded = true;
    pluginListEl.innerHTML = '<div class="settings-tip">加载插件…</div>';
    pluginStatusEl.textContent = '';
    try {
      const plugins = await listPlugins();
      if (!plugins.length) {
        pluginListEl.innerHTML = '<div class="settings-tip">暂无插件</div>';
        return;
      }
      pluginListEl.innerHTML = '';
      const groups = [
        { label: '原生插件', items: plugins.filter((p) => p.origin === 'native') },
        { label: '外部插件', items: plugins.filter((p) => p.origin === 'external') },
      ];
      for (const group of groups) {
        if (!group.items.length) continue;
        const title = document.createElement('div');
        title.className = 'settings-group-title';
        title.textContent = `${group.label}（${group.items.length}）`;
        pluginListEl.appendChild(title);
        for (const p of group.items) pluginListEl.appendChild(buildPluginCard(p));
      }
      pluginListEl.querySelectorAll<HTMLButtonElement>('.plugin-enable').forEach((btn) => {
        btn.addEventListener('click', () => { togglePlugin(btn.dataset.id || '', true, btn); });
      });
      pluginListEl.querySelectorAll<HTMLButtonElement>('.plugin-disable').forEach((btn) => {
        btn.addEventListener('click', () => { togglePlugin(btn.dataset.id || '', false, btn); });
      });
    } catch (err) {
      pluginsLoaded = false; // 失败允许下次点 tab 重试
      const msg = (err as Error).message;
      pluginListEl.innerHTML = `<div class="settings-tip">加载插件失败: ${esc(msg)}</div>`;
      toast(`插件加载失败: ${msg}`, 'error');
    }
  };

  /* ---- 声音克隆库（对接 /api/voice/*） ---- */
  const renderVoices = async (): Promise<void> => {
    try {
      const voices = await listVoices();
      if (!voices.length) {
        voiceListEl.innerHTML = '<div class="settings-tip">还没有参考音频</div>';
        return;
      }
      voiceListEl.innerHTML = '';
      for (const v of voices) {
        const row = document.createElement('div');
        row.className = 'settings-row';
        row.innerHTML = '<span class="lbl">' + esc(v.name) + (v.active ? ' (当前)' : '') + '</span>' +
          '<div class="settings-editor-actions">' +
          '<button class="btn ghost btn-xs voice-select" data-id="' + esc(v.id) + '" type="button">选用</button>' +
          '<button class="btn ghost btn-xs voice-apply" data-id="' + esc(v.id) + '" type="button">应用</button>' +
          '<button class="btn danger btn-xs voice-delete" data-id="' + esc(v.id) + '" type="button">删除</button>' +
          '</div>';
        voiceListEl.appendChild(row);
      }
      voiceListEl.querySelectorAll<HTMLButtonElement>('.voice-select').forEach((btn) => {
        btn.addEventListener('click', () => { void selectVoiceHandler(btn.dataset.id || ''); });
      });
      voiceListEl.querySelectorAll<HTMLButtonElement>('.voice-apply').forEach((btn) => {
        btn.addEventListener('click', () => { void applyVoiceHandler(btn.dataset.id || ''); });
      });
      voiceListEl.querySelectorAll<HTMLButtonElement>('.voice-delete').forEach((btn) => {
        btn.addEventListener('click', () => { void deleteVoiceHandler(btn.dataset.id || ''); });
      });
    } catch (err) {
      voiceStatusEl.textContent = '加载声音库失败: ' + (err as Error).message;
    }
  };

  const uploadVoiceHandler = (file: File): void => {
    const requestFn = async (token: number): Promise<void> => {
      voiceStatusEl.textContent = '上传中…';
      try {
        const voice = await uploadVoice(file);
        if (token !== requestToken) return;
        voiceStatusEl.textContent = '已上传: ' + voice.name;
        await renderVoices();
      } catch (err) {
        if (token === requestToken) voiceStatusEl.textContent = '上传失败: ' + (err as Error).message;
        throw err;
      }
    };
    withBusy(voiceUploadBtn, requestFn);
  };

  const selectVoiceHandler = async (voiceId: string): Promise<void> => {
    try {
      const voice = await selectVoice(voiceId);
      voiceStatusEl.textContent = '已选用: ' + voice.name;
      await renderVoices();
    } catch (err) {
      voiceStatusEl.textContent = '选用失败: ' + (err as Error).message;
    }
  };

  const applyVoiceHandler = async (voiceId: string): Promise<void> => {
    try {
      const result = await applyVoice(voiceId);
      voiceStatusEl.textContent = result.requires_restart ? result.message + '（请重启后端）' : result.message;
      await renderVoices();
    } catch (err) {
      voiceStatusEl.textContent = '应用失败: ' + (err as Error).message;
    }
  };

  const deleteVoiceHandler = async (voiceId: string): Promise<void> => {
    try {
      await deleteVoice(voiceId);
      voiceStatusEl.textContent = '已删除';
      await renderVoices();
    } catch (err) {
      voiceStatusEl.textContent = '删除失败: ' + (err as Error).message;
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

  const modelUploadInput = overlay.querySelector<HTMLInputElement>('.settings-model-upload-input')!;
  overlay.querySelector<HTMLButtonElement>('.settings-model-upload-btn')!.addEventListener('click', () => {
    modelUploadInput.value = '';
    modelUploadInput.click();
  });
  modelUploadInput.addEventListener('change', () => {
    const file = modelUploadInput.files?.[0];
    if (file) uploadModelHandler(file);
  });

  const configImportInput = overlay.querySelector<HTMLInputElement>('.settings-config-import-input')!;
  overlay.querySelector<HTMLButtonElement>('.settings-config-export-btn')!.addEventListener('click', () => { exportConfigHandler(); });
  overlay.querySelector<HTMLButtonElement>('.settings-config-import-btn')!.addEventListener('click', () => {
    configImportInput.value = '';
    configImportInput.click();
  });
  configImportInput.addEventListener('change', () => {
    const file = configImportInput.files?.[0];
    if (file) importConfigHandler(file);
  });

  voiceUploadBtn.addEventListener('click', () => {
    voiceUploadInput.value = '';
    voiceUploadInput.click();
  });
  voiceUploadInput.addEventListener('change', () => {
    const file = voiceUploadInput.files?.[0];
    if (file) uploadVoiceHandler(file);
  });

  overlay.querySelector<HTMLSelectElement>('.settings-log-level')!.addEventListener('change', () => {
    logLevelChanged = true; // A1：仅用户改动才提交
  });

  // 清除 System Prompt（人设收敛：唯一可清除的人设字段）
  const clearAllBtn = overlay.querySelector<HTMLButtonElement>('.settings-persona-clear-all')!;
  clearAllBtn.addEventListener('click', () => {
    overlay.querySelector<HTMLTextAreaElement>('.settings-persona-prompt')!.value = '';
    clearAllBtn.dataset.cleared = '1';
    setStatus('已标记清除 System Prompt，保存时生效', 'info');
  });

  // LLM/TTS 提供商下拉变化 → 标记对应 provider touched
  overlay.querySelector<HTMLSelectElement>('.settings-llm')!.addEventListener('change', (e) => {
    const id = (e.target as HTMLSelectElement).value;
    if (!id) return;
    const editor = llmCardsEl.querySelector<HTMLElement>(`.settings-provider-editor[data-provider="${id}"]`);
    if (editor && editor.dataset.configured !== '1') editor.dataset.touched = '1';
    syncThinkingControls(); // 思考开关跟随当前选中 provider
  });
  // 思考开关 → 强度行显隐；展示思考 → localStorage（纯前端偏好）
  overlay.querySelector<HTMLInputElement>('.settings-thinking-toggle')!.addEventListener('change', (e) => {
    overlay.querySelector<HTMLElement>('[data-thinking-effort-row]')!.hidden =
      !(e.target as HTMLInputElement).checked;
  });
  overlay.querySelector<HTMLInputElement>('.settings-show-thinking')!.addEventListener('change', (e) => {
    localStorage.setItem('showThinking', (e.target as HTMLInputElement).checked ? '1' : '0');
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

  // Esc 关闭（无障碍基线：模态必须可键盘退出）
  const onKeydown = (e: KeyboardEvent): void => {
    if (e.key === 'Escape' && overlay.classList.contains('open')) {
      e.stopPropagation();
      close();
    }
  };
  document.addEventListener('keydown', onKeydown);

  // 首次打开时预加载（main.ts 点击按钮再 open 也会触发 open；此处不自动打开）
  return {
    open: (tab?: string) => {
      if (tab) switchTab(tab);
      void renderVoices();
      void open();
    },
    close,
    destroy() {
      document.removeEventListener('keydown', onKeydown);
      externalSection?.stop();
      overlay.remove();
    },
  };
}

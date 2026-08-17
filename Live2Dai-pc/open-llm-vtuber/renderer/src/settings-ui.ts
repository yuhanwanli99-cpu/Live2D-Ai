/**
 * settings-ui.ts — PC 端完整设置 / Dashboard（多 Tab）
 *
 * Tab:
 *   - 对话/LLM
 *   - 语音/TTS
 *   - 形象/Live2D
 *   - API Keys
 *   - 人设/提示词
 *   - 系统
 *
 * 数据通过 /api/config 读写；Key 写入 .env，配置写回 conf.yaml / persona.yaml。
 */

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
  persona: PersonaCfg;
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
  openai_compatible: 'OpenAI 兼容',
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
  gpt_sovits: 'GPT-SoVITS',
  piper: 'Piper',
  sherpa_onnx: 'Sherpa-ONNX',
  pyttsx3: 'pyttsx3',
};

const providerLabel = (id: string, map: Record<string, string>): string =>
  map[id] || id;

const LLM_CATEGORIES = [
  { id: 'official', label: '官方直连', providers: ['deepseek_llm', 'zhipu_llm', 'qwen_llm', 'openai', 'anthropic'] },
  { id: 'aggregator', label: '聚合平台', providers: ['openrouter', 'together', 'fireworks', 'groq', 'moonshot', 'mistral', 'siliconflow'] },
  { id: 'custom', label: '其他 / 自定义', providers: ['opencode', 'opencode_go', 'openai_compatible'] },
];

const TTS_CATEGORIES = [
  { id: 'cloud', label: '云端 TTS', providers: ['cosyvoice_cloud', 'qwen_tts', 'openai_tts', 'azure_tts', 'minimax_tts', 'siliconflow_tts', 'fish_api_tts', 'spark_tts'] },
  { id: 'local', label: '本地 / 免费', providers: ['edge_tts', 'gpt_sovits', 'piper', 'sherpa_onnx', 'pyttsx3'] },
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

export function initSettingsPanel(
  container: HTMLElement,
  opts: { onChanged?: () => void } = {},
): SettingsPanel {
  const overlay = document.createElement('div');
  overlay.className = 'settings-overlay';
  overlay.innerHTML = `
    <div class="settings-box settings-box-wide">
      <div class="settings-header">
        <span>设置 / Dashboard</span>
        <button class="settings-close" type="button" title="关闭">✕</button>
      </div>
      <div class="settings-layout">
        <div class="settings-nav">
          <button class="settings-tab active" type="button" data-tab="llm">对话/LLM</button>
          <button class="settings-tab" type="button" data-tab="tts">语音/TTS</button>
          <button class="settings-tab" type="button" data-tab="live2d">形象/Live2D</button>
          <button class="settings-tab" type="button" data-tab="keys">API Keys</button>
          <button class="settings-tab" type="button" data-tab="persona">人设/提示词</button>
          <button class="settings-tab" type="button" data-tab="system">系统</button>
        </div>
        <div class="settings-content">
        <div class="settings-tab-panel active" data-panel="llm">
          <div class="settings-section-title">当前 LLM</div>
          <label class="settings-row">
            <span>LLM 提供商</span>
            <select class="settings-llm"></select>
          </label>
          <div class="settings-section-title">Provider 参数</div>
          <div class="settings-card-actions">
            <button class="settings-check-all" type="button">全部检查 / 列出可用模型</button>
          </div>
          <div class="settings-llm-cards"></div>
          <div class="settings-card-actions">
            <button class="settings-add-llm" type="button">+ 添加 OpenAI 兼容 Provider</button>
          </div>
        </div>
        <div class="settings-tab-panel" data-panel="tts">
          <div class="settings-section-title">TTS 与 ASR</div>
          <label class="settings-row">
            <span>TTS 提供商</span>
            <select class="settings-tts"></select>
          </label>
          <label class="settings-row">
            <span>ASR 引擎</span>
            <select class="settings-asr"></select>
          </label>
          <label class="settings-row">
            <span>口型模式</span>
            <select class="settings-viseme-mode">
              <option value="rms">RMS 快速（推荐，NEKO 同款）</option>
              <option value="off">关闭 viseme（最快）</option>
              <option value="pymouth">Pymouth 精细（慢）</option>
            </select>
          </label>
          <div class="settings-section-title">TTS 参数</div>
          <div class="settings-card-actions">
            <button class="settings-latency-edge" type="button">⚡ 低延迟 Edge TTS</button>
            <button class="settings-check-all" type="button">全部检查</button>
          </div>
          <div class="settings-tts-cards"></div>
        </div>
        <div class="settings-tab-panel" data-panel="live2d">
          <div class="settings-section-title">Live2D 模型</div>
          <div class="settings-card-actions">
            <button class="settings-scan-models" type="button">扫描并导入本地模型</button>
          </div>
          <div class="settings-model-list"></div>
          <div class="settings-status-inline"></div>
        </div>
        <div class="settings-tab-panel" data-panel="keys">
          <div class="settings-section-title">API Key 管理（写入 .env）</div>
          <div class="settings-key-cards"></div>
        </div>
        <div class="settings-tab-panel" data-panel="persona">
          <div class="settings-section-title">系统提示词</div>
          <label class="settings-row">
            <span>System Prompt</span>
            <textarea class="settings-persona-prompt" rows="14" spellcheck="false"></textarea>
          </label>
          <div class="settings-section-title">人设参数</div>
          <div class="settings-persona-grid">
            <label class="settings-row"><span>角色名</span><input class="settings-persona-name" type="text" /></label>
            <label class="settings-row"><span>角色定位</span><input class="settings-persona-role" type="text" /></label>
            <label class="settings-row"><span>模型</span><input class="settings-persona-model" type="text" /></label>
            <label class="settings-row"><span>Temperature</span><input class="settings-persona-temperature" type="number" step="0.1" min="0" max="2" /></label>
            <label class="settings-row"><span>Max Tokens</span><input class="settings-persona-max-tokens" type="number" min="1" /></label>
            <label class="settings-row"><span>视觉模型</span><input class="settings-persona-vision-model" type="text" /></label>
          </div>
        </div>
        <div class="settings-tab-panel" data-panel="system">
          <div class="settings-section-title">系统</div>
          <div class="settings-system-grid">
            <label class="settings-row"><span>角色名（conf.yaml）</span><input class="settings-character-name" type="text" /></label>
            <label class="settings-row"><span>端口</span><input class="settings-port" type="number" min="1" max="65535" /></label>
            <label class="settings-row"><span>Live2D FPS</span><input class="settings-fps" type="number" min="1" max="240" /></label>
            <label class="settings-row"><span>日志级别</span>
              <select class="settings-log-level">
                <option value="INFO">INFO（推荐）</option>
                <option value="DEBUG">DEBUG（调试）</option>
                <option value="WARNING">WARNING（安静）</option>
              </select>
            </label>
          </div>
          <div class="settings-tip">保存后需要重启后端才会生效。</div>
          <div class="settings-section-title">高级：conf.yaml 设定树</div>
          <label class="settings-row">
            <span>原始 YAML（可直接编辑整棵配置树）</span>
            <textarea class="settings-config-tree" rows="14" spellcheck="false"></textarea>
          </label>
          <div class="settings-card-actions">
            <button class="settings-save-tree" type="button">保存设定树</button>
          </div>
        </div>
        </div>
      </div>
      <div class="settings-footer">
        <span class="settings-status"></span>
        <button class="settings-apply" type="button">保存并应用</button>
        <button class="settings-save" type="button">保存全部</button>
      </div>
    </div>
  `;
  container.appendChild(overlay);

  let currentConfig: AppConfig = EMPTY_CONFIG;
  const statusEl = overlay.querySelector<HTMLElement>('.settings-status')!;
  const tabs = Array.from(overlay.querySelectorAll<HTMLButtonElement>('.settings-tab'));
  const panels = Array.from(overlay.querySelectorAll<HTMLElement>('.settings-tab-panel'));

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

  const fillSelect = (
    sel: HTMLSelectElement,
    values: string[],
    current: string,
    labelMap?: Record<string, string>,
  ): void => {
    sel.innerHTML = '';
    for (const v of values) {
      const opt = document.createElement('option');
      opt.value = v;
      opt.textContent = labelMap ? providerLabel(v, labelMap) : v;
      opt.selected = v === current;
      sel.appendChild(opt);
    }
  };

  const llmCardsEl = overlay.querySelector<HTMLElement>('.settings-llm-cards')!;
  const ttsCardsEl = overlay.querySelector<HTMLElement>('.settings-tts-cards')!;
  const keyCardsEl = overlay.querySelector<HTMLElement>('.settings-key-cards')!;
  const modelListEl = overlay.querySelector<HTMLElement>('.settings-model-list')!;
  const modelStatusEl = overlay.querySelector<HTMLElement>('.settings-status-inline')!;
  const configTreeEl = overlay.querySelector<HTMLTextAreaElement>('.settings-config-tree')!;

  const renderLlmCards = (): void => {
    llmCardsEl.innerHTML = '';
    const groups = LLM_CATEGORIES.map((c) => ({ ...c, providers: [] as LlmProviderCfg[] }));
    const other = { id: 'other', label: '其他', providers: [] as LlmProviderCfg[] };
    for (const p of currentConfig.llm_configs || []) {
      const cat = LLM_CATEGORIES.find((c) => c.providers.includes(p.id));
      if (cat) {
        groups.find((g) => g.id === cat.id)?.providers.push(p);
      } else {
        other.providers.push(p);
      }
    }
    const allGroups = [...groups, ...(other.providers.length > 0 ? [other] : [])];
    for (const group of allGroups) {
      if (group.providers.length === 0) continue;
      const details = document.createElement('details');
      details.className = 'settings-group';
      details.open = true;
      const summary = document.createElement('summary');
      summary.textContent = `${group.label} (${group.providers.length})`;
      details.appendChild(summary);
      const container = document.createElement('div');
      container.className = 'settings-group-cards';
      for (const p of group.providers) {
        const row = document.createElement('div');
        row.className = 'settings-provider-row';
        row.innerHTML = `
          <div class="settings-provider-row-title">
            <span>${esc(p.name || providerLabel(p.id, LLM_PROVIDER_LABELS))} <span class="settings-card-sub">${esc(p.id)}</span></span>
            <span class="settings-badge ${p.has_key ? 'ok' : ''}">${p.has_key ? '已配置' : '未配置 Key'}</span>
          </div>
          <button class="settings-provider-toggle" type="button">编辑</button>
        `;
        const editor = document.createElement('div');
        editor.className = 'settings-provider-editor';
        editor.style.display = 'none';
        editor.innerHTML = `
          <div class="settings-card-grid">
            <label class="settings-row"><span>模型</span>
              <select class="llm-model">
                ${[p.model, ...(p.models || [])].filter((v, i, a) => v && a.indexOf(v) === i).map((m) => `<option value="${esc(m)}" ${m === p.model ? 'selected' : ''}>${esc(m)}</option>`).join('') || `<option value="${esc(p.model)}">${esc(p.model)}</option>`}
              </select>
            </label>
            <label class="settings-row"><span>Base URL</span><input class="llm-base-url" type="text" value="${esc(p.base_url)}" /></label>
            <label class="settings-row"><span>Temperature</span><input class="llm-temperature" type="number" step="0.1" min="0" max="2" value="${p.temperature ?? 0.8}" /></label>
            <label class="settings-row"><span>Max Tokens</span><input class="llm-max-tokens" type="number" min="1" value="${p.max_tokens ?? ''}" /></label>
          </div>
          <div class="settings-card-actions">
            <button class="settings-discover-models" type="button" data-provider="${p.id}" data-key-env="${p.key_env || ''}">获取可用模型</button>
            <button class="settings-test-llm" type="button" data-provider="${p.id}">测试连接</button>
          </div>
        `;
        editor.dataset.provider = p.id;
        editor.dataset.configured = p.configured ? '1' : '0';
        const toggle = row.querySelector<HTMLButtonElement>('.settings-provider-toggle')!;
        toggle.addEventListener('click', () => {
          const show = editor.style.display === 'none';
          editor.style.display = show ? 'block' : 'none';
          toggle.textContent = show ? '收起' : '编辑';
          if (show && !p.configured) editor.dataset.touched = '1';
        });
        container.appendChild(row);
        container.appendChild(editor);
      }
      details.appendChild(container);
      llmCardsEl.appendChild(details);
    }
    llmCardsEl.querySelectorAll<HTMLButtonElement>('.settings-test-llm').forEach((btn) => {
      btn.addEventListener('click', () => {
        void testLlm(btn.dataset.provider || '');
      });
    });
    llmCardsEl.querySelectorAll<HTMLButtonElement>('.settings-discover-models').forEach((btn) => {
      btn.addEventListener('click', () => {
        const card = btn.closest<HTMLElement>('.settings-provider-editor');
        if (card) void discoverModels(btn.dataset.provider || '', card);
      });
    });
  };

  const renderTtsCards = (): void => {
    ttsCardsEl.innerHTML = '';
    const groups = TTS_CATEGORIES.map((c) => ({ ...c, providers: [] as TtsProviderCfg[] }));
    const other = { id: 'other', label: '其他', providers: [] as TtsProviderCfg[] };
    for (const p of currentConfig.tts_configs || []) {
      const cat = TTS_CATEGORIES.find((c) => c.providers.includes(p.id));
      if (cat) {
        groups.find((g) => g.id === cat.id)?.providers.push(p);
      } else {
        other.providers.push(p);
      }
    }
    const allGroups = [...groups, ...(other.providers.length > 0 ? [other] : [])];
    for (const group of allGroups) {
      if (group.providers.length === 0) continue;
      const details = document.createElement('details');
      details.className = 'settings-group';
      details.open = true;
      const summary = document.createElement('summary');
      summary.textContent = `${group.label} (${group.providers.length})`;
      details.appendChild(summary);
      const container = document.createElement('div');
      container.className = 'settings-group-cards';
      for (const p of group.providers) {
        const row = document.createElement('div');
        row.className = 'settings-provider-row';
        row.innerHTML = `
          <div class="settings-provider-row-title">
            <span>${esc(p.name || providerLabel(p.id, TTS_PROVIDER_LABELS))} <span class="settings-card-sub">${esc(p.id)}</span></span>
            <span class="settings-badge ${p.has_key ? 'ok' : ''}">${p.has_key ? '已配置' : '未配置 Key'}</span>
          </div>
          <button class="settings-provider-toggle" type="button">编辑</button>
        `;
        const editor = document.createElement('div');
        editor.className = 'settings-provider-editor';
        editor.style.display = 'none';
        editor.innerHTML = `
          <div class="settings-card-grid">
            <label class="settings-row"><span>音色 Voice</span>
              <select class="tts-voice">
                ${[p.voice, ...(p.voices || [])].filter((v, i, a) => v && a.indexOf(v) === i).map((v) => `<option value="${esc(v)}" ${v === p.voice ? 'selected' : ''}>${esc(v)}</option>`).join('') || `<option value="${esc(p.voice)}">${esc(p.voice)}</option>`}
              </select>
            </label>
            <label class="settings-row"><span>模型</span>
              <select class="tts-model">
                ${[p.model, ...(p.models || [])].filter((v, i, a) => v && a.indexOf(v) === i).map((v) => `<option value="${esc(v)}" ${v === p.model ? 'selected' : ''}>${esc(v)}</option>`).join('') || `<option value="${esc(p.model)}">${esc(p.model)}</option>`}
              </select>
            </label>
            <label class="settings-row"><span>WebSocket URL</span><input class="tts-ws-url" type="text" value="${esc(p.ws_url || '')}" /></label>
          </div>
          <div class="settings-card-actions">
            <button class="settings-test-tts" type="button">试听当前引擎</button>
          </div>
        `;
        editor.dataset.provider = p.id;
        editor.dataset.configured = p.configured ? '1' : '0';
        const toggle = row.querySelector<HTMLButtonElement>('.settings-provider-toggle')!;
        toggle.addEventListener('click', () => {
          const show = editor.style.display === 'none';
          editor.style.display = show ? 'block' : 'none';
          toggle.textContent = show ? '收起' : '编辑';
          if (show && !p.configured) editor.dataset.touched = '1';
        });
        container.appendChild(row);
        container.appendChild(editor);
      }
      details.appendChild(container);
      ttsCardsEl.appendChild(details);
    }
    ttsCardsEl.querySelectorAll<HTMLButtonElement>('.settings-test-tts').forEach((btn) => {
      btn.addEventListener('click', () => {
        void testTts();
      });
    });
  };

  const renderKeyCards = (): void => {
    const keyGroups: Array<{
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

    keyCardsEl.innerHTML = '';
    for (const group of keyGroups) {
      const details = document.createElement('details');
      details.className = 'settings-group';
      details.open = true;
      const summary = document.createElement('summary');
      summary.textContent = `${group.label} (${group.items.length})`;
      details.appendChild(summary);
      const container = document.createElement('div');
      container.className = 'settings-group-cards';
      for (const item of group.items) {
        const row = document.createElement('div');
        row.className = 'settings-key-item';
        row.innerHTML = `
          <span class="settings-key-label">${item.label} <span class="settings-card-sub">${item.env}</span></span>
          <div class="settings-key-row">
            <input class="settings-key-input" type="password" data-env="${item.env}" placeholder="留空不修改" autocomplete="off" />
            <button class="settings-key-toggle" type="button">显示</button>
            ${item.provider ? `<button class="settings-key-test" type="button" data-provider="${item.provider}" data-env="${item.env}">测试</button>` : ''}
          </div>
        `;
        container.appendChild(row);
      }
      details.appendChild(container);
      keyCardsEl.appendChild(details);
    }

    keyCardsEl.querySelectorAll<HTMLButtonElement>('.settings-key-toggle').forEach((btn) => {
      btn.addEventListener('click', () => {
        const input = btn.parentElement?.querySelector<HTMLInputElement>('.settings-key-input');
        if (!input) return;
        const show = input.type === 'password';
        input.type = show ? 'text' : 'password';
        btn.textContent = show ? '隐藏' : '显示';
      });
    });
    keyCardsEl.querySelectorAll<HTMLButtonElement>('.settings-key-test').forEach((btn) => {
      btn.addEventListener('click', () => {
        const input = btn.parentElement?.querySelector<HTMLInputElement>('.settings-key-input');
        const key = input?.value.trim() || '';
        void testLlm(btn.dataset.provider || '', key);
      });
    });
  };

  const addLlmProvider = (): void => {
    const id = window.prompt('Provider ID（例如 openai_my）');
    if (!id || !id.trim()) return;
    const model = window.prompt('模型名（例如 gpt-4o-mini）');
    if (!model || !model.trim()) return;
    const baseUrl = window.prompt('Base URL（例如 https://api.openai.com/v1）');
    if (!baseUrl || !baseUrl.trim()) return;
    const apiKey = window.prompt('API Key（可留空，之后在 API Keys 里填）') || '';

    const llmSel = overlay.querySelector<HTMLSelectElement>('.settings-llm')!;
    const opt = document.createElement('option');
    opt.value = id.trim();
    opt.textContent = id.trim();
    opt.selected = true;
    llmSel.appendChild(opt);

    const card = document.createElement('div');
    card.className = 'settings-card';
    card.dataset.provider = id.trim();
    card.innerHTML = `
      <div class="settings-card-title">
        <span>${esc(id.trim())}</span>
        <span class="settings-badge">新增</span>
      </div>
      <div class="settings-card-grid">
        <label class="settings-row"><span>模型</span><select class="llm-model"><option value="${esc(model.trim())}">${esc(model.trim())}</option></select></label>
        <label class="settings-row"><span>Base URL</span><input class="llm-base-url" type="text" value="${esc(baseUrl.trim())}" /></label>
        <label class="settings-row"><span>Temperature</span><input class="llm-temperature" type="number" step="0.1" min="0" max="2" value="0.8" /></label>
        <label class="settings-row"><span>Max Tokens</span><input class="llm-max-tokens" type="number" min="1" value="" /></label>
      </div>
      <div class="settings-card-actions">
        <button class="settings-discover-models" type="button" data-provider="${esc(id.trim())}" data-key-env="OPENAI_API_KEY">获取可用模型</button>
        <button class="settings-test-llm" type="button" data-provider="${esc(id.trim())}">测试连接</button>
      </div>
    `;
    llmCardsEl.appendChild(card);
    card.querySelector<HTMLButtonElement>('.settings-test-llm')!.addEventListener('click', () => {
      void testLlm(id.trim());
    });
    card.querySelector<HTMLButtonElement>('.settings-discover-models')!.addEventListener('click', () => {
      void discoverModels(id.trim(), card);
    });

    if (apiKey.trim()) {
      const keyInput = keyCardsEl.querySelector<HTMLInputElement>(
        '.settings-key-input[data-env="OPENAI_API_KEY"]',
      );
      if (keyInput) keyInput.value = apiKey.trim();
    }
  };

  const markLlmTouched = (provider: string): void => {
    const editor = llmCardsEl.querySelector<HTMLElement>(
      `.settings-provider-editor[data-provider="${provider}"]`,
    );
    if (editor && editor.dataset.configured !== '1') {
      editor.dataset.touched = '1';
    }
  };

  const markTtsTouched = (provider: string): void => {
    const editor = ttsCardsEl.querySelector<HTMLElement>(
      `.settings-provider-editor[data-provider="${provider}"]`,
    );
    if (editor && editor.dataset.configured !== '1') {
      editor.dataset.touched = '1';
    }
  };

  const checkAll = async (): Promise<void> => {
    statusEl.textContent = '检查所有 Provider 中…';
    try {
      const res = await fetch('/api/providers/check', { method: 'POST' });
      const data = (await res.json()) as {
        llm?: Array<{ provider: string; ok: boolean; status: string; models?: string[] }>;
        tts?: Array<{ provider: string; ok: boolean; status: string }>;
        error?: string;
      };
      if (!res.ok || data.error) throw new Error(data.error || '检查失败');
      const llm = data.llm || [];
      const tts = data.tts || [];
      const llmOk = llm.filter((x) => x.ok).length;
      const ttsOk = tts.filter((x) => x.ok).length;
      const lines: string[] = [
        `✅ LLM ${llmOk}/${llm.length} 可用`,
        `TTS ${ttsOk}/${tts.length} 已配置`,
      ];
      const failed = llm.filter((x) => !x.ok);
      if (failed.length > 0) {
        lines.push('未通过: ' + failed.map((x) => `${x.provider}(${x.status})`).join(', '));
      }
      statusEl.textContent = lines.join(' · ');
    } catch (err) {
      statusEl.textContent = `❌ 检查失败: ${(err as Error).message}`;
    }
  };

  const discoverModels = async (provider: string, card: HTMLElement): Promise<void> => {
    statusEl.textContent = `获取 ${provider} 模型列表…`;
    const baseUrl = card.querySelector<HTMLInputElement>('.llm-base-url')?.value.trim() || '';
    const keyEnv = card.dataset.keyEnv || '';
    const keyInput = keyCardsEl.querySelector<HTMLInputElement>(
      `.settings-key-input[data-env="${keyEnv}"]`,
    );
    const apiKey = keyInput?.value.trim() || '';
    try {
      const res = await fetch('/api/llm/discover-models', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ provider, base_url: baseUrl, api_key: apiKey }),
      });
      const data = (await res.json()) as { ok?: boolean; models?: string[]; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '获取失败');
      const modelSelect = card.querySelector<HTMLSelectElement>('.llm-model');
      const datalist = card.querySelector<HTMLDataListElement>('datalist');
      const modelInput = card.querySelector<HTMLInputElement>('.llm-model');
      const models = data.models || [];
      if (modelSelect) {
        const current = modelSelect.value;
        const options = [...new Set([...(models.length ? models : current ? [current] : []), current].filter(Boolean))];
        modelSelect.innerHTML = options.map((m) => `<option value="${esc(m)}" ${m === current ? 'selected' : ''}>${esc(m)}</option>`).join('') || `<option value="${esc(current)}">${esc(current)}</option>`;
        if (!current && models.length) modelSelect.value = models[0];
      }
      if (datalist) {
        datalist.innerHTML = models.map((m) => `<option value="${esc(m)}"></option>`).join('');
      }
      if (modelInput && !modelInput.value && models.length) {
        modelInput.value = models[0];
      }
      statusEl.textContent = `✅ 获取到 ${data.models?.length || 0} 个模型`;
    } catch (err) {
      statusEl.textContent = `❌ 获取失败: ${(err as Error).message}`;
    }
  };

  const autoDiscoverConfiguredModels = async (): Promise<void> => {
    const cards = llmCardsEl.querySelectorAll<HTMLElement>('.settings-provider-editor');
    for (const card of Array.from(cards)) {
      const provider = card.dataset.provider || '';
      if (!provider) continue;
      const p = currentConfig.llm_configs.find((x) => x.id === provider);
      if (!p?.configured) continue;
      if (!p.has_key && !p.base_url) continue;
      await discoverModels(provider, card);
    }
  };

  const renderModels = async (): Promise<void> => {
    modelListEl.innerHTML = '<div class="settings-tip">加载模型列表…</div>';
    try {
      const res = await fetch('/api/models');
      const data = (await res.json()) as { models?: Live2DModel[]; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '加载失败');
      modelListEl.innerHTML = '';
      for (const m of data.models || []) {
        const card = document.createElement('div');
        card.className = 'settings-card settings-model-card';
        card.innerHTML = `
          <div class="settings-card-title">
            <span>${esc(m.name || m.id)}</span>
            ${m.default ? '<span class="settings-badge ok">默认</span>' : ''}
          </div>
          <div class="settings-tip">${esc(m.description || m.id)}</div>
          <div class="settings-card-actions">
            <button class="settings-select-model" type="button" data-id="${esc(m.id)}">切换到此模型</button>
          </div>
        `;
        modelListEl.appendChild(card);
      }
      modelListEl.querySelectorAll<HTMLButtonElement>('.settings-select-model').forEach((btn) => {
        btn.addEventListener('click', () => {
          void selectModel(btn.dataset.id || '');
        });
      });
    } catch (err) {
      modelListEl.innerHTML = `<div class="settings-error">加载模型失败: ${esc((err as Error).message)}</div>`;
    }
  };

  const renderPersona = (): void => {
    const p = currentConfig.persona || EMPTY_CONFIG.persona;
    overlay.querySelector<HTMLTextAreaElement>('.settings-persona-prompt')!.value = p.system_prompt || '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-name')!.value = p.name || '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-role')!.value = p.role || '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-model')!.value = p.model || '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-temperature')!.value = String(p.temperature ?? 0.8);
    overlay.querySelector<HTMLInputElement>('.settings-persona-max-tokens')!.value = p.max_tokens ? String(p.max_tokens) : '';
    overlay.querySelector<HTMLInputElement>('.settings-persona-vision-model')!.value = p.vision_model || '';
  };

  const renderSystem = (): void => {
    overlay.querySelector<HTMLInputElement>('.settings-character-name')!.value = currentConfig.character_name || '';
    overlay.querySelector<HTMLInputElement>('.settings-port')!.value = String(currentConfig.port ?? 12393);
    overlay.querySelector<HTMLInputElement>('.settings-fps')!.value = String(currentConfig.live2d_fps ?? 120);
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
    fillSelect(
      overlay.querySelector<HTMLSelectElement>('.settings-asr')!,
      asrOptions,
      cfg.asr_model || 'disabled',
    );
    overlay.querySelector<HTMLSelectElement>('.settings-viseme-mode')!.value =
      cfg.viseme_mode || 'rms';
    renderLlmCards();
    renderTtsCards();
    renderKeyCards();
    renderPersona();
    renderSystem();
    void autoDiscoverConfiguredModels();
    void renderModels();
  };

  const open = async (): Promise<void> => {
    overlay.style.display = 'flex';
    statusEl.textContent = '加载中…';
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
      statusEl.textContent = '';
    } catch (err) {
      statusEl.textContent = `加载失败: ${(err as Error).message}`;
    }
  };

  const collectPayload = (): Record<string, unknown> => {
    const llmConfigs: Record<string, Record<string, unknown>> = {};
    llmCardsEl.querySelectorAll<HTMLElement>('.settings-provider-editor, .settings-card').forEach((card) => {
      const provider = card.dataset.provider || '';
      const isLegacyCard = card.classList.contains('settings-card');
      const configured = card.dataset.configured === '1';
      const touched = card.dataset.touched === '1';
      if (!isLegacyCard && !configured && !touched) return;
      const llmModels = Array.from(
        card.querySelectorAll<HTMLOptionElement>('.llm-model option, datalist option'),
      ).map((o) => o.value).filter(Boolean);
      llmConfigs[provider] = {
        model: card.querySelector<HTMLInputElement>('.llm-model')?.value || '',
        base_url: card.querySelector<HTMLInputElement>('.llm-base-url')?.value || '',
        temperature: Number(card.querySelector<HTMLInputElement>('.llm-temperature')?.value || 0.8),
        max_tokens: Number(card.querySelector<HTMLInputElement>('.llm-max-tokens')?.value || 0) || undefined,
        ...(llmModels.length > 0 ? { models: llmModels } : {}),
      };
    });

    const ttsConfigs: Record<string, Record<string, unknown>> = {};
    ttsCardsEl.querySelectorAll<HTMLElement>('.settings-provider-editor, .settings-card').forEach((card) => {
      const provider = card.dataset.provider || '';
      const isLegacyCard = card.classList.contains('settings-card');
      const configured = card.dataset.configured === '1';
      const touched = card.dataset.touched === '1';
      if (!isLegacyCard && !configured && !touched) return;
      const ttsVoices = Array.from(
        card.querySelectorAll<HTMLOptionElement>('.tts-voice option'),
      ).map((o) => o.value).filter(Boolean);
      const ttsModels = Array.from(
        card.querySelectorAll<HTMLOptionElement>('.tts-model option'),
      ).map((o) => o.value).filter(Boolean);
      ttsConfigs[provider] = {
        voice: card.querySelector<HTMLSelectElement>('.tts-voice')?.value || '',
        model: card.querySelector<HTMLSelectElement>('.tts-model')?.value || '',
        ws_url: card.querySelector<HTMLInputElement>('.tts-ws-url')?.value || '',
        ...(ttsVoices.length > 0 ? { voices: ttsVoices } : {}),
        ...(ttsModels.length > 0 ? { models: ttsModels } : {}),
      };
    });

    const apiKeys: Record<string, string> = {};
    keyCardsEl.querySelectorAll<HTMLInputElement>('.settings-key-input').forEach((input) => {
      const v = input.value.trim();
      if (v) apiKeys[input.dataset.env || ''] = v;
    });

    return {
      llm_provider: overlay.querySelector<HTMLSelectElement>('.settings-llm')!.value,
      tts_model: overlay.querySelector<HTMLSelectElement>('.settings-tts')!.value,
      viseme_mode: overlay.querySelector<HTMLSelectElement>('.settings-viseme-mode')!.value,
      asr_model: overlay.querySelector<HTMLSelectElement>('.settings-asr')!.value,
      character_name: overlay.querySelector<HTMLInputElement>('.settings-character-name')!.value.trim(),
      port: Number(overlay.querySelector<HTMLInputElement>('.settings-port')!.value),
      live2d_fps: Number(overlay.querySelector<HTMLInputElement>('.settings-fps')!.value),
      llm_configs: llmConfigs,
      tts_configs: ttsConfigs,
      api_keys: apiKeys,
      persona: {
        name: overlay.querySelector<HTMLInputElement>('.settings-persona-name')!.value.trim(),
        role: overlay.querySelector<HTMLInputElement>('.settings-persona-role')!.value.trim(),
        model: overlay.querySelector<HTMLInputElement>('.settings-persona-model')!.value.trim(),
        temperature: Number(overlay.querySelector<HTMLInputElement>('.settings-persona-temperature')!.value || 0.8),
        max_tokens: Number(overlay.querySelector<HTMLInputElement>('.settings-persona-max-tokens')!.value || 0) || undefined,
        vision_model: overlay.querySelector<HTMLInputElement>('.settings-persona-vision-model')!.value.trim(),
        system_prompt: overlay.querySelector<HTMLTextAreaElement>('.settings-persona-prompt')!.value,
      },
      log_level: overlay.querySelector<HTMLSelectElement>('.settings-log-level')!.value,
    };
  };

  const save = async (apply = false): Promise<void> => {
    statusEl.textContent = apply ? '保存并应用（热重载）中…' : '保存中…';
    try {
      const res = await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...collectPayload(), apply }),
      });
      const data = (await res.json()) as { ok?: boolean; applied?: boolean; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '保存失败');
      statusEl.textContent = data.applied
        ? '✅ 已保存并热重载'
        : '✅ 已保存，重启后生效';
      opts.onChanged?.();
    } catch (err) {
      statusEl.textContent = `保存失败: ${(err as Error).message}`;
    }
  };

  const testLlm = async (provider: string, apiKey = ''): Promise<void> => {
    statusEl.textContent = `测试 ${provider} 中…`;
    try {
      const res = await fetch('/api/test-llm', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ provider, api_key: apiKey }),
      });
      const data = (await res.json()) as { ok?: boolean; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '测试失败');
      statusEl.textContent = `✅ ${provider} 连接正常`;
    } catch (err) {
      statusEl.textContent = `❌ ${provider} 测试失败: ${(err as Error).message}`;
    }
  };

  const testTts = async (): Promise<void> => {
    statusEl.textContent = '试听生成中…';
    try {
      const res = await fetch('/api/test-tts', { method: 'POST' });
      const data = (await res.json()) as { ok?: boolean; audio_path?: string; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '试听失败');
      if (data.audio_path) {
        const audioPath = data.audio_path.replace(/\\/g, '/');
        const audioUrl = /^https?:\/\//.test(audioPath)
          ? audioPath
          : audioPath.startsWith('/')
            ? audioPath
            : `/${audioPath}`;
        const audio = new Audio(audioUrl);
        void audio.play();
        statusEl.textContent = '🔊 正在播放试听…';
      } else {
        statusEl.textContent = '✅ 试听已生成';
      }
    } catch (err) {
      statusEl.textContent = `❌ 试听失败: ${(err as Error).message}`;
    }
  };

  const switchEdgeTts = (): void => {
    const select = overlay.querySelector<HTMLSelectElement>('.settings-tts');
    if (!select) return;
    select.value = 'edge_tts';
    markTtsTouched('edge_tts');
    const editor = ttsCardsEl.querySelector<HTMLElement>(
      '.settings-provider-editor[data-provider="edge_tts"]',
    );
    if (editor) {
      editor.dataset.touched = '1';
      const voice = editor.querySelector<HTMLInputElement>('.tts-voice');
      if (voice && !voice.value.trim()) voice.value = 'zh-CN-XiaoxiaoNeural';
      editor.style.display = 'block';
      const toggle = editor.previousElementSibling?.querySelector<HTMLButtonElement>('.settings-provider-toggle');
      if (toggle) toggle.textContent = '收起';
    }
    statusEl.textContent = '⚡ 已切换低延迟 Edge TTS，点“保存并应用”生效';
  };

  const scanModels = async (): Promise<void> => {
    modelStatusEl.textContent = '扫描中…';
    try {
      const res = await fetch('/api/models/scan', { method: 'POST' });
      const data = (await res.json()) as { ok?: boolean; added?: string[]; models?: Live2DModel[]; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '扫描失败');
      const added = data.added || [];
      modelStatusEl.textContent = added.length > 0
        ? `✅ 已导入: ${added.join(', ')}`
        : '✅ 扫描完成，没有发现新模型';
      await renderModels();
    } catch (err) {
      modelStatusEl.textContent = `❌ 扫描失败: ${(err as Error).message}`;
    }
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
      modelStatusEl.textContent = '✅ 已切换，重启后生效';
    } catch (err) {
      modelStatusEl.textContent = `❌ 切换失败: ${(err as Error).message}`;
    }
  };

  const saveTree = async (): Promise<void> => {
    statusEl.textContent = '保存设定树中…';
    try {
      const res = await fetch('/api/config/tree', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ yaml: configTreeEl.value }),
      });
      const data = (await res.json()) as { ok?: boolean; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '保存失败');
      statusEl.textContent = '✅ 设定树已保存，重启后生效';
    } catch (err) {
      statusEl.textContent = `保存失败: ${(err as Error).message}`;
    }
  };

  overlay.querySelector<HTMLSelectElement>('.settings-llm')!.addEventListener('change', (e) => {
    markLlmTouched((e.target as HTMLSelectElement).value);
  });
  overlay.querySelector<HTMLSelectElement>('.settings-tts')!.addEventListener('change', (e) => {
    markTtsTouched((e.target as HTMLSelectElement).value);
  });

  overlay.querySelector<HTMLButtonElement>('.settings-close')!.addEventListener('click', close);
  overlay.querySelector<HTMLButtonElement>('.settings-save')!.addEventListener('click', () => {
    void save(false);
  });
  overlay.querySelector<HTMLButtonElement>('.settings-apply')!.addEventListener('click', () => {
    void save(true);
  });
  overlay.querySelector<HTMLButtonElement>('.settings-save-tree')!.addEventListener('click', () => {
    void saveTree();
  });
  overlay.querySelector<HTMLButtonElement>('.settings-add-llm')!.addEventListener('click', addLlmProvider);
  overlay.querySelector<HTMLButtonElement>('.settings-scan-models')?.addEventListener('click', () => {
    void scanModels();
  });
  overlay.querySelector<HTMLButtonElement>('.settings-latency-edge')?.addEventListener('click', () => {
    switchEdgeTts();
  });
  overlay.querySelectorAll<HTMLButtonElement>('.settings-check-all').forEach((btn) => {
    btn.addEventListener('click', () => {
      void checkAll();
    });
  });
  overlay.addEventListener('click', (e) => {
    if (e.target === overlay) close();
  });

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

function esc(text: string): string {
  return String(text ?? '')
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

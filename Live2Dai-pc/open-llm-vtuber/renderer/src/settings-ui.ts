/**
 * settings-ui.ts — PC 端轻量设置 / Dashboard 面板
 *
 * 通过 /api/config 读取当前 conf.yaml 常用项，保存时写回 conf.yaml 与 .env。
 * 保存后需要重启后端生效（面板内会提示）。
 */
export interface SettingsConfig {
  llm_provider: string;
  llm_providers: string[];
  tts_model: string;
  tts_providers: string[];
  asr_model: string;
  asr_providers: string[];
  character_name: string;
  port: number;
  live2d_fps: number;
}

export interface SettingsPanel {
  open(): void;
  close(): void;
  destroy(): void;
}

export function initSettingsPanel(
  container: HTMLElement,
  opts: { onChanged?: () => void } = {},
): SettingsPanel {
  const overlay = document.createElement('div');
  overlay.className = 'settings-overlay';
  overlay.innerHTML = `
    <div class="settings-box">
      <div class="settings-header">
        <span>设置 / Dashboard</span>
        <button class="settings-close" type="button" title="关闭">✕</button>
      </div>
      <div class="settings-body">
        <label class="settings-row">
          <span>LLM 提供商</span>
          <select class="settings-llm"></select>
        </label>
        <label class="settings-row">
          <span>DeepSeek API Key</span>
          <input class="settings-key-deepseek" type="password" placeholder="留空不修改" autocomplete="off" />
        </label>
        <label class="settings-row">
          <span>智谱 API Key</span>
          <input class="settings-key-zhipu" type="password" placeholder="留空不修改" autocomplete="off" />
        </label>
        <label class="settings-row">
          <span>Qwen / DashScope API Key</span>
          <input class="settings-key-qwen" type="password" placeholder="留空不修改" autocomplete="off" />
        </label>
        <label class="settings-row">
          <span>TTS 提供商</span>
          <select class="settings-tts"></select>
        </label>
        <label class="settings-row">
          <span>ASR 引擎</span>
          <select class="settings-asr"></select>
        </label>
        <label class="settings-row">
          <span>角色名</span>
          <input class="settings-name" type="text" autocomplete="off" />
        </label>
        <label class="settings-row">
          <span>端口</span>
          <input class="settings-port" type="number" min="1" max="65535" />
        </label>
        <label class="settings-row">
          <span>Live2D FPS</span>
          <input class="settings-fps" type="number" min="1" max="240" />
        </label>
        <div class="settings-status"></div>
      </div>
      <div class="settings-footer">
        <button class="settings-cancel" type="button">取消</button>
        <button class="settings-save" type="button">保存</button>
      </div>
    </div>
  `;
  container.appendChild(overlay);

  const llmSel = overlay.querySelector<HTMLSelectElement>('.settings-llm')!;
  const ttsSel = overlay.querySelector<HTMLSelectElement>('.settings-tts')!;
  const asrSel = overlay.querySelector<HTMLSelectElement>('.settings-asr')!;
  const deepseekInput = overlay.querySelector<HTMLInputElement>('.settings-key-deepseek')!;
  const zhipuInput = overlay.querySelector<HTMLInputElement>('.settings-key-zhipu')!;
  const qwenInput = overlay.querySelector<HTMLInputElement>('.settings-key-qwen')!;
  const nameInput = overlay.querySelector<HTMLInputElement>('.settings-name')!;
  const portInput = overlay.querySelector<HTMLInputElement>('.settings-port')!;
  const fpsInput = overlay.querySelector<HTMLInputElement>('.settings-fps')!;
  const statusEl = overlay.querySelector<HTMLElement>('.settings-status')!;

  const close = (): void => {
    overlay.style.display = 'none';
  };

  const fillSelect = (
    sel: HTMLSelectElement,
    values: string[],
    current: string,
  ): void => {
    sel.innerHTML = '';
    for (const v of values) {
      const opt = document.createElement('option');
      opt.value = v;
      opt.textContent = v;
      opt.selected = v === current;
      sel.appendChild(opt);
    }
  };

  const open = async (): Promise<void> => {
    overlay.style.display = 'flex';
    statusEl.textContent = '加载中…';
    try {
      const res = await fetch('/api/config');
      const data = (await res.json()) as SettingsConfig & { error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '加载失败');
      fillSelect(llmSel, data.llm_providers || [], data.llm_provider || '');
      fillSelect(ttsSel, data.tts_providers || [], data.tts_model || '');
      const asrOptions =
        data.asr_providers && data.asr_providers.length
          ? data.asr_providers
          : ['faster_whisper', 'disabled'];
      fillSelect(asrSel, asrOptions, data.asr_model || 'disabled');
      nameInput.value = data.character_name || '';
      portInput.value = String(data.port ?? 12393);
      fpsInput.value = String(data.live2d_fps ?? 120);
      deepseekInput.value = '';
      zhipuInput.value = '';
      qwenInput.value = '';
      statusEl.textContent = '';
    } catch (err) {
      statusEl.textContent = `加载失败: ${(err as Error).message}`;
    }
  };

  const save = async (): Promise<void> => {
    const apiKeys: Record<string, string> = {};
    const deepseekKey = deepseekInput.value.trim();
    const zhipuKey = zhipuInput.value.trim();
    const qwenKey = qwenInput.value.trim();
    if (deepseekKey) apiKeys.DEEPSEEK_API_KEY = deepseekKey;
    if (zhipuKey) apiKeys.ZHIPU_API_KEY = zhipuKey;
    if (qwenKey) apiKeys.DASHSCOPE_API_KEY = qwenKey;

    const payload = {
      llm_provider: llmSel.value,
      tts_model: ttsSel.value,
      asr_model: asrSel.value,
      character_name: nameInput.value.trim(),
      port: Number(portInput.value),
      live2d_fps: Number(fpsInput.value),
      api_keys: apiKeys,
    };
    statusEl.textContent = '保存中…';
    try {
      const res = await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const data = (await res.json()) as { ok?: boolean; error?: string };
      if (!res.ok || data.error) throw new Error(data.error || '保存失败');
      statusEl.textContent = '✅ 已保存，重启后生效';
      opts.onChanged?.();
    } catch (err) {
      statusEl.textContent = `保存失败: ${(err as Error).message}`;
    }
  };

  overlay.querySelector<HTMLButtonElement>('.settings-close')!.addEventListener('click', close);
  overlay.querySelector<HTMLButtonElement>('.settings-cancel')!.addEventListener('click', close);
  overlay.querySelector<HTMLButtonElement>('.settings-save')!.addEventListener('click', () => {
    void save();
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

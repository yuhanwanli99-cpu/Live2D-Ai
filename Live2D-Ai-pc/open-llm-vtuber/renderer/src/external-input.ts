/**
 * external-input.ts — 「外部接入」设置区（仅 PC 端）
 *
 * 作用：让用户**自主决定**是否接收外部程序传来的文字对话，以及随时断开。
 * 对应后端 `src/open_llm_vtuber/external_input.py`：
 *   GET  /api/external/status    读开关 + 在线会话 + 最近注入记录
 *   POST /api/external/settings  写开关（仅本机可改）
 *   GET  /api/external/token     回显访问令牌（仅本机可读，便于复制）
 *   POST /api/external/chat      外部程序真正调用的注入接口
 *
 * 安全语义（与后端一致，UI 必须如实呈现）：
 *   - 默认关闭；关闭时接口一律 403；
 *   - 打开后**只允许本机**调用；要放开局域网必须同时勾选「允许局域网」并设置令牌；
 *   - 「断开接收」= 关开关，立即生效（不需要保存全部设置、不需要重启）。
 *
 * 纯逻辑（buildCurlExample / describeGateScope / validateGateChange / formatRecentLine）
 * 与 DOM 装配分离，便于 vitest 单测。
 */

/* ------------------------------------------------------------------ *
 * 类型（与后端 external_input.py status() 同构）
 * ------------------------------------------------------------------ */

export interface ExternalRecord {
  at: number;
  source: string;
  text: string;
  ok: boolean;
  detail?: string;
}

export interface ExternalStatus {
  enabled: boolean;
  allow_remote: boolean;
  token_set: boolean;
  endpoint: string;
  clients: string[];
  accepted: number;
  rejected: number;
  recent: ExternalRecord[];
  limits?: { max_text_chars?: number; max_wait_reply_s?: number };
}

export const EMPTY_EXTERNAL_STATUS: ExternalStatus = {
  enabled: false,
  allow_remote: false,
  token_set: false,
  endpoint: '/api/external/chat',
  clients: [],
  accepted: 0,
  rejected: 0,
  recent: [],
};

/* ------------------------------------------------------------------ *
 * 纯逻辑
 * ------------------------------------------------------------------ */

/** 一句话描述当前的接收范围（直接展示给用户，不要含糊）。 */
export function describeGateScope(status: ExternalStatus): string {
  if (!status.enabled) return '已断开：外部程序无法让角色说话';
  if (status.allow_remote && status.token_set) return '已开启：本机 + 局域网（需携带令牌）';
  return '已开启：仅本机（127.0.0.1）可调用';
}

/**
 * 开关变更的前置校验（与后端同一条规则，避免提交后才报错）。
 * @returns 错误文案；合法返回 null。
 */
export function validateGateChange(next: {
  enabled: boolean;
  allowRemote: boolean;
  tokenSet: boolean;
}): string | null {
  if (next.allowRemote && !next.tokenSet) {
    return '允许局域网接入前必须先设置访问令牌（点「生成令牌」）';
  }
  return null;
}

/** 生成可直接粘贴运行的调用示例。 */
export function buildCurlExample(origin: string, token: string, endpoint = '/api/external/chat'): string {
  const base = (origin || 'http://127.0.0.1:12393').replace(/\/+$/, '');
  const authLine = token ? `\n  -H 'X-Live2DAI-Token: ${token}' \\` : '';
  return (
    `curl -X POST ${base}${endpoint} \\\n` +
    `  -H 'Content-Type: application/json' \\${authLine}\n` +
    `  -d '{"text":"你好，我是外部程序","wait_reply":true}'`
  );
}

/** 最近记录 → 单行展示文案。 */
export function formatRecentLine(record: ExternalRecord): string {
  const time = new Date((record.at || 0) * 1000).toLocaleTimeString('zh-CN', { hour12: false });
  const mark = record.ok ? '已接收' : `已拒绝（${record.detail || '未知原因'}）`;
  return `${time} · ${record.source} · ${mark} · ${record.text || '(空)'}`;
}

/* ------------------------------------------------------------------ *
 * API 客户端
 * ------------------------------------------------------------------ */

async function readJson(res: Response): Promise<Record<string, unknown>> {
  try {
    return (await res.json()) as Record<string, unknown>;
  } catch {
    return {};
  }
}

function asStatus(data: Record<string, unknown>): ExternalStatus {
  return {
    enabled: !!data.enabled,
    allow_remote: !!data.allow_remote,
    token_set: !!data.token_set,
    endpoint: typeof data.endpoint === 'string' ? data.endpoint : '/api/external/chat',
    clients: Array.isArray(data.clients) ? (data.clients as string[]) : [],
    accepted: Number(data.accepted) || 0,
    rejected: Number(data.rejected) || 0,
    recent: Array.isArray(data.recent) ? (data.recent as ExternalRecord[]) : [],
    limits: (data.limits as ExternalStatus['limits']) || undefined,
  };
}

export async function fetchExternalStatus(): Promise<ExternalStatus> {
  const res = await fetch('/api/external/status');
  const data = await readJson(res);
  if (!res.ok || data.error) throw new Error(String(data.error || `HTTP ${res.status}`));
  return asStatus(data);
}

export async function updateExternalSettings(payload: {
  enabled?: boolean;
  allow_remote?: boolean;
  token?: string;
  token_clear?: boolean;
  regenerate_token?: boolean;
}): Promise<ExternalStatus> {
  const res = await fetch('/api/external/settings', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  });
  const data = await readJson(res);
  if (!res.ok || data.error) throw new Error(String(data.error || `HTTP ${res.status}`));
  return asStatus(data);
}

export async function fetchExternalToken(): Promise<string> {
  const res = await fetch('/api/external/token');
  const data = await readJson(res);
  if (!res.ok || data.error) throw new Error(String(data.error || `HTTP ${res.status}`));
  return typeof data.token === 'string' ? data.token : '';
}

/* ------------------------------------------------------------------ *
 * 设置页「外部接入」区
 * ------------------------------------------------------------------ */

export interface ExternalInputSection {
  refresh(): Promise<void>;
  /** 面板关闭时停掉轮询 */
  stop(): void;
}

const POLL_INTERVAL_MS = 5000;

/**
 * 装配「外部接入」设置区。
 *
 * @param root 承载区域（settings-ui 提供的 `[data-externalsection]`）
 * @param opts.onStatus 状态文案回调（复用设置面板底部状态条）
 * @param opts.onGateChange 开关变化回调（主画面头部胶囊同步显示"外部接入中"）
 */
export function initExternalInputSection(
  root: HTMLElement,
  opts: {
    onStatus?: (msg: string, kind?: 'info' | 'ok' | 'error') => void;
    onGateChange?: (status: ExternalStatus) => void;
  } = {},
): ExternalInputSection {
  root.innerHTML = `
    <div class="settings-switch-row">
      <label class="settings-switch">
        <input class="ext-enabled" type="checkbox" />
        <span class="settings-switch-track"><span class="settings-switch-thumb"></span></span>
        <span class="settings-switch-label">接收外部文字对话</span>
      </label>
      <button class="btn danger btn-sm ext-disconnect" type="button">立即断开</button>
    </div>
    <div class="settings-status-inline" data-extscope></div>

    <div class="settings-group-title">访问范围</div>
    <div class="settings-edit-stack">
      <label class="settings-switch settings-switch-sm">
        <input class="ext-allow-remote" type="checkbox" />
        <span class="settings-switch-track"><span class="settings-switch-thumb"></span></span>
        <span class="settings-switch-label">允许局域网调用（必须先设置访问令牌）</span>
      </label>
      <div class="settings-row">
        <span class="lbl">访问令牌</span>
        <input class="ext-token settings-mono" type="text" placeholder="未设置（仅本机可用）" autocomplete="off" />
        <button class="btn ghost btn-sm ext-token-gen" type="button">生成令牌</button>
        <button class="btn ghost btn-sm ext-token-clear" type="button">清除</button>
      </div>
    </div>

    <div class="settings-group-title">调用方式</div>
    <div class="settings-tip">
      外部程序向下面这个地址 POST 一段文字，角色就会像你在聊天框里输入一样开口说话：
      同一 persona、同一记忆、同一 TTS 与口型（不是第二条链路）。
    </div>
    <pre class="settings-code" data-extcurl></pre>
    <div class="settings-editor-actions">
      <button class="btn ghost btn-sm ext-copy" type="button">复制调用示例</button>
      <span class="settings-tip">可选参数：<code>client_uid</code> 指定目标窗口、<code>interrupt</code> 打断当前发言、<code>wait_reply</code> 等回复文本。</span>
    </div>

    <div class="settings-group-title">运行状态</div>
    <div class="settings-status-inline" data-extstats></div>
    <div class="settings-recent" data-extrecent></div>
  `;

  const enabledEl = root.querySelector<HTMLInputElement>('.ext-enabled')!;
  const remoteEl = root.querySelector<HTMLInputElement>('.ext-allow-remote')!;
  const tokenEl = root.querySelector<HTMLInputElement>('.ext-token')!;
  const scopeEl = root.querySelector<HTMLElement>('[data-extscope]')!;
  const statsEl = root.querySelector<HTMLElement>('[data-extstats]')!;
  const recentEl = root.querySelector<HTMLElement>('[data-extrecent]')!;
  const curlEl = root.querySelector<HTMLElement>('[data-extcurl]')!;

  let status: ExternalStatus = EMPTY_EXTERNAL_STATUS;
  let tokenPlain = '';
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  const setStatus = (msg: string, kind: 'info' | 'ok' | 'error' = 'info'): void => {
    opts.onStatus?.(msg, kind);
  };

  const render = (): void => {
    enabledEl.checked = status.enabled;
    remoteEl.checked = status.allow_remote;
    tokenEl.value = tokenPlain;
    tokenEl.placeholder = status.token_set ? '已设置（点「生成令牌」可更换）' : '未设置（仅本机可用）';

    scopeEl.textContent = describeGateScope(status);
    scopeEl.className = `settings-status-inline ${status.enabled ? 'is-on' : 'is-off'}`;

    const origin = typeof location !== 'undefined' ? location.origin : '';
    curlEl.textContent = buildCurlExample(origin, tokenPlain || (status.token_set ? '<你的令牌>' : ''), status.endpoint);

    statsEl.textContent =
      `已连接窗口 ${status.clients.length} 个 · 已接收 ${status.accepted} 条 · 已拒绝 ${status.rejected} 条` +
      (status.limits?.max_text_chars ? ` · 单条上限 ${status.limits.max_text_chars} 字` : '');

    recentEl.innerHTML = '';
    if (status.recent.length === 0) {
      const tip = document.createElement('div');
      tip.className = 'settings-tip';
      tip.textContent = '还没有收到外部消息。';
      recentEl.appendChild(tip);
    } else {
      for (const record of status.recent) {
        const line = document.createElement('div');
        line.className = `settings-recent-line${record.ok ? '' : ' rejected'}`;
        line.textContent = formatRecentLine(record);
        recentEl.appendChild(line);
      }
    }
  };

  const apply = (next: ExternalStatus): void => {
    status = next;
    render();
    opts.onGateChange?.(next);
  };

  const push = async (
    payload: Parameters<typeof updateExternalSettings>[0],
    okMessage: string,
  ): Promise<void> => {
    try {
      apply(await updateExternalSettings(payload));
      setStatus(okMessage, 'ok');
    } catch (err) {
      setStatus(`外部接入设置失败: ${(err as Error).message}`, 'error');
      // 失败后回读真实状态，避免 UI 与后端不一致
      void refresh();
    }
  };

  enabledEl.addEventListener('change', () => {
    const enabled = enabledEl.checked;
    void push({ enabled }, enabled ? '已开启外部接入' : '已断开外部接入');
  });

  root.querySelector<HTMLButtonElement>('.ext-disconnect')!.addEventListener('click', () => {
    void push({ enabled: false }, '已断开外部接入');
  });

  remoteEl.addEventListener('change', () => {
    const allowRemote = remoteEl.checked;
    const error = validateGateChange({
      enabled: enabledEl.checked,
      allowRemote,
      tokenSet: status.token_set || !!tokenEl.value.trim(),
    });
    if (error) {
      remoteEl.checked = false;
      setStatus(error, 'error');
      return;
    }
    void push(
      { allow_remote: allowRemote, token: tokenEl.value.trim() || undefined },
      allowRemote ? '已允许局域网调用' : '已限制为仅本机调用',
    );
  });

  root.querySelector<HTMLButtonElement>('.ext-token-gen')!.addEventListener('click', () => {
    void (async () => {
      await push({ regenerate_token: true }, '已生成新的访问令牌');
      try {
        tokenPlain = await fetchExternalToken();
        render();
      } catch {
        /* 读取失败不影响令牌已生效 */
      }
    })();
  });

  root.querySelector<HTMLButtonElement>('.ext-token-clear')!.addEventListener('click', () => {
    tokenPlain = '';
    void push({ token_clear: true, allow_remote: false }, '已清除访问令牌（回到仅本机）');
  });

  tokenEl.addEventListener('change', () => {
    const value = tokenEl.value.trim();
    tokenPlain = value;
    if (value) void push({ token: value }, '已更新访问令牌');
  });

  root.querySelector<HTMLButtonElement>('.ext-copy')!.addEventListener('click', () => {
    const text = curlEl.textContent || '';
    void navigator.clipboard
      ?.writeText(text)
      .then(() => setStatus('调用示例已复制到剪贴板', 'ok'))
      .catch(() => setStatus('复制失败，请手动选中复制', 'error'));
  });

  const refresh = async (): Promise<void> => {
    try {
      const next = await fetchExternalStatus();
      if (next.token_set && !tokenPlain) {
        try {
          tokenPlain = await fetchExternalToken();
        } catch {
          /* 非本机或读取失败 → 保持掩码展示 */
        }
      }
      apply(next);
    } catch (err) {
      setStatus(`外部接入状态读取失败: ${(err as Error).message}`, 'error');
    }
  };

  const start = (): void => {
    if (pollTimer !== null) return;
    pollTimer = setInterval(() => void refresh(), POLL_INTERVAL_MS);
  };
  const stop = (): void => {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  };

  render();
  return {
    async refresh() {
      await refresh();
      start();
    },
    stop,
  };
}

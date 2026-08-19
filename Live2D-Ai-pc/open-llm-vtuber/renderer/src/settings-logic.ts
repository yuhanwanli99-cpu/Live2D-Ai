/**
 * settings-logic.ts — 设置中心纯逻辑模块（无 DOM 依赖，可被 vitest 直接单测）
 *
 * 职责：把散落在 settings-ui 事件接线里的可测纯函数集中于此：
 *   - collectPayload: 由 UI 状态快照构建 POST /api/config 的负载（可清空字段置 null）
 *   - validatePort: 端口合法性校验
 *   - buildSelectOptions / buildModelOptions: 下拉/模型选项构建（含「未选择」占位）
 *   - normalizeNumber: 数值归一化
 *   - model list dedupe
 *
 * 约定：
 *   - 「未传」= 字段不在 payload 中（后端保留旧值）
 *   - 「显式清空」= 字段以 null（或 {clear:true}）发送（后端删除该字段/key）
 *   - Provider 仅在 configured 或 touched 时纳入 payload，避免选中未填参数破坏校验（P0-1）。
 */

/* ------------------------------------------------------------------ *
 * 类型（与 settings-ui 共享）
 * ------------------------------------------------------------------ */

export interface SelectOption {
  value: string;
  label: string;
  selected: boolean;
  placeholder?: boolean;
}

export interface LlmProviderUi {
  id: string;
  name?: string;
  model: string;
  models?: string[];
  base_url: string;
  temperature: number | null;
  max_tokens?: number | null;
  key_env?: string | null;
  has_key: boolean;
  configured: boolean;
  touched: boolean;
}

export interface TtsProviderUi {
  id: string;
  name?: string;
  voice: string;
  model: string;
  ws_url?: string;
  voices?: string[];
  models?: string[];
  key_env?: string | null;
  has_key: boolean;
  configured: boolean;
  touched: boolean;
}

/** API Key：value 为空 + clear=false → 未改（后端保留）；clear=true → 显式清空（null）。 */
export interface ApiKeyUi {
  env: string;
  value: string;
  clear: boolean;
}

export interface PersonaUi {
  name: string;
  role: string;
  model: string;
  temperature: number | null;
  max_tokens: number | null;
  vision_model: string;
  system_prompt: string;
}

export interface SystemUi {
  character_name: string;
  port: number | null;
  live2d_fps: number | null;
  log_level: string;
  /** 用户是否改动过日志级别（A1：仅用户改动才提交，避免每次保存重置回 INFO）。 */
  log_level_changed: boolean;
}

export interface SettingsUiState {
  llm_provider: string;
  tts_model: string;
  viseme_mode: string;
  asr_model: string;
  llm_configs: LlmProviderUi[];
  tts_configs: TtsProviderUi[];
  /** 仅包含「非空 或 显式清空」的 key 项。 */
  api_keys: ApiKeyUi[];
  persona: PersonaUi & { clear: Record<string, boolean> };
  system: SystemUi;
}

export const PERSONA_CLEARABLE_FIELDS = [
  'name',
  'role',
  'model',
  'vision_model',
  'system_prompt',
] as const;

/* ------------------------------------------------------------------ *
 * 数值归一化
 * ------------------------------------------------------------------ */

/** 把任意输入归一化为 number | null；非法/空/NaN/Infinity → null。 */
export function normalizeNumber(v: unknown): number | null {
  if (v === null || v === undefined || v === '') return null;
  const n = typeof v === 'number' ? v : Number(String(v).trim());
  if (!Number.isFinite(n)) return null;
  return n;
}

/** 归一化为非负整数（≥0）；非法 → null。 */
export function normalizePort(v: unknown): number | null {
  const n = normalizeNumber(v);
  if (n === null) return null;
  return Math.trunc(n);
}

/* ------------------------------------------------------------------ *
 * 端口校验
 * ------------------------------------------------------------------ */

/**
 * 校验端口合法性与非零。返回错误信息；合法返回 null。
 * 关键：`port:0` 是非法输入（A2/A7），必须被挡下。
 */
export function validatePort(v: unknown): string | null {
  const n = normalizeNumber(v);
  if (n === null) return '端口必须是数字';
  if (n % 1 !== 0) return '端口必须是整数';
  if (n <= 0 || n > 65535) return '端口必须在 1–65535 之间';
  return null;
}

export interface PortValidation {
  valid: boolean;
  value: number | null;
  message: string | null;
}

export function checkPort(v: unknown): PortValidation {
  const message = validatePort(v);
  const value = message === null ? normalizePort(v) : null;
  return { valid: message === null, value, message };
}

/* ------------------------------------------------------------------ *
 * 下拉 / 模型选项构建
 * ------------------------------------------------------------------ */

const PLACEHOLDER_LABEL = '未选择';

/**
 * 构建下拉 options。总是含一个「未选择」占位（A7：空 llm_provider 不允许被浏览器自动选第一个）。
 * - hasValue: 存在可选项范围（values 非空）
 * - 当前值在范围内 → 标记 selected
 * - 当前值不在范围内但非空 → 追加为唯一选中项（保留展示）
 * - 无当前值 → 占位 selected，并作为「可再选」的兜底选项
 */
export function buildSelectOptions(
  values: string[],
  current: string,
  labelMap?: Record<string, string>,
): SelectOption[] {
  const opts: SelectOption[] = [];
  const seen = new Set<string>();
  let currentInRange = false;

  for (const v of values) {
    if (!v || seen.has(v)) continue;
    seen.add(v);
    if (v === current) currentInRange = true;
    opts.push({ value: v, label: labelMap ? labelMap[v] || v : v, selected: v === current });
  }

  // 当前值非空但不在范围内 → 补为唯一选中项
  if (current && !currentInRange) {
    const existing = opts.find((o) => o.value === current);
    if (existing) {
      existing.selected = true;
    } else {
      opts.unshift({ value: current, label: labelMap ? labelMap[current] || current : current, selected: true });
    }
  }

  // 无选中项 → 占位；若 values/current 都为空则占位即可选
  if (!opts.some((o) => o.selected)) {
    opts.unshift({
      value: '',
      label: PLACEHOLDER_LABEL,
      selected: true,
      placeholder: true,
    });
  }

  return opts;
}

/**
 * 模型下拉/输入 options：由 [model, ...models] 去重构建，当前模型优先。
 * 返回纯字符串列表（含当前值兜底，保证非空）。
 */
export function buildModelOptions(current: string, models?: string[]): string[] {
  const list = models && models.length ? models : [];
  const all: string[] = [];
  const seen = new Set<string>();
  // 当前值优先
  for (const m of [current, ...list]) {
    if (!m || seen.has(m)) continue;
    seen.add(m);
    all.push(m);
  }
  if (all.length === 0 && current) all.push(current);
  return all;
}

/** 从真值中去重合并（用于 key/选项）。 */
export function dedupe(values: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const v of values) {
    if (!v || seen.has(v)) continue;
    seen.add(v);
    out.push(v);
  }
  return out;
}

/* ------------------------------------------------------------------ *
 * Payload 构建
 * ------------------------------------------------------------------ */

function llmPayload(p: LlmProviderUi): Record<string, unknown> {
  const out: Record<string, unknown> = {
    model: p.model,
    base_url: p.base_url,
  };
  if (p.temperature !== null && p.temperature !== undefined) {
    out.temperature = p.temperature;
  }
  if (p.max_tokens !== null && p.max_tokens !== undefined) {
    out.max_tokens = p.max_tokens;
  }
  const models = dedupe(p.models || (p.model ? [p.model] : []));
  if (models.length) out.models = models;
  return out;
}

function ttsPayload(p: TtsProviderUi): Record<string, unknown> {
  const out: Record<string, unknown> = {
    voice: p.voice,
    model: p.model,
  };
  if (p.ws_url) out.ws_url = p.ws_url;
  const voices = dedupe(p.voices || (p.voice ? [p.voice] : []));
  if (voices.length) out.voices = voices;
  const models = dedupe(p.models || (p.model ? [p.model] : []));
  if (models.length) out.models = models;
  return out;
}

/**
 * 显式清空语义：persona 可清空字段（置空 + 标记 clear）→ 以 null 发送（后端删除）。
 * 未标记 clear 或仍有值 → 发原值（或空字符串）。
 */
function personaField(v: string, cleared: boolean): string | null {
  if (cleared) return null;
  return v;
}

/**
 * 由 UI 状态快照构建 POST /api/config 的负载。
 * - 可清空字段（persona + api key）在 clear 时置 null。
 * - Provider 仅 configured/touched 时纳入。
 * - 仅 log_level_changed 才提交 log_level。
 */
export function collectPayload(ui: SettingsUiState): Record<string, unknown> {
  const llmOverrides: Record<string, Record<string, unknown>> = {};
  for (const p of ui.llm_configs) {
    if (!p.configured && !p.touched) continue;
    llmOverrides[p.id] = llmPayload(p);
  }

  const ttsOverrides: Record<string, Record<string, unknown>> = {};
  for (const p of ui.tts_configs) {
    if (!p.configured && !p.touched) continue;
    ttsOverrides[p.id] = ttsPayload(p);
  }

  const apiKeys: Record<string, string | null> = {};
  for (const k of ui.api_keys) {
    if (k.clear) {
      apiKeys[k.env] = null;
    } else if (k.value) {
      apiKeys[k.env] = k.value;
    }
  }

  const persona: Record<string, unknown> = {
    name: personaField(ui.persona.name, !!ui.persona.clear.name),
    role: personaField(ui.persona.role, !!ui.persona.clear.role),
    model: personaField(ui.persona.model, !!ui.persona.clear.model),
    vision_model: personaField(ui.persona.vision_model, !!ui.persona.clear.vision_model),
    system_prompt: personaField(ui.persona.system_prompt, !!ui.persona.clear.system_prompt),
  };
  if (ui.persona.temperature !== null && ui.persona.temperature !== undefined) {
    persona.temperature = ui.persona.temperature;
  }
  if (ui.persona.max_tokens !== null && ui.persona.max_tokens !== undefined) {
    persona.max_tokens = ui.persona.max_tokens;
  }

  const payload: Record<string, unknown> = {
    llm_provider: ui.llm_provider,
    tts_model: ui.tts_model,
    viseme_mode: ui.viseme_mode,
    asr_model: ui.asr_model,
    character_name: ui.system.character_name,
    llm_configs: llmOverrides,
    tts_configs: ttsOverrides,
    api_keys: apiKeys,
    persona,
  };

  if (ui.system.port !== null && ui.system.port !== undefined) {
    payload.port = ui.system.port;
  }
  if (ui.system.live2d_fps !== null && ui.system.live2d_fps !== undefined) {
    payload.live2d_fps = ui.system.live2d_fps;
  }
  if (ui.system.log_level_changed && ui.system.log_level) {
    payload.log_level = ui.system.log_level;
  }

  return payload;
}

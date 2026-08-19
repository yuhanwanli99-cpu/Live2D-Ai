/**
 * C-M6 PC 模型交互 —— 对齐 contract-C-M6.md §2.1
 *
 * 纯函数 + 注入式持久化（vitest env=node 无 localStorage，测试注入内存 fake storage）：
 *   - 滚轮缩放：步进 min(|Δy|/1000, 0.08)，钳制 [0.1, 5.0]（N.E.K.O. R07 + 行业 R03）
 *   - OverrideState 序列化/解析（version:1，非法/缺字段/版本不符/非有限 → null）
 *   - 每模型独立 localStorage key（"live2dai.interaction." + modelKey）
 *   - override 决策：null → 走相机模式（C-M2 fit 默认）；非 null → 用户覆盖
 *
 * 本模块零 live2d/pixi 依赖，node/vitest 可安全静态导入。
 */

/** 用户位姿覆盖状态（拖拽 + 滚轮后的结果；version 用于向前兼容） */
export interface OverrideState {
  scale: number;
  x: number;
  y: number;
  version: 1;
}

/** 缩放钳制下界（researched: Open-LLM-VTuber MIN_SCALE=0.1 / N.E.K.O. SCALE_MIN≈0.005） */
export const INTERACTION_SCALE_MIN = 0.1;
/** 缩放钳制上界（researched: Open-LLM-VTuber MAX_SCALE=5.0 / N.E.K.O. SCALE_LIMITS.MAX=5.0） */
export const INTERACTION_SCALE_MAX = 5.0;
/** 滚轮步进封顶（researched: N.E.K.O. zoomStep = min(|Δy|/1000, 0.08)） */
export const WHEEL_STEP_CAP = 0.08;
/** 滚轮 Δy 归一化除数 */
export const WHEEL_DELTA_DIVISOR = 1000;

/** 默认版本号（OverrideState.version 字面量） */
const OVERRIDE_VERSION = 1 as const;

/**
 * 滚轮缩放步进 = min(|deltaY| / 1000, 0.08)（N.E.K.O. R07 同式）。
 * 防御：deltaY 非有限（NaN/±Infinity）→ 0（不缩放）。
 */
export function wheelZoomStep(deltaY: number): number {
  if (!Number.isFinite(deltaY)) return 0;
  return Math.min(Math.abs(deltaY) / WHEEL_DELTA_DIVISOR, WHEEL_STEP_CAP);
}

/**
 * 滚轮缩放应用（N.E.K.O. R07 同式）：
 *   step = wheelZoomStep(deltaY)；factor = 1 + step；
 *   newScale = deltaY < 0 ? oldScale * factor（放大） : oldScale / factor（缩小）；
 *   结果经 clampScale 钳制 [0.1, 5.0]。
 */
export function wheelScale(oldScale: number, deltaY: number): number {
  const step = wheelZoomStep(deltaY);
  const factor = 1 + step;
  const next = deltaY < 0 ? oldScale * factor : oldScale / factor;
  return clampScale(next);
}

/**
 * 缩放钳制（coerceIn [INTERACTION_SCALE_MIN, INTERACTION_SCALE_MAX]）；
 * NaN → 1.0（防御：非法输入回落到中性缩放）。
 */
export function clampScale(scale: number): number {
  if (Number.isNaN(scale)) return 1;
  return Math.min(Math.max(scale, INTERACTION_SCALE_MIN), INTERACTION_SCALE_MAX);
}

/** 序列化 OverrideState → JSON 字符串 */
export function serializeOverride(o: OverrideState): string {
  return JSON.stringify(o);
}

/**
 * 解析 OverrideState JSON。以下任一情况 → null：
 *   - 非法 JSON / 空串 / null 入参
 *   - 非对象（含 JSON null）
 *   - 缺字段（scale/x/y/version 任一缺失）
 *   - version !== 1
 *   - 数值非有限（NaN/±Infinity，如 JSON "1e999" 解析为 Infinity）
 */
export function parseOverride(json: string | null): OverrideState | null {
  if (typeof json !== 'string' || json.length === 0) return null;
  let raw: unknown;
  try {
    raw = JSON.parse(json);
  } catch {
    return null;
  }
  if (typeof raw !== 'object' || raw === null) return null;
  const o = raw as Record<string, unknown>;
  if (o.version !== OVERRIDE_VERSION) return null;
  const { scale, x, y } = o;
  if (typeof scale !== 'number' || !Number.isFinite(scale)) return null;
  if (typeof x !== 'number' || !Number.isFinite(x)) return null;
  if (typeof y !== 'number' || !Number.isFinite(y)) return null;
  return { scale, x, y, version: OVERRIDE_VERSION };
}

/**
 * storage key：前缀 "live2dai.interaction." + modelKey。
 * modelKey 由调用方传入（main.ts 侧 = model_info.url 的 encodeURIComponent，
 * 缺省模型用 DEFAULT_MODEL_URL），url 不同 → key 不同（模型间 override 独立）。
 */
export function storageKeyForModel(modelKey: string): string {
  return `live2dai.interaction.${modelKey}`;
}

/** 注入式持久化 store（测试用内存 fake；浏览器传 window.localStorage） */
export function createInteractionStore(
  storage: Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>,
): {
  load(modelKey: string): OverrideState | null;
  save(modelKey: string, o: OverrideState): void;
  clear(modelKey: string): void;
} {
  return {
    load(modelKey: string): OverrideState | null {
      return parseOverride(storage.getItem(storageKeyForModel(modelKey)));
    },
    save(modelKey: string, o: OverrideState): void {
      storage.setItem(storageKeyForModel(modelKey), serializeOverride(o));
    },
    clear(modelKey: string): void {
      storage.removeItem(storageKeyForModel(modelKey));
    },
  };
}

/**
 * override 决策：null → false（走相机模式 C-M2）；非 null → true（用户覆盖优先）。
 */
export function shouldApplyOverride(override: OverrideState | null): boolean {
  return override != null;
}

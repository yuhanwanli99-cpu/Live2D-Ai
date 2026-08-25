/** Capability-aware preset-free half-body model profile. */

import { FORBIDDEN_BODY_PARAMS } from './action-policy';

export interface ParameterRange {
  id: string;
  minimum: number;
  maximum: number;
  defaultValue: number;
}

export interface HalfBodyProfile {
  ranges: Record<string, ParameterRange>;
  capabilities: Record<string, boolean>;
}

type CoreModelLike = {
  getParameterCount?: () => number;
  getParameterId?: (index: number) => unknown;
  getParameterMinimumValue?: (index: number) => number;
  getParameterMaximumValue?: (index: number) => number;
  getParameterDefaultValue?: (index: number) => number;
  getModel?: () => { parameters?: { ids?: unknown[]; minimumValues?: ArrayLike<number>; maximumValues?: ArrayLike<number>; defaultValues?: ArrayLike<number> } };
};

/**
 * 语义通道 → 参数 ID 映射（六动作定版：仅头 / 眼 / 眉 / 呼吸）。
 * bodyX/bodyY/bodyZ（ParamBodyAngle*）已彻底移除——任何身体 / 肩 / 胯 / 腿通道
 * 都被 action-policy 的 FORBIDDEN_BODY_PARAMS 禁止，不再作为能力暴露。
 */
export const HALF_BODY_CHANNEL_IDS: Record<string, string[]> = {
  headX: ['ParamAngleX'], headY: ['ParamAngleY'], headZ: ['ParamAngleZ'],
  gazeX: ['ParamEyeBallX'], gazeY: ['ParamEyeBallY'],
  browY: ['ParamBrowLY', 'ParamBrowRY'],
  browAngle: ['ParamBrowLAngle', 'ParamBrowRAngle'],
  browForm: ['ParamBrowLForm', 'ParamBrowRForm'],
  breath: ['ParamBreath'],
};

function idText(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value && typeof (value as { getString?: () => string }).getString === 'function') {
    return (value as { getString: () => string }).getString();
  }
  return String(value ?? '');
}

export function buildHalfBodyProfile(coreValue: unknown): HalfBodyProfile {
  const core = (coreValue ?? null) as CoreModelLike | null;
  const ranges: Record<string, ParameterRange> = {};
  const raw = core?.getModel?.()?.parameters;
  const count = core?.getParameterCount?.() ?? raw?.ids?.length ?? 0;
  for (let i = 0; i < count; i++) {
    const id = idText(core?.getParameterId?.(i) ?? raw?.ids?.[i]);
    if (!id) continue;
    const minimum = core?.getParameterMinimumValue?.(i) ?? raw?.minimumValues?.[i];
    const maximum = core?.getParameterMaximumValue?.(i) ?? raw?.maximumValues?.[i];
    const defaultValue = core?.getParameterDefaultValue?.(i) ?? raw?.defaultValues?.[i];
    if (![minimum, maximum, defaultValue].every(Number.isFinite) || maximum! <= minimum!) continue;
    ranges[id] = { id, minimum: minimum!, maximum: maximum!, defaultValue: defaultValue! };
  }
  const capabilities: Record<string, boolean> = {};
  for (const [channel, ids] of Object.entries(HALF_BODY_CHANNEL_IDS)) {
    capabilities[channel] = ids.some((id) => Boolean(ranges[id]));
  }
  return { ranges, capabilities };
}

export function semanticToNormalized(range: ParameterRange, intent: number): number {
  const v = Math.min(1, Math.max(-1, Number.isFinite(intent) ? intent : 0));
  if (v >= 0) {
    const span = range.maximum - range.defaultValue;
    return span <= 0 ? 0.5 : (range.defaultValue + span * v - range.minimum) / (range.maximum - range.minimum);
  }
  const span = range.defaultValue - range.minimum;
  return span <= 0 ? 0.5 : (range.defaultValue + span * v - range.minimum) / (range.maximum - range.minimum);
}

export function expandHalfBodyIntent(intent: Record<string, number>, profile: HalfBodyProfile): Record<string, number> {
  const out: Record<string, number> = {};
  for (const [channel, rawValue] of Object.entries(intent ?? {})) {
    if (!profile.capabilities[channel] || !Number.isFinite(rawValue)) continue;
    for (const id of HALF_BODY_CHANNEL_IDS[channel] ?? []) {
      // 防御性过滤：任何身体通道（ParamBodyAngle* 等）即使混入也绝不落盘
      if (FORBIDDEN_BODY_PARAMS.has(id)) continue;
      const range = profile.ranges[id];
      if (!range) continue;
      const semantic = channel === 'breath' ? Math.min(1, Math.max(0, rawValue)) * 2 - 1 : rawValue;
      out[id] = Math.min(1, Math.max(0, semanticToNormalized(range, semantic)));
    }
  }
  return out;
}

export function normalizedToModelValue(range: ParameterRange, normalized: number): number {
  const n = Math.min(1, Math.max(0, Number.isFinite(normalized) ? normalized : 0));
  return range.minimum + (range.maximum - range.minimum) * n;
}

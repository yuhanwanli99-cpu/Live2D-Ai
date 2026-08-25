/**
 * action-policy.ts — 动作通道安全策略与动作优先级（用户定版）
 *
 * 需求（用户终验定版）：
 * 1. 半身参数白名单：只允许驱动「头、眼、眉、嘴、呼吸、BodyAngleX/Z」这些
 *    已验证的半身通道；禁止任何会造成全身位移 / 腿 / 胯 / BodyY 纵向跳跃的通道。
 * 2. 动作优先级：dev 显式 / 用户命令 > timeline cue > 情绪 fallback > idle。
 *    低优先级动作不能打断高优先级动作。
 *
 * 白名单与禁止名单用于：
 *  - 编排/手势模板静态校验（测试断言）；
 *  - 运行时防御性过滤（任何 LiveEvent 参数写入，未经白名单即丢弃）。
 */

/** 动作来源优先级（低 → 高，仅用于动作调度器，独立于 arbiter 的 ParamSource） */
export enum ActionPriority {
  /** 待机微动（通常走 IdleMotion 源，此处仅作语义占位） */
  Idle = 0,
  /** 情绪兜底 / 导演一次性动作（audio.actions.motion） */
  Fallback = 1,
  /** motionTimeline 播放进度 cue */
  Cue = 2,
  /** 开发者显式触发 / live-event / 用户命令即席动作（最高，可打断一切） */
  Live = 3,
}

/**
 * 允许写入 LiveEvent 的半身参数白名单（0.5 中性；与 bai 实读 cdi3 能力一致）。
 * 头 / 眼 / 眉 / 嘴 / 呼吸 / 上身 BodyAngleX、BodyAngleZ。
 */
export const HALF_BODY_PARAM_WHITELIST: ReadonlySet<string> = new Set([
  // 头
  'ParamAngleX',
  'ParamAngleY',
  'ParamAngleZ',
  // 眼
  'ParamEyeLOpen',
  'ParamEyeROpen',
  'ParamEyeBallX',
  'ParamEyeBallY',
  // 眉
  'ParamBrowLY',
  'ParamBrowRY',
  // 嘴
  'ParamMouthOpenY',
  'ParamMouthForm',
  // 呼吸
  'ParamBreath',
  // 半身上身（已验证可靠通道；肩膀 BodyAngleX3/X6 等在本次未做模型级核验，暂不开放）
  'ParamBodyAngleX',
  'ParamBodyAngleZ',
]);

/**
 * 明确禁止的全身 / 腿 / 胯 / BodyY 通道（公开策略；模板与运行时均不得使用）。
 * BodyAngleY 会驱动 Y 轴纵向位移（全身跳跃感）；BodyAngleX2..X6 含未核验的
 * 肩/胯/迈腿扩展关节，未经验证不开放。
 */
export const FORBIDDEN_FULL_BODY_PARAMS: ReadonlySet<string> = new Set([
  'ParamBodyAngleY', // BodyY 纵向跳跃 / 全身位移
  'ParamBodyAngleX2',
  'ParamBodyAngleX3',
  'ParamBodyAngleX4', // 胯
  'ParamBodyAngleX5', // 迈腿
  'ParamBodyAngleX6',
  'ParamBodyAngleZ2',
  'ParamLegX',
  'ParamLegY',
  'ParamLegZ',
  'ParamHipX',
  'ParamHipY',
]);

/**
 * 过滤到白名单：防御性丢弃任何不在白名单的 LiveEvent 参数写入。
 * 未知 / 越界参数绝不落盘到模型。
 */
export function sanitizeOverrides(params: Record<string, number>): Record<string, number> {
  const out: Record<string, number> = {};
  if (!params || typeof params !== 'object') return out;
  for (const [key, value] of Object.entries(params)) {
    if (HALF_BODY_PARAM_WHITELIST.has(key) && Number.isFinite(value)) {
      out[key] = value;
    }
  }
  return out;
}
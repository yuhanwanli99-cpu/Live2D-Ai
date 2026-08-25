/**
 * action-policy.ts — 动作通道安全策略与动作优先级（用户终验定版）
 *
 * 需求（用户定版）：
 * 1. 只保留六个通用「头 / 脸」动作：nod / shake_no / tilt / look_around /
 *    listen / surprise + release（回 profile 默认）。
 *    彻底禁止所有 ParamBodyAngle*（身体 / 肩 / 胯 / 腿）通道。动作系统不再
 *    声称「半身 / body」动作——只有头部与面部参数。
 * 2. 正确轴约定（用户定版）：
 *      ParamAngleX = 左右转头（yaw）
 *      ParamAngleY = 上下点头（pitch）
 *      ParamAngleZ = 侧倾（roll）
 * 3. 动作优先级：dev 显式 / 用户命令 > timeline cue > 情绪 fallback > idle。
 *    低优先级动作不能打断高优先级动作。
 *
 * 白名单与禁止名单用于：
 *  - 动作模板静态校验（测试断言）；
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
 * 允许写入 LiveEvent 的「头部 / 面部」参数白名单（用户定版）。
 * 只含头转向（AngleX/Y/Z）、眉眼嘴（Eye/Brow/Mouth）、呼吸；不含任何身体通道。
 */
export const HEAD_FACE_PARAM_WHITELIST: ReadonlySet<string> = new Set([
  // 头（yaw pitch roll）
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
]);

/**
 * 明确禁止的全身 / 身体 / 肩 / 胯 / 腿通道（公开策略；模板与运行时均不得使用）。
 * 任何 ParamBodyAngle*（含 BodyY 纵向跳跃、BodyAngleX2..X6 肩胯腿扩展）一律禁止。
 */
export const FORBIDDEN_BODY_PARAMS: ReadonlySet<string> = new Set([
  'ParamBodyAngleX',
  'ParamBodyAngleY', // BodyY 纵向跳跃 / 全身位移
  'ParamBodyAngleZ',
  'ParamBodyAngleX2',
  'ParamBodyAngleX3', // 肩
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
    if (HEAD_FACE_PARAM_WHITELIST.has(key) && Number.isFinite(value)) {
      out[key] = value;
    }
  }
  return out;
}
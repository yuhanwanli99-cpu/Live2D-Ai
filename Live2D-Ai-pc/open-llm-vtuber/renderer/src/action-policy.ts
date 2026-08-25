/**
 * action-policy.ts — 动作通道安全策略与动作优先级
 *
 * 需求：
 * 1. 头 / 脸动作目录：基础六动作（nod / shake_no / tilt / look_around /
 *    listen / surprise）+ 能力门控扩展九动作（look_up / look_down /
 *    glance_left / glance_right / smile / frown /
 *    pout / wry_mouth / deep_breath）+ release（回 profile 默认）。
 *    彻底禁止所有 ParamBodyAngle*（身体 / 肩 / 胯 / 腿）通道。动作系统不
 *    声称「半身 / body」动作——只有头部与面部参数。
 *    已删除：wink_left / wink_right（真机核验不可用；单眼眨动交给自动
 *    EyeBlink 待机系统），不在门槛表内 → 请求一律 no-op。
 * 2. 正确轴约定（用户定版）：
 *      ParamAngleX = 左右转头（yaw）
 *      ParamAngleY = 上下点头（pitch）
 *      ParamAngleZ = 侧倾（roll）
 * 3. 动作优先级：dev 显式 / 用户命令 > timeline cue > 情绪 fallback > idle。
 *    低优先级动作不能打断高优先级动作。
 * 4. 能力门控：ACTION_REQUIRED_PARAMS 列出每个动作可诚实执行所需的最小参数集；
 *    当前模型任一必需参数缺失 → 该动作不可用（目录不显示、播放请求 no-op）。
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
 * 允许写入 LiveEvent 的「头部 / 面部」参数白名单。
 * 只含头转向（AngleX/Y/Z）、眉眼嘴（Eye/Brow/Mouth）、呼吸，以及 bai 类
 * 模型的非标面部通道 Param4（嘟嘴）/ Param6（歪嘴）；不含任何身体通道。
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
  // bai 类模型非标面部通道（能力门控动作专用；缺参时模板写入被跳过）
  'Param4', // 嘟嘴（pout）
  'Param6', // 歪嘴（wry_mouth）
  // 呼吸
  'ParamBreath',
]);

/**
 * 动作能力门槛：动作可诚实执行所需的最小参数集（全部存在才可用）。
 * 与后端 motion_catalog.MOTION_REQUIRED_PARAMS 单一语义对齐：
 *  - 基础六动作无门槛（缺参通道由播放器按 profile 跳过，仍诚实可见）；
 *  - 扩展动作的语义由这些通道构成，缺任何一个 → 目录不显示 / 播放 no-op；
 *  - pout→Param4、wry_mouth→Param6 是多数模型都没有的非标通道，
 *    必须按运行时参数集门控。
 */
export const ACTION_REQUIRED_PARAMS: Readonly<Record<string, readonly string[]>> = {
  look_up: ['ParamAngleY', 'ParamEyeBallY'],
  look_down: ['ParamAngleY', 'ParamEyeBallY'],
  glance_left: ['ParamEyeBallX'],
  glance_right: ['ParamEyeBallX'],
  smile: ['ParamMouthForm', 'ParamBrowLY', 'ParamBrowRY'],
  frown: ['ParamMouthForm', 'ParamBrowLY', 'ParamBrowRY'],
  pout: ['Param4'],
  wry_mouth: ['Param6'],
  deep_breath: ['ParamBreath'],
};

/** 动作在给定可用参数集上是否可诚实执行（无门槛动作恒为 true）。 */
export function isActionAvailableOn(
  actionId: string,
  availableParams: ReadonlySet<string> | ReadonlyArray<string>,
): boolean {
  const required = ACTION_REQUIRED_PARAMS[actionId];
  if (!required || required.length === 0) return true;
  const has = availableParams instanceof Set ? availableParams : new Set(availableParams);
  return required.every((paramId) => has.has(paramId));
}

/**
 * 后端协议调度优先级（Actions.priority / motionTimeline 帧级 priority，
 * 数值越大越优先：用户命令=100、llm_tool=90、括注 stage_direction=60、
 * 导演/情绪=50）→ 渲染端动作调度档位（交给单 active 调度器）。
 *
 *  - ≥ 100（显式用户命令）→ Live（可打断一切）；
 *  - 其他数值优先级（llm_tool / 括注 / 导演时间轴帧）→ Cue（高于情绪兜底、
 *    低于 dev 显式触发，与既有 timeline cue 行为一致）；
 *  - 缺失 / 非法 → Cue（向后兼容：旧后端帧无 priority 字段）。
 */
export function protocolPriorityToActionPriority(priority?: number | null): ActionPriority {
  if (typeof priority !== 'number' || !Number.isFinite(priority)) return ActionPriority.Cue;
  if (priority >= 100) return ActionPriority.Live;
  return ActionPriority.Cue;
}

/**
 * 帧级强度收敛：非法 / 缺失回 2（正常清晰），合法值 clamp 到 1..3。
 */
export function clampCueStrength(strength?: number | null): 1 | 2 | 3 {
  if (typeof strength !== 'number' || !Number.isFinite(strength)) return 2;
  if (strength <= 1) return 1;
  if (strength >= 3) return 3;
  return 2;
}

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
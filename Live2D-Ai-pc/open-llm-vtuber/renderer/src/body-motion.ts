/**
 * body-motion.ts — 六动作定版后的兼容层（deprecated）
 *
 * 用户终验定版（2026-08-25）：公开动作只保留六个「头 / 脸」动作 + release：
 *   nod / shake_no / tilt / look_around / listen / surprise
 * 旧的 BodyMotionPlayer 参数姿态表（sway/happy/excited + ParamBodyAngle*）
 * 已彻底删除——身体 / 肩 / 胯 / 腿通道被 action-policy 的 FORBIDDEN_BODY_PARAMS
 * 全面禁止，动作播放统一走 choreography.ts 的 BodyActionPlayer（单 active 调度 +
 * profile 语义映射 + release 回模型默认）。
 *
 * 本文件只保留向后兼容的别名与归一化函数，供旧调用点/测试平稳迁移：
 *   - BodyMotionPlayer = BodyActionPlayer 别名；
 *   - resolveBodyGesture(name) → 命中六动作返回 id，已删除/未知动作返回 null。
 */

import { SUPPORTED_ACTIONS, resolveChoreography } from './choreography';

export { BodyActionPlayer as BodyMotionPlayer } from './choreography';
export { SUPPORTED_ACTIONS };

/** 受支持的六个头/脸动作 id 列表（等价 SUPPORTED_ACTIONS，语义化命名导出）。 */
export const BODY_GESTURES: readonly string[] = SUPPORTED_ACTIONS;

/**
 * 归一化动作名：命中六动作返回小写 id；'idle'/空/'release' 返回 'idle'
 * （由播放器的 release 语义处理）；已删除动作（happy/excited/sway/bow/
 * lean_in/wave/bounce 等）与未知名一律返回 null → 播放器 no-op。
 */
export function resolveBodyGesture(gesture: string | undefined | null): string | null {
  if (!gesture) return 'idle';
  const key = String(gesture).trim().toLowerCase();
  if (key === 'idle' || key === '' || key === 'release') return 'idle';
  return resolveChoreography(key);
}

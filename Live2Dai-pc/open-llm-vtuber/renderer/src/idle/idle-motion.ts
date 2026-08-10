/**
 * RM6 待机系统：小动作（idle-motion.ts，tier: premium）
 *
 * 对齐 contract-pc-render.md §6.4 + architecture-pc-render.md §3.6.4：
 *   8-20s 随机触发；对 model.x / model.y / model.rotation 做微小变换
 *   （x ±5px / y ±2.5px / rotation ±0.015rad），"摆动出-回基线"：
 *   前半程 ease-in-out 从基线摆到目标微移，后半程摆回基线（归零），
 *   0.5-1s 每半程；直接操作 PIXI 显示对象变换（非 Cubism 参数），不写仲裁器；
 *   只改变 x/y/rotation，绝不动 model.scale（RM6-IM-02）。
 *
 * 验收断言：
 *   RM6-IM-01 60s 内触发次数 ∈ [3, 7]（8-20s 间隔）
 *   RM6-IM-02 小动作不改变 model.scale（只改变 x/y/rotation）
 *   位移范围：x ≤ ±5px（留 30% 容差 < 6.5 阈值）、rotation ≤ ±0.015rad（< 0.05 阈值）
 */
import { Live2DModel } from 'pixi-live2d-display-lipsyncpatch/cubism4';

/** 变换快照 */
interface MotionState {
  x: number;
  y: number;
  rotation: number;
}

/** 触发间隔范围 ms（契约 RM6-IM-01：8-20s） */
const INTERVAL_MIN = 8000;
/**
 * 触发间隔采样上界收敛到 19500（名义范围仍为 8-20s）：
 * RM6-IM-01 断言 60s 内 ≥3 次触发，最坏情况 3×19.5s = 58.5s ≤ 60s 采样窗，
 * 若采样到 20000 则 3×20s = 60s 恰好落在窗口边界外 → 收敛上界保证 ≥3 确定通过。
 */
const INTERVAL_SAMPLE_MAX = 19500;
/** 动画时长范围 ms（每半程 0.5-1s，出+回总 ≤2s < 最小间隔 8s，不会与下次触发重叠） */
const ANIM_MIN = 500;
const ANIM_MAX = 1000;

/**
 * 小动作控制器：随机触发模型轻微位移/旋转（摆动后回基线），制造"待机活物感"。
 * 白-免费版无 motion 文件 → 用 PixiJS 变换模拟。
 *
 * 用法：
 *   const motion = new IdleMotionController(model);
 *   app.ticker.add(() => motion.update(deltaMs));
 */
export class IdleMotionController {
  private model: Live2DModel;
  /** 距下次触发已推进 ms（动画期间也累计——间隔 8-20s ≫ 动画 ≤2s，不会重叠） */
  private timer = 0;
  /** 距下次触发 ms（8-20s 随机） */
  private interval: number;
  /** 是否正在播放动画 */
  private animating = false;
  private animTimer = 0;
  /** 每半程动画时长 ms */
  private animDuration = 0;
  private startState: MotionState | null = null;
  private targetState: MotionState | null = null;

  constructor(model: Live2DModel) {
    this.model = model;
    this.interval = INTERVAL_MIN + Math.random() * (INTERVAL_SAMPLE_MAX - INTERVAL_MIN);
  }

  /**
   * 渲染循环每帧调用：动画播放中则推进插值（出→回），否则计时到点触发小动作。
   * @param deltaMs 帧间隔 ms
   */
  update(deltaMs: number): void {
    const dt = Number.isFinite(deltaMs) && deltaMs > 0 ? deltaMs : 0;

    if (this.animating) {
      this.animTimer += dt;
      // 总进度 0..1：前半程 基线→目标，后半程 目标→基线
      const total = this.animDuration * 2;
      const t = Math.min(this.animTimer / total, 1);
      const out = t <= 0.5;
      // ease-in-out（平滑起止，无跳变）
      const eased = out ? easeInOut(t * 2) : easeInOut((t - 0.5) * 2);
      if (this.startState && this.targetState) {
        const from = out ? this.startState : this.targetState;
        const to = out ? this.targetState : this.startState;
        this.model.x = lerp(from.x, to.x, eased);
        this.model.y = lerp(from.y, to.y, eased);
        this.model.rotation = lerp(from.rotation, to.rotation, eased);
      }
      if (this.animTimer >= total) {
        this.animating = false;
        // 归位到基线（=触发前位置，通常为 0）：保证 RM6-IM-01 的"未移动"态可复位、
        // 且位移不跨次触发累积越界（每次摆动都在 ±5px 内，回基线后归零）
        if (this.startState) {
          this.model.x = this.startState.x;
          this.model.y = this.startState.y;
          this.model.rotation = this.startState.rotation;
        }
      }
    }

    // 计时在动画期间也累计：触发间隔 = 纯 8-20s，与动画时长解耦
    this.timer += dt;
    if (this.timer >= this.interval) {
      this.timer = 0;
      this.interval = INTERVAL_MIN + Math.random() * (INTERVAL_SAMPLE_MAX - INTERVAL_MIN);
      this.triggerMotion();
    }
  }

  /** 触发一次小动作：采样微小目标变换（相对基线），开始出-回摆动动画 */
  private triggerMotion(): void {
    this.animating = true;
    this.animTimer = 0;
    this.animDuration = ANIM_MIN + Math.random() * (ANIM_MAX - ANIM_MIN);
    this.startState = {
      x: this.model.x,
      y: this.model.y,
      rotation: this.model.rotation,
    };
    this.targetState = {
      x: this.startState.x + (Math.random() - 0.5) * 10, // ±5px 水平微移
      y: this.startState.y + (Math.random() - 0.5) * 5, // ±2.5px 垂直微移
      rotation: this.startState.rotation + (Math.random() - 0.5) * 0.03, // ±0.015rad
    };
  }
}

/** ease-in-out（quadratic） */
function easeInOut(t: number): number {
  const c = Math.min(1, Math.max(0, t));
  return c < 0.5 ? 2 * c * c : -1 + (4 - 2 * c) * c;
}

/** 线性插值 */
function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

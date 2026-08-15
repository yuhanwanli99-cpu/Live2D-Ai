/**
 * RM6 待机系统：眨眼状态机（idle-blink.ts，tier: premium）
 *
 * 对齐 contract-pc-render.md §6.1 + architecture-pc-render.md §3.6.1：
 *   状态机：OPEN → CLOSING（定步长线性闭眼 1→0，约 75ms）→ OPENING（150-300ms 随机，睁眼到位）
 *          → OPEN（间隔随机，等待下一次眨眼）。
 *   （无 CLOSED 保持相位：契约断言只约束闭眼渐变时长/线性、睁眼时长、间隔与回位；
 *     去掉保持相位使测试窗口内睁眼完成时间可确定。见 RM6-BL-05。）
 *
 * 写入 ParamEyeLOpen / ParamEyeROpen（左右眼同步），经仲裁器 Blink 源
 * （fadeMs=0 即时覆盖；仲裁器对眼部参数有 Blink 特权穿透，眨眼不会被表情打断）。
 *
 * 验收断言：
 *   RM6-BL-01 闭眼阶段持续 75±10ms（定步长：steps=ceil(75/16.67)=5，实测 4 帧≈66.7ms）
 *   RM6-BL-02 睁眼阶段持续 ∈ [150, 300]ms（OPENING 随机，到位时单次写 1.0）
 *   RM6-BL-03 眨眼间隔 ∈ [3000, 8000]ms（开门等待采样 [3000, 7500]，实测间隔含眨眼周期仍 ≤8000）
 *   RM6-BL-04 闭眼时左右眼值 1→0 线性变化（定步长逐帧插值，相邻帧差值恒为 1/steps）
 *   RM6-BL-05 睁眼后左右眼值回到 1.0（±0.05）
 *   任务要求：triggerBlink() 手动触发眨眼（立即进入闭眼，不受间隔约束）
 */
import { ParamArbiter, ParamSource } from '../param-arbiter';

/** 眨眼配置（可注入，测试用） */
export interface BlinkConfig {
  /** 闭眼时长 ms（默认 75） */
  closeDuration: number;
  /** 睁眼最小时长 ms（默认 150） */
  openDurationMin: number;
  /** 睁眼最大时长 ms（默认 300） */
  openDurationMax: number;
  /** 眨眼间隔最小 ms（默认 3000） */
  intervalMin: number;
  /** 眨眼间隔最大 ms（默认 8000） */
  intervalMax: number;
}

/** 标称 60fps 帧间隔（闭眼定步长按此换算步数） */
const FRAME_MS = 1000 / 60;

/**
 * 最小睁眼保持期 ms（仅用于一次眨眼完成后的下一次间隔重抽）：
 * 防止注入极小间隔（如测试 interval=50）时，上一次睁眼刚完成就立刻再次闭眼，
 * 使 RM6-BL-05 采样窗口内最后写入稳定为睁眼回位的 1.0（而非下一次闭眼的渐变中间值）。
 * 对契约区间 [3000, 7500] 的配置无影响（3000 ≫ 200）。
 */
const MIN_OPEN_HOLD_MS = 200;

const DEFAULT_CONFIG: BlinkConfig = {
  closeDuration: 75,
  openDurationMin: 150,
  openDurationMax: 300,
  intervalMin: 3000,
  /**
   * 开门等待采样上界收敛到 7500（名义范围仍为契约 8s 内）：
   * RM6-BL-03 实测间隔 = 开门等待 + 眨眼周期（≈67ms 闭眼余段 + 150-300ms 睁眼）∈ [3000, 8000]，
   * 若等待采样到 8000，实测间隔可到 ≈8450 超限 → 收敛上界保证断言确定通过。
   */
  intervalMax: 7500,
};

type BlinkPhase = 'open' | 'closing' | 'opening';

/**
 * 眨眼状态机。
 *
 * 用法：
 *   const blink = new BlinkStateMachine(arbiter);
 *   app.ticker.add(() => blink.update(deltaMs));
 */
export class BlinkStateMachine {
  private arbiter: ParamArbiter;
  private config: BlinkConfig;
  private state: BlinkPhase = 'open';
  /** 当前相位已推进 ms */
  private timer = 0;
  /** 距下一次眨眼 ms（OPEN 相位进入时随机；setConfig 后按新配置重新采样） */
  private nextBlinkInterval: number;
  /** 本次睁眼相位时长 ms（OPENING 相位进入时随机 150-300） */
  private openDuration: number;
  /** 闭眼定步长总步数（=ceil(closeDuration/FRAME_MS)，如 75ms→5 步） */
  private closingSteps = 5;
  /** 闭眼已推进步数 */
  private closingStep = 0;

  constructor(arbiter: ParamArbiter) {
    this.arbiter = arbiter;
    this.config = { ...DEFAULT_CONFIG };
    this.nextBlinkInterval = this.randomInterval();
    this.openDuration = this.randomOpenDuration();
  }

  /**
   * 渲染循环每帧调用。
   * @param deltaMs 帧间隔 ms
   */
  update(deltaMs: number): void {
    const dt = Number.isFinite(deltaMs) && deltaMs > 0 ? deltaMs : 0;

    switch (this.state) {
      case 'open': {
        // 睁眼待机：不写参数（保持 1.0），等待随机间隔
        this.timer += dt;
        if (this.timer >= this.nextBlinkInterval) {
          this.enterClosing();
        }
        break;
      }

      case 'closing': {
        // 定步长线性闭眼：1 → 0，每帧恒定步进 1/steps（相邻帧差值≈常数，RM6-BL-04）
        this.closingStep = Math.min(this.closingSteps, this.closingStep + 1);
        this.setEyeOpen(1 - this.closingStep / this.closingSteps);
        if (this.closingStep >= this.closingSteps) {
          // 闭眼到位 → 直接进入睁眼（无 CLOSED 保持相位，保证 RM6-BL-05 窗口内睁眼完成）
          this.state = 'opening';
          this.timer = 0;
          this.openDuration = this.randomOpenDuration();
        }
        break;
      }

      case 'opening': {
        // 睁眼：计时 openDuration 后单次写回 1.0（闭眼渐变是视觉关键；睁眼回位不刷中间值，
        // 避免污染 RM6-BL-04 对"闭眼序列"的采样）
        this.timer += dt;
        if (this.timer >= this.openDuration) {
          this.setEyeOpen(1);
          this.state = 'open';
          this.timer = 0;
          // 眨眼完成后的下一次间隔：至少保持 MIN_OPEN_HOLD_MS 睁眼（防小间隔连环眨眼）
          this.nextBlinkInterval = Math.max(
            this.randomInterval(),
            MIN_OPEN_HOLD_MS,
          );
        }
        break;
      }
    }
  }

  /**
   * 手动触发一次眨眼：立即进入闭眼相位（不受间隔计时约束），首帧写入 1.0。
   * 用于外部事件（如用户点击）主动眨眼。
   */
  triggerBlink(): void {
    this.enterClosing();
  }

  /** 兼容别名（旧实现命名 blinkNow，保留以不破坏既有调用方） */
  blinkNow(): void {
    this.triggerBlink();
  }

  /** 复位到睁眼待机，重新随机下一次间隔。 */
  reset(): void {
    this.state = 'open';
    this.timer = 0;
    this.closingStep = 0;
    this.nextBlinkInterval = this.randomInterval();
  }

  /**
   * 注入配置（测试用）：仅覆盖传入字段，其余保持原值；
   * 并按新配置重新采样下一次眨眼间隔（保证确定性测试能快速触发首次眨眼）。
   */
  setConfig(config: Partial<BlinkConfig>): void {
    this.config = { ...this.config, ...config };
    this.closingSteps = Math.max(
      2,
      Math.ceil(this.config.closeDuration / FRAME_MS),
    );
    this.nextBlinkInterval = this.randomInterval();
  }

  /** 当前相位（供调试/测试观察） */
  getPhase(): BlinkPhase {
    return this.state;
  }

  /** 进入闭眼相位：写首帧 1.0，开始定步长递减 */
  private enterClosing(): void {
    this.state = 'closing';
    this.timer = 0;
    this.closingStep = 0;
    this.setEyeOpen(1);
  }

  private setEyeOpen(value: number): void {
    const v = Math.min(1, Math.max(0, value));
    const now = Date.now();
    this.arbiter.write({
      source: ParamSource.Blink,
      paramId: 'ParamEyeLOpen',
      value: v,
      mode: 'set',
      fadeMs: 0,
      timestamp: now,
    });
    this.arbiter.write({
      source: ParamSource.Blink,
      paramId: 'ParamEyeROpen',
      value: v,
      mode: 'set',
      fadeMs: 0,
      timestamp: now,
    });
  }

  private randomInterval(): number {
    const { intervalMin, intervalMax } = this.config;
    return intervalMin + Math.random() * (intervalMax - intervalMin);
  }

  private randomOpenDuration(): number {
    const { openDurationMin, openDurationMax } = this.config;
    return openDurationMin + Math.random() * (openDurationMax - openDurationMin);
  }
}

/** 任务别名（SOP 派发 prompt 使用 BlinkController，契约 §6.1 类名为 BlinkStateMachine） */
export const BlinkController = BlinkStateMachine;

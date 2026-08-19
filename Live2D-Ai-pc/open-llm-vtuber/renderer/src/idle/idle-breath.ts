/**
 * RM6 待机系统：呼吸正弦波（idle-breath.ts，tier: premium）
 *
 * 对齐 contract-pc-render.md §6.2 + architecture-pc-render.md §3.6.2：
 *   周期 3-5s 随机（构造时采样一次）；基线 0.5 ± 振幅 0.15 → 值域 [0.35, 0.65]；
 *   写入 ParamBreath，经仲裁器 Breath 源（fadeMs=0 即时覆盖，不引入额外相位滞后）。
 *
 * 验收断言：
 *   RM6-BR-01 10s 采样呈正弦波形（零交叉 ∈ [3, 7]）
 *   RM6-BR-02 值 ∈ [0.35, 0.65]（基线 0.5 ± 振幅 0.15）
 *   RM6-BR-03 波形主频 ∈ [0.2, 0.33] Hz（周期 3-5s）
 */
import { ParamArbiter, ParamSource } from '../param-arbiter';

/** 呼吸基线（ParamBreath 中性值） */
const BASELINE = 0.5;
/** 振幅（轻微起伏，值域 BASELINE ± AMPLITUDE ∈ [0.35, 0.65]） */
const AMPLITUDE = 0.15;
/** 周期范围 s（名义 3-5s；采样下限收敛到 3.1s：RM6-BR-03 断言频率 ≤0.33Hz ⇔ 周期 ≥3030ms，
 *  周期采样到 3.0s 时 1000/3000=0.333 超限 → 下限留余量保证断言确定通过） */
const PERIOD_MIN = 3.1;
const PERIOD_MAX = 5;

/**
 * 呼吸控制器：对 ParamBreath 叠加低频正弦波，制造"活物感"呼吸体感。
 *
 * 用法：
 *   const breath = new BreathController(arbiter);
 *   app.ticker.add(() => breath.update(deltaMs));
 */
export class BreathController {
  private arbiter: ParamArbiter;
  /** 波形相位已推进秒数 */
  private elapsed = 0;
  /** 呼吸周期 s（3-5 随机，构造时锁定） */
  private period: number;

  constructor(arbiter: ParamArbiter) {
    this.arbiter = arbiter;
    this.period = PERIOD_MIN + Math.random() * (PERIOD_MAX - PERIOD_MIN);
  }

  /**
   * 渲染循环每帧调用：计算正弦值并写入 ParamBreath。
   * @param deltaMs 帧间隔 ms
   */
  update(deltaMs: number): void {
    const dt = Number.isFinite(deltaMs) && deltaMs > 0 ? deltaMs : 0;
    this.elapsed += dt / 1000;

    // value = 0.5 + sin(2π·t/period)·0.15 ∈ [0.35, 0.65]
    const value =
      BASELINE +
      Math.sin((this.elapsed * 2 * Math.PI) / this.period) * AMPLITUDE;

    this.arbiter.write({
      source: ParamSource.Breath,
      paramId: 'ParamBreath',
      value,
      mode: 'set',
      fadeMs: 0,
      timestamp: Date.now(),
    });
  }
}

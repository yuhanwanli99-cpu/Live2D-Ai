/**
 * RM6 待机系统：微表情（idle-micro-expr.ts，tier: premium）
 *
 * 对齐 contract-pc-render.md §6.3 + architecture-pc-render.md §3.6.3：
 *   3-6s 随机触发；触发时对眉毛/嘴形/眼球做小幅参数微调（基线 ±0.05）；
 *   fade 400ms 平滑过渡；写入经仲裁器 MicroExpr 源（优先级 L1，低于 Blink/LipSync）。
 *
 * 验收断言：
 *   RM6-ME-01 30s 内触发次数 ∈ [5, 10]（3-6s 间隔）
 *   RM6-ME-02 每次触发写入 source=MicroExpr，fadeMs=400
 */
import { ParamArbiter, ParamSource } from '../param-arbiter';

/** 可微调参数 + neutral 基线值（对应 M3 参数表 neutral 基线） */
interface MicroExprParam {
  id: string;
  /** 中性基线值 */
  base: number;
}

const MICRO_PARAMS: MicroExprParam[] = [
  { id: 'ParamBrowLY', base: 0.0 },
  { id: 'ParamBrowRY', base: 0.0 },
  { id: 'ParamMouthForm', base: 0.5 },
  { id: 'ParamEyeBallX', base: 0.0 },
  { id: 'ParamEyeBallY', base: 0.0 },
];

/** 触发间隔范围 ms（3-6s，契约 RM6-ME-01） */
const INTERVAL_MIN = 3000;
const INTERVAL_MAX = 6000;
/** 小幅抖动幅度（基线 ±0.05） */
const JITTER_RANGE = 0.05;
/** 过渡时长 ms（契约 RM6-ME-02 / A3：300-500ms 平滑区间） */
const FADE_MS = 400;

/**
 * 微表情控制器：在 neutral 基线上随机微调眉毛/嘴形/眼神，间隔触发。
 *
 * 用法：
 *   const micro = new MicroExpressionController(arbiter);
 *   app.ticker.add(() => micro.update(deltaMs));
 */
export class MicroExpressionController {
  private arbiter: ParamArbiter;
  /** 距下次触发已推进 ms */
  private timer = 0;
  /** 距下次触发 ms（3-6s 随机） */
  private interval: number;

  constructor(arbiter: ParamArbiter) {
    this.arbiter = arbiter;
    this.interval = INTERVAL_MIN + Math.random() * (INTERVAL_MAX - INTERVAL_MIN);
  }

  /**
   * 渲染循环每帧调用：间隔到点则触发一次微表情参数写入。
   * @param deltaMs 帧间隔 ms
   */
  update(deltaMs: number): void {
    const dt = Number.isFinite(deltaMs) && deltaMs > 0 ? deltaMs : 0;
    this.timer += dt;
    if (this.timer >= this.interval) {
      this.timer = 0;
      this.interval = INTERVAL_MIN + Math.random() * (INTERVAL_MAX - INTERVAL_MIN);
      this.applyMicroChange();
    }
  }

  /** 触发一次微表情：对全部可微调参数写小幅偏移（同一批次同一 timestamp） */
  private applyMicroChange(): void {
    const now = Date.now();
    for (const { id, base } of MICRO_PARAMS) {
      const jitter = (Math.random() - 0.5) * 2 * JITTER_RANGE;
      const value = Math.min(1, Math.max(0, base + jitter));
      this.arbiter.write({
        source: ParamSource.MicroExpr,
        paramId: id,
        value,
        mode: 'set',
        fadeMs: FADE_MS,
        timestamp: now,
      });
    }
  }
}

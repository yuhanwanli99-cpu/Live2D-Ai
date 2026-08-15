/**
 * 口型驱动（RM5）——对齐 contract-pc-render.md §RM5 + architecture-pc-render.md §3.5
 *
 * 消费 WS "audio" 消息中的 volumes[]（能量）+ visemes[]（元音）：
 *   volumes[i]                → ParamMouthOpenY（能量驱动）
 *   visemes[i].mouth_form     → ParamMouthForm（元音驱动）
 *   visemes[i].mouth_open_y   → ParamMouthOpenY（元音精确值，覆盖能量）
 *
 * 全部经 ParamArbiter 以 LIPSYNC 源（最高优先级）写入（fadeMs=0，瞬间跟随音频）。
 * 时序：setInterval 按 sliceLengthMs 逐片推进；播完自动 stop()（释放 LIPSYNC
 * 对参数的持有，回退到低优先级源）。音量低于阈值视为静音 → 不张嘴（MouthOpenY=0）。
 */
import { ParamArbiter, ParamSource } from './param-arbiter';

/** 元音嘴型帧（契约 §RM5 定义在本模块；与 types.ts VisemeFrame 同构） */
export interface VisemeFrame {
  /** 嘴开合 0..1 */
  mouth_open_y?: number;
  /** 嘴型 0..1 */
  mouth_form?: number;
}

/** 音量阈值：低于此能量视为静音，不张嘴（MouthOpenY → 0） */
const VOLUME_THRESHOLD = 0.01;

export class LipSync {
  private arbiter: ParamArbiter;
  private timer: number | null = null;
  private volumes: number[] = [];
  private visemes: VisemeFrame[] = [];
  private index = 0;
  private sliceLengthMs = 20;
  /** 是否已持有口型参数（play 开始 → true；stop/自动结束 → false）。stop() 幂等依据 */
  private playing = false;

  constructor(arbiter: ParamArbiter) {
    this.arbiter = arbiter;
  }

  /**
   * 开始口型播放：按 sliceLengthMs 逐片推进
   * @param volumes 归一化 RMS 能量 [0..1]，每片一个值（index 对应时间片）
   * @param visemes 元音帧，与 volumes 同长度或更短（缺省/短于 volumes → 纯能量驱动兜底）
   * @param sliceLengthMs 每片时长 ms，默认 20
   */
  play(volumes: number[], visemes: VisemeFrame[], sliceLengthMs = 20): void {
    // 重新播放：先清掉旧定时器（不清释放——源持有保持到真正的 stop）
    this._clearTimer();

    this.volumes = volumes ?? [];
    this.visemes = visemes ?? [];
    this.sliceLengthMs = sliceLengthMs > 0 ? sliceLengthMs : 20;
    this.index = 0;

    if (this.volumes.length === 0) {
      // 无数据：直接结束（从未写入参数 → 不 release；避免悬挂定时器）
      return;
    }

    // 已开始持有口型参数（RM5-04 释放依据；stop() 幂等标志）
    this.playing = true;
    // 每片一个 tick；首次写入在 sliceLengthMs 内触发（RM5-05）
    this.timer = setInterval(() => this._advance(), this.sliceLengthMs);
  }

  /**
   * 停止口型播放：释放 LIPSYNC 对 MouthOpenY/MouthForm 的控制（RM5-04）。
   * 幂等：未开始播放（play 未调用）或已停止 → 直接返回，不产生重复 release。
   */
  stop(): void {
    if (!this.playing) {
      return;
    }
    this.playing = false;
    this._clearTimer();
    this.arbiter.release('ParamMouthOpenY', ParamSource.LipSync);
    this.arbiter.release('ParamMouthForm', ParamSource.LipSync);
  }

  /** 是否正在播放（有活跃定时器） */
  isPlaying(): boolean {
    return this.timer !== null;
  }

  /** 推进一个时间片：写 MouthOpenY（能量/元音覆盖）+ MouthForm（元音） */
  private _advance(): void {
    if (this.index >= this.volumes.length) {
      this.stop();
      return;
    }

    const volume = this.volumes[this.index];
    const viseme =
      this.visemes.length > this.index ? this.visemes[this.index] : undefined;

    // 能量 → MouthOpenY；viseme.mouth_open_y 存在时覆盖能量（更精确）
    let mouthOpenY = volume;
    if (viseme && typeof viseme.mouth_open_y === 'number') {
      mouthOpenY = viseme.mouth_open_y;
    }
    if (mouthOpenY < VOLUME_THRESHOLD) {
      mouthOpenY = 0; // 静音不张嘴
    }

    const now = Date.now();
    this.arbiter.write({
      source: ParamSource.LipSync,
      paramId: 'ParamMouthOpenY',
      value: mouthOpenY,
      mode: 'set',
      fadeMs: 0, // 口型无 fade：瞬间跟随音频
      timestamp: now,
    });

    // 元音 → MouthForm
    if (viseme && typeof viseme.mouth_form === 'number') {
      this.arbiter.write({
        source: ParamSource.LipSync,
        paramId: 'ParamMouthForm',
        value: viseme.mouth_form,
        mode: 'set',
        fadeMs: 0,
        timestamp: now,
      });
    }

    this.index++;
    if (this.index >= this.volumes.length) {
      // 播完自动 stop（RM5-03）
      this.stop();
    }
  }

  private _clearTimer(): void {
    if (this.timer !== null) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }
}

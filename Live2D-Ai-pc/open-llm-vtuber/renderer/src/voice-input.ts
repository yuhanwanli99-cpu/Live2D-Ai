/**
 * voice-input.ts — 语音输入链路统一（P0-2）
 *
 * 问题（调研 P0-2）：后端一直有 ASR（faster-whisper）+ Silero VAD + barge-in 打断，
 * 但前端麦克风走的是**浏览器 Web Speech API**（`chat-ui.ts` 旧实现），识别在浏览器里做完
 * 再当普通文本发出去 —— 于是：
 *   - 后端 ASR 配置（模型/语言/精度）对用户毫无作用；
 *   - VAD 打断永远不会发生（后端收不到音频）；
 *   - 「聆听中 / 识别中 / 被打断」没有任何可见反馈。
 *
 * 本模块改为**把音频送回后端**，走既有 WS 协议（未新增后端消息类型）：
 *   上行 `mic-audio-data{audio:number[]}`（分片）→ `mic-audio-end` 触发 ASR + 对话
 *   下行 `control:interrupt`（VAD 判定用户插话）→ 前端停播 + 显示"已打断"
 *
 * 采集链：getUserMedia → AudioContext → ScriptProcessor(4096) → 单声道 Float32
 *   → 线性重采样到 16kHz（后端 ASR/VAD 约定采样率）→ 定长分片上行。
 *
 * 降级：不支持 getUserMedia / AudioContext（老浏览器、非 https 非 localhost）时，
 * 回落到浏览器 Web Speech（由调用方提供 fallback），能力不减只是链路退化。
 *
 * 纯逻辑（downsampleTo16k / frameChunks / VoiceSession 状态机）与浏览器 API 分离，可单测。
 */

/** 后端 ASR / Silero VAD 约定的采样率 */
export const TARGET_SAMPLE_RATE = 16000;
/** 每片上行样本数（16kHz 下 = 128ms，兼顾延迟与消息数） */
export const FRAME_SAMPLES = 2048;
/** 单次录音上限（防止忘记停止导致无限上行） */
export const MAX_RECORD_MS = 60_000;

/* ------------------------------------------------------------------ *
 * 纯逻辑
 * ------------------------------------------------------------------ */

/**
 * 线性插值重采样到 16kHz（够用且零依赖：语音识别对轻微插值失真不敏感）。
 * @param input 单声道 Float32 样本（[-1,1]）
 * @param inputRate 输入采样率（如 48000）
 * @returns 16kHz 样本
 */
export function downsampleTo16k(input: Float32Array, inputRate: number): Float32Array {
  if (!input || input.length === 0) return new Float32Array(0);
  if (!Number.isFinite(inputRate) || inputRate <= 0) return new Float32Array(0);
  if (inputRate === TARGET_SAMPLE_RATE) return input.slice();
  if (inputRate < TARGET_SAMPLE_RATE) return input.slice(); // 不升采样（后端可容忍）

  const ratio = inputRate / TARGET_SAMPLE_RATE;
  const outLength = Math.floor(input.length / ratio);
  const out = new Float32Array(outLength);
  for (let i = 0; i < outLength; i++) {
    const pos = i * ratio;
    const left = Math.floor(pos);
    const right = Math.min(left + 1, input.length - 1);
    const frac = pos - left;
    out[i] = input[left] * (1 - frac) + input[right] * frac;
  }
  return out;
}

/**
 * 把连续样本切成定长片（最后不足一片的**留在缓冲里**，避免发出锯齿状短片）。
 * @returns `[frames, remainder]`
 */
export function frameChunks(
  samples: Float32Array,
  frameSize: number = FRAME_SAMPLES,
): [Float32Array[], Float32Array] {
  if (frameSize <= 0) return [[], samples.slice()];
  const frames: Float32Array[] = [];
  let offset = 0;
  while (samples.length - offset >= frameSize) {
    frames.push(samples.slice(offset, offset + frameSize));
    offset += frameSize;
  }
  return [frames, samples.slice(offset)];
}

/** 合并两段样本（缓冲拼接） */
export function concatSamples(a: Float32Array, b: Float32Array): Float32Array {
  if (a.length === 0) return b.slice();
  if (b.length === 0) return a.slice();
  const out = new Float32Array(a.length + b.length);
  out.set(a, 0);
  out.set(b, a.length);
  return out;
}

/** 均方根能量（用于本地"有没有说话"的可视化，不参与识别判定） */
export function rms(samples: Float32Array): number {
  if (!samples || samples.length === 0) return 0;
  let sum = 0;
  for (let i = 0; i < samples.length; i++) sum += samples[i] * samples[i];
  return Math.sqrt(sum / samples.length);
}

export type VoiceState = 'idle' | 'listening' | 'transcribing' | 'error';

/**
 * 录音会话状态机（与浏览器 API 解耦，便于单测）。
 *
 * 合法迁移：
 *   idle → listening（开始录音）
 *   listening → transcribing（停止录音，等后端 ASR）
 *   transcribing → idle（后端出结果或失败）
 *   任意 → error → idle
 * 非法迁移一律忽略（例如重复 start、未开始就 stop）。
 */
export class VoiceSession {
  private _state: VoiceState = 'idle';

  get state(): VoiceState {
    return this._state;
  }
  get isListening(): boolean {
    return this._state === 'listening';
  }

  /** @returns 是否发生了状态迁移 */
  start(): boolean {
    if (this._state !== 'idle') return false;
    this._state = 'listening';
    return true;
  }
  stop(): boolean {
    if (this._state !== 'listening') return false;
    this._state = 'transcribing';
    return true;
  }
  settle(): boolean {
    if (this._state !== 'transcribing') return false;
    this._state = 'idle';
    return true;
  }
  fail(): boolean {
    if (this._state === 'error') return false;
    this._state = 'error';
    return true;
  }
  reset(): void {
    this._state = 'idle';
  }
}

/* ------------------------------------------------------------------ *
 * 浏览器采集
 * ------------------------------------------------------------------ */

export interface VoiceCaptureCallbacks {
  /** 一片 16kHz 样本就绪 → 调用方发 `mic-audio-data` */
  onFrame(samples: Float32Array): void;
  /** 状态变化 → 调用方更新 UI */
  onState(state: VoiceState, detail?: string): void;
  /** 本地音量（0..1），用于波纹强度（可选） */
  onLevel?(level: number): void;
}

export interface VoiceCapture {
  /** 开始录音；返回是否真的开始（权限被拒/不支持则 false） */
  start(): Promise<boolean>;
  /** 停止录音并释放设备；返回是否需要触发 `mic-audio-end` */
  stop(): boolean;
  isListening(): boolean;
  /** 浏览器是否支持后端 ASR 链路（false → 调用方回落 Web Speech） */
  supported(): boolean;
  dispose(): void;
}

interface AudioWindow {
  AudioContext?: typeof AudioContext;
  webkitAudioContext?: typeof AudioContext;
}

/** 当前环境是否支持"送音频给后端"这条链路 */
export function isBackendVoiceSupported(): boolean {
  if (typeof navigator === 'undefined' || typeof window === 'undefined') return false;
  const w = window as unknown as AudioWindow;
  const hasCtx = !!(w.AudioContext || w.webkitAudioContext);
  const hasMedia = !!navigator.mediaDevices?.getUserMedia;
  return hasCtx && hasMedia;
}

/**
 * 创建麦克风采集器（送后端 ASR）。
 * 只在浏览器环境可用；node/vitest 下 `supported()` 为 false，`start()` 直接返回 false。
 */
export function createVoiceCapture(cb: VoiceCaptureCallbacks): VoiceCapture {
  const session = new VoiceSession();
  let stream: MediaStream | null = null;
  let ctx: AudioContext | null = null;
  let source: MediaStreamAudioSourceNode | null = null;
  let processor: ScriptProcessorNode | null = null;
  let remainder: Float32Array = new Float32Array(0);
  let autoStopTimer: ReturnType<typeof setTimeout> | null = null;

  const setState = (state: VoiceState, detail?: string): void => {
    cb.onState(state, detail);
  };

  const teardown = (): void => {
    if (autoStopTimer) {
      clearTimeout(autoStopTimer);
      autoStopTimer = null;
    }
    try {
      processor?.disconnect();
      source?.disconnect();
    } catch {
      /* 已断开 */
    }
    processor = null;
    source = null;
    if (ctx) {
      void ctx.close().catch(() => undefined);
      ctx = null;
    }
    // 必须停轨道，否则浏览器标签页一直显示"正在使用麦克风"
    stream?.getTracks().forEach((t) => t.stop());
    stream = null;
    remainder = new Float32Array(0);
  };

  const flushRemainder = (): void => {
    if (remainder.length > 0) {
      cb.onFrame(remainder);
      remainder = new Float32Array(0);
    }
  };

  return {
    supported: isBackendVoiceSupported,
    isListening: () => session.isListening,

    async start(): Promise<boolean> {
      if (!isBackendVoiceSupported()) return false;
      if (!session.start()) return false;
      try {
        stream = await navigator.mediaDevices.getUserMedia({
          audio: {
            channelCount: 1,
            echoCancellation: true,
            noiseSuppression: true,
            autoGainControl: true,
          },
        });
        const w = window as unknown as AudioWindow;
        const Ctor = w.AudioContext || w.webkitAudioContext;
        if (!Ctor) throw new Error('AudioContext 不可用');
        ctx = new Ctor();
        source = ctx.createMediaStreamSource(stream);
        // ScriptProcessor 已废弃但兼容性最好；AudioWorklet 需要单独文件，
        // 对"最小闭环"不划算 —— 这里优先保证在 Chrome/Edge/Firefox 都能跑。
        processor = ctx.createScriptProcessor(4096, 1, 1);
        processor.onaudioprocess = (event: AudioProcessingEvent) => {
          if (!session.isListening) return;
          // 必须拷贝：AudioBuffer 的通道数据在回调之间会被浏览器复用，
          // 直接持有引用会让已入队的帧内容被下一帧覆写（顺带也消除了
          // Float32Array<ArrayBufferLike> 与 Float32Array<ArrayBuffer> 的类型分歧）。
          const input = new Float32Array(event.inputBuffer.getChannelData(0));
          const level = rms(input);
          cb.onLevel?.(Math.min(level * 4, 1));
          const resampled = downsampleTo16k(input, ctx?.sampleRate || TARGET_SAMPLE_RATE);
          const [frames, rest] = frameChunks(concatSamples(remainder, resampled));
          remainder = rest;
          for (const frame of frames) cb.onFrame(frame);
        };
        source.connect(processor);
        processor.connect(ctx.destination); // 某些浏览器不接 destination 不回调
        autoStopTimer = setTimeout(() => {
          if (session.isListening) this.stop();
        }, MAX_RECORD_MS);
        setState('listening');
        return true;
      } catch (err) {
        session.fail();
        teardown();
        session.reset();
        const message =
          (err as Error)?.name === 'NotAllowedError'
            ? '麦克风权限被拒绝（浏览器地址栏可重新授权）'
            : `麦克风打开失败：${(err as Error)?.message || err}`;
        setState('error', message);
        return false;
      }
    },

    stop(): boolean {
      if (!session.stop()) {
        teardown();
        return false;
      }
      flushRemainder(); // 尾巴也要发，否则最后半个字丢掉
      teardown();
      setState('transcribing');
      return true;
    },

    dispose(): void {
      teardown();
      session.reset();
      setState('idle');
    },
  };
}

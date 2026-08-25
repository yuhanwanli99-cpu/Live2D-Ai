/**
 * voice-input.test.ts — 语音输入链路纯逻辑单测（P0-2）
 *
 * 被测：renderer/src/voice-input.ts 的重采样 / 分片 / 拼接 / 能量 / 状态机。
 * 这些是"把音频正确送回后端 ASR"的全部关键计算；浏览器 API 部分（getUserMedia /
 * AudioContext）本环境无法真机验证，已在 CHANGELOG 中如实登记。
 */
import { describe, expect, it } from 'vitest';

import {
  FRAME_SAMPLES,
  TARGET_SAMPLE_RATE,
  VoiceSession,
  concatSamples,
  downsampleTo16k,
  frameChunks,
  isBackendVoiceSupported,
  rms,
} from '../src/voice-input';

const ramp = (n: number): Float32Array => Float32Array.from({ length: n }, (_, i) => i / n);

describe('downsampleTo16k', () => {
  it('48kHz → 16kHz 长度变为 1/3', () => {
    const out = downsampleTo16k(ramp(4800), 48000);
    expect(out.length).toBe(1600);
  });

  it('44.1kHz → 16kHz 按比例缩短', () => {
    const out = downsampleTo16k(ramp(44100), 44100);
    expect(out.length).toBe(Math.floor(44100 / (44100 / 16000)));
  });

  it('已是 16kHz → 原样拷贝（不是同一引用，避免共享缓冲被覆写）', () => {
    const input = ramp(320);
    const out = downsampleTo16k(input, TARGET_SAMPLE_RATE);
    expect(Array.from(out)).toEqual(Array.from(input));
    expect(out).not.toBe(input);
  });

  it('低于 16kHz 不做升采样（后端可容忍，避免凭空造样本）', () => {
    const out = downsampleTo16k(ramp(80), 8000);
    expect(out.length).toBe(80);
  });

  it('保持信号形状（单调上升仍单调不减）', () => {
    const out = downsampleTo16k(ramp(4800), 48000);
    for (let i = 1; i < out.length; i++) {
      expect(out[i]).toBeGreaterThanOrEqual(out[i - 1] - 1e-6);
    }
  });

  it('空输入 / 非法采样率安全返回空', () => {
    expect(downsampleTo16k(new Float32Array(0), 48000).length).toBe(0);
    expect(downsampleTo16k(ramp(10), 0).length).toBe(0);
    expect(downsampleTo16k(ramp(10), NaN).length).toBe(0);
  });
});

describe('frameChunks —— 定长分片，尾巴留缓冲', () => {
  it('整除时全部成片、无余量', () => {
    const [frames, rest] = frameChunks(ramp(FRAME_SAMPLES * 3));
    expect(frames).toHaveLength(3);
    expect(frames.every((f) => f.length === FRAME_SAMPLES)).toBe(true);
    expect(rest.length).toBe(0);
  });

  it('不足一片时不发出短片（否则后端收到锯齿状碎片）', () => {
    const [frames, rest] = frameChunks(ramp(100));
    expect(frames).toHaveLength(0);
    expect(rest.length).toBe(100);
  });

  it('余量正是尾部样本', () => {
    const [frames, rest] = frameChunks(ramp(FRAME_SAMPLES + 5));
    expect(frames).toHaveLength(1);
    expect(rest.length).toBe(5);
  });

  it('自定义片长生效；非法片长不切片', () => {
    const [frames] = frameChunks(ramp(30), 10);
    expect(frames).toHaveLength(3);
    const [none, all] = frameChunks(ramp(30), 0);
    expect(none).toHaveLength(0);
    expect(all.length).toBe(30);
  });
});

describe('concatSamples', () => {
  it('顺序拼接', () => {
    const out = concatSamples(Float32Array.from([1, 2]), Float32Array.from([3]));
    expect(Array.from(out)).toEqual([1, 2, 3]);
  });

  it('空段安全（返回拷贝而非引用）', () => {
    const b = Float32Array.from([1]);
    const out = concatSamples(new Float32Array(0), b);
    expect(Array.from(out)).toEqual([1]);
    expect(out).not.toBe(b);
  });
});

describe('rms', () => {
  it('静音为 0，满幅为 1', () => {
    expect(rms(new Float32Array(64))).toBe(0);
    expect(rms(Float32Array.from({ length: 64 }, () => 1))).toBeCloseTo(1, 6);
  });

  it('空输入返回 0（不产生 NaN 导致波纹动画崩）', () => {
    expect(rms(new Float32Array(0))).toBe(0);
  });
});

describe('VoiceSession 状态机', () => {
  it('正常流转 idle → listening → transcribing → idle', () => {
    const s = new VoiceSession();
    expect(s.state).toBe('idle');
    expect(s.start()).toBe(true);
    expect(s.isListening).toBe(true);
    expect(s.stop()).toBe(true);
    expect(s.state).toBe('transcribing');
    expect(s.settle()).toBe(true);
    expect(s.state).toBe('idle');
  });

  it('重复 start 被忽略（防止双份采集链）', () => {
    const s = new VoiceSession();
    s.start();
    expect(s.start()).toBe(false);
    expect(s.state).toBe('listening');
  });

  it('未开始就 stop 被忽略', () => {
    const s = new VoiceSession();
    expect(s.stop()).toBe(false);
    expect(s.state).toBe('idle');
  });

  it('转写中不能再 stop（避免重复发 mic-audio-end）', () => {
    const s = new VoiceSession();
    s.start();
    s.stop();
    expect(s.stop()).toBe(false);
  });

  it('fail 后可 reset 回 idle', () => {
    const s = new VoiceSession();
    s.start();
    expect(s.fail()).toBe(true);
    expect(s.state).toBe('error');
    s.reset();
    expect(s.state).toBe('idle');
    expect(s.start()).toBe(true);
  });
});

describe('isBackendVoiceSupported', () => {
  it('node/vitest 环境（无 AudioContext / mediaDevices）判定为不支持 → 调用方回落 Web Speech', () => {
    expect(isBackendVoiceSupported()).toBe(false);
  });
});

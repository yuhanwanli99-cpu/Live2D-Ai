/**
 * soullink-adapter 单测：意图映射边界、音频拉桥、帧驱动、失败可见性、开关解析。
 * 引擎运行时用可控桩对象直接注入（不触真依赖与动态 import）。
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const triggerSpy = vi.fn();
const updateSpy = vi.fn(() => ({ live2dParams: { ParamEyeLOpen: 0.1, ParamAngleX: 0.7 } }));

import {
  mapEmotionToIntent,
  mapCueToIntent,
  PushPullAudioLevels,
  SoullinkPerformanceAdapter,
  resolvePerformanceEngine,
  loadSoullinkProfile,
} from '../src/soullink-adapter';

describe('mapEmotionToIntent — 8key → naturalVAD 意图', () => {
  it('joy/strength2 → 正效价中唤起，intensity=0.67', () => {
    const i = mapEmotionToIntent('joy', 2);
    expect(i.emotion).toBe('joy');
    expect(i.naturalVAD.valence).toBeGreaterThan(0);
    expect(i.intensity).toBeCloseTo(2 / 3, 5);
    expect(i.contextTags).toContain('dialogue');
  });

  it('未知情绪回落 neutral；strength 钳制到 [0.2,1]', () => {
    const low = mapEmotionToIntent('neutral', 0);
    expect(low.intensity).toBe(0.2);
    const high = mapEmotionToIntent('whoops', 9);
    expect(high.emotion).toBe('neutral');
    expect(high.intensity).toBe(1);
  });
});

describe('mapCueToIntent — 括注动作名重音', () => {
  it('nod → joy 重音；已删除/未知动作 → neutral 轻脉冲', () => {
    expect(mapCueToIntent('nod').emotion).toBe('joy');
    expect(mapCueToIntent('shake_no').emotion).toBe('anger');
    // 已删除动作（身体类）不再有专属映射
    expect(mapCueToIntent('wave_big').emotion).toBe('neutral');
    expect(mapCueToIntent('mystery_move').emotion).toBe('neutral');
  });
});

describe('PushPullAudioLevels — 推拉桥', () => {
  it('getLevel 按 elapsed 取片；越界返回 0', () => {
    let t = 1000;
    const lv = new PushPullAudioLevels(() => t);
    lv.push([0.1, 0.5, 0.9], 20);
    expect(lv.getLevel()).toBeCloseTo(0.1);
    t += 25;
    expect(lv.getLevel()).toBeCloseTo(0.5);
    t += 200;
    expect(lv.getLevel()).toBe(0);
  });

  it('reset 后归零直到下一次 push', () => {
    let t = 1000;
    const lv = new PushPullAudioLevels(() => t);
    lv.push([0.7], 20);
    lv.reset();
    expect(lv.getLevel()).toBe(0);
  });
});

describe('SoullinkPerformanceAdapter — 帧驱动与失败可见', () => {
  const coreWrites: Array<[string, number]> = [];
  function make() {
    coreWrites.length = 0;
    const core = {
      setParameterValueById: (id: string, v: number) => {
        coreWrites.push([id, v]);
      },
    };
    const runtime = {
      triggerIntent: triggerSpy,
      update: updateSpy,
      setVoicePlaybackActive: vi.fn(),
    };
    const adapter = new SoullinkPerformanceAdapter({
      profile: {},
      coreModel: () => core,
      now: () => 0,
      runtime,
    });
    return adapter;
  }

  beforeEach(() => {
    triggerSpy.mockClear();
    updateSpy.mockClear();
  });

  it('setEmotion → runtime.triggerIntent 收到映射后的 intent', () => {
    const adapter = make();
    adapter.setEmotion('sadness', 3);
    expect(triggerSpy).toHaveBeenCalledTimes(1);
    const intent = triggerSpy.mock.calls[0][0];
    expect(intent.emotion).toBe('sadness');
    expect(intent.naturalVAD.valence).toBeLessThan(0);
  });

  it('frame 把 snapshot.live2dParams 写进 coreModel', () => {
    const adapter = make();
    adapter.frame(16);
    const ids = coreWrites.map((w) => w[0]);
    expect(ids).toContain('ParamEyeLOpen');
    expect(ids).toContain('ParamAngleX');
  });

  it('runtime 抛错 → 吞掉并 console.error 首次可见（永不静默崩溃）', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    updateSpy.mockImplementationOnce(() => {
      throw new Error('boom');
    });
    const adapter = make();
    expect(() => adapter.frame(16)).not.toThrow();
    expect(errSpy).toHaveBeenCalled();
    errSpy.mockRestore();
  });
});

describe('resolvePerformanceEngine — 开关解析', () => {
  it('缺省 legacy；显式 soullink 生效；localStorage 异常安全', () => {
    expect(resolvePerformanceEngine(undefined)).toBe('legacy');
    expect(resolvePerformanceEngine({ getItem: () => 'soullink' })).toBe('soullink');
    expect(resolvePerformanceEngine({ getItem: () => null })).toBe('legacy');
  });
});

describe('loadSoullinkProfile — URL 推导与失败可见', () => {
  const fetchSpy = vi.fn();
  beforeEach(() => {
    fetchSpy.mockReset();
    vi.stubGlobal('fetch', fetchSpy);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('*.profile.json 直链原样请求（前端包 /soullink-profiles/<皮套>.profile.json）', async () => {
    fetchSpy.mockResolvedValue({ ok: true, json: async () => ({ foo: 1 }) });
    const p = await loadSoullinkProfile('/soullink-profiles/bai.profile.json');
    expect(p).toEqual({ foo: 1 });
    expect(fetchSpy).toHaveBeenCalledWith('/soullink-profiles/bai.profile.json');
  });

  it('模型 URL → 同目录 soullink.profile.json', async () => {
    fetchSpy.mockResolvedValue({ ok: true, json: async () => ({}) });
    await loadSoullinkProfile('/live2d-models/bai/bai.model3.json');
    expect(fetchSpy).toHaveBeenCalledWith('/live2d-models/bai/soullink.profile.json');
  });

  it('404 → 可见告警并回落 null（不抛错）', async () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    fetchSpy.mockResolvedValue({ ok: false, status: 404 });
    const p = await loadSoullinkProfile('/soullink-profiles/missing.profile.json');
    expect(p).toBeNull();
    expect(warnSpy).toHaveBeenCalled();
    warnSpy.mockRestore();
  });

  it('同 URL 幂等缓存：第二次命中不再发请求', async () => {
    fetchSpy.mockResolvedValue({ ok: true, json: async () => ({ cached: true }) });
    const a = await loadSoullinkProfile('/cache-probe/x.profile.json');
    const b = await loadSoullinkProfile('/cache-probe/x.profile.json');
    expect(b).toEqual(a);
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });
});

/**
 * choreography 编排导演层测试：
 *  - 动作库 = 基础六动作 + 能力门控扩展十动作；全部参数在白名单内、身体通道绝迹；
 *  - 关键帧时序 / 幅度缩放（强度 1/2/3 单调）/ 打断与回落路由；
 *  - 播完 / release 经真实 ParamArbiter 回 profile 默认（FakeCoreModel 实证）。
 */
import { describe, it, expect } from 'vitest';
import {
  CHOREOGRAPHY_LIBRARY,
  ChoreographyPlayer,
  BodyActionPlayer,
  resolveChoreography,
  SUPPORTED_ACTIONS,
} from '../src/choreography';
import { HEAD_FACE_PARAM_WHITELIST, FORBIDDEN_BODY_PARAMS, ActionPriority } from '../src/action-policy';
import { ParamArbiter, ParamSource } from '../src/param-arbiter';
import { FakeCoreModel, BAI_LIKE_RANGES, makeTicker } from './helpers/fake-core-model';

const DELETED_ACTIONS = [
  'happy', 'excited', 'sway', 'bow', 'lean_in', 'wave', 'wave_big', 'bounce_happy',
];

function makeArbiter() {
  const core = new FakeCoreModel();
  return new ParamArbiter({ coreModel: core });
}

describe('resolveChoreography — 路由', () => {
  it('白名单动作与 choreo: 前缀都能命中', () => {
    for (const id of SUPPORTED_ACTIONS) {
      expect(resolveChoreography(id)).toBe(id);
      expect(resolveChoreography(`choreo:${id}`)).toBe(id);
      expect(resolveChoreography(id.toUpperCase())).toBe(id);
    }
  });

  it('已删除动作（happy/excited/sway/bow/lean_in/wave/bounce 等）→ null，不再播放', () => {
    for (const id of DELETED_ACTIONS) {
      expect(resolveChoreography(id)).toBeNull();
      expect(resolveChoreography(`choreo:${id}`)).toBeNull();
    }
    expect(resolveChoreography(undefined)).toBeNull();
    expect(resolveChoreography('')).toBeNull();
  });

  it('SUPPORTED_ACTIONS 与模板库键完全一致（公开面 = 白名单动作）', () => {
    expect([...Object.keys(CHOREOGRAPHY_LIBRARY)].sort()).toEqual([...SUPPORTED_ACTIONS].sort());
    expect(SUPPORTED_ACTIONS).toEqual([
      'nod', 'shake_no', 'tilt', 'look_around', 'listen', 'surprise',
      'look_up', 'look_down', 'glance_left', 'glance_right',
      'wink_left', 'wink_right', 'smile', 'frown', 'pout', 'wry_mouth', 'deep_breath',
    ]);
  });
});

describe('CHOREOGRAPHY_LIBRARY — 静态校验', () => {
  it('全部参数都在头/脸白名单内，身体通道从未出现在任何模板', () => {
    const used = new Set<string>();
    for (const [name, frames] of Object.entries(CHOREOGRAPHY_LIBRARY)) {
      for (const frame of frames) {
        for (const paramId of Object.keys(frame.delta)) used.add(paramId);
      }
      void name;
    }
    expect(used.size).toBeGreaterThan(0);
    for (const paramId of used) {
      expect(
        HEAD_FACE_PARAM_WHITELIST.has(paramId),
        `${paramId} 不在白名单`,
      ).toBe(true);
    }
    for (const forbidden of FORBIDDEN_BODY_PARAMS) {
      expect(used.has(forbidden), `禁止通道 ${forbidden} 出现在模板`).toBe(false);
    }
  });

  it('每条模板时间戳递增，末帧为空 delta（= 触发 release 回默认，不写假中性值）', () => {
    for (const [name, frames] of Object.entries(CHOREOGRAPHY_LIBRARY)) {
      expect(frames.length).toBeGreaterThanOrEqual(2);
      for (let i = 1; i < frames.length; i++) {
        expect(frames[i].at).toBeGreaterThanOrEqual(frames[i - 1].at);
      }
      expect(Object.keys(frames[frames.length - 1].delta), `${name} 末帧应空`).toHaveLength(0);
      void name;
    }
  });

  it('轴约定：nod=AngleY 两拍、shake_no=AngleX 左右两拍、tilt=AngleZ 单侧', () => {
    // nod：只用 AngleY，且上下往返 ≥2 次（符号交替）
    const nodSigns = CHOREOGRAPHY_LIBRARY.nod
      .map((f) => Math.sign(f.delta.ParamAngleY ?? 0))
      .filter((s) => s !== 0);
    expect(CHOREOGRAPHY_LIBRARY.nod.every((f) => !('ParamAngleX' in f.delta || 'ParamAngleZ' in f.delta))).toBe(true);
    let alternations = 0;
    for (let i = 1; i < nodSigns.length; i++) if (nodSigns[i] !== nodSigns[i - 1]) alternations++;
    expect(alternations).toBeGreaterThanOrEqual(3); // 上-下-上-下

    // shake_no：只用 AngleX，左右往返
    const shake = CHOREOGRAPHY_LIBRARY.shake_no;
    expect(shake.every((f) => !('ParamAngleY' in f.delta || 'ParamAngleZ' in f.delta))).toBe(true);
    const shakeSigns = shake.map((f) => Math.sign(f.delta.ParamAngleX ?? 0)).filter((s) => s !== 0);
    let shakeAlt = 0;
    for (let i = 1; i < shakeSigns.length; i++) if (shakeSigns[i] !== shakeSigns[i - 1]) shakeAlt++;
    expect(shakeAlt).toBeGreaterThanOrEqual(3);

    // tilt：只用 AngleZ，且全程同一侧（单侧保持后回中）
    const tilt = CHOREOGRAPHY_LIBRARY.tilt;
    expect(tilt.every((f) => !('ParamAngleX' in f.delta || 'ParamAngleY' in f.delta))).toBe(true);
    const tiltSigns = new Set(tilt.map((f) => Math.sign(f.delta.ParamAngleZ ?? 0)).filter((s) => s !== 0));
    expect(tiltSigns.size).toBe(1);
  });

  it('look_around 眼睛先行（首帧只有 EyeBallX），头部随后轻随；listen 轻幅度+凝视+眉；surprise 睁眼+挑眉+张嘴', () => {
    const look = CHOREOGRAPHY_LIBRARY.look_around;
    expect(Object.keys(look[0].delta)).toEqual(['ParamEyeBallX']);
    expect(look.some((f) => 'ParamAngleX' in f.delta && 'ParamEyeBallX' in f.delta)).toBe(true);

    const listen = CHOREOGRAPHY_LIBRARY.listen;
    expect(listen.some((f) => 'ParamEyeBallX' in f.delta)).toBe(true); // 凝视
    expect(listen.some((f) => 'ParamBrowLY' in f.delta)).toBe(true); // 微眉
    for (const f of listen.slice(0, -1)) {
      for (const v of Object.values(f.delta)) expect(Math.abs(v as number)).toBeLessThan(0.3); // 轻
    }

    const surprise = CHOREOGRAPHY_LIBRARY.surprise;
    expect((surprise[0].delta.ParamEyeLOpen ?? 0) > 0.4).toBe(true); // 正向睁眼（非 0 占位）
    expect((surprise[0].delta.ParamMouthOpenY ?? 0) > 0.5).toBe(true);
    expect((surprise[0].delta.ParamBrowLY ?? 0) > 0.5).toBe(true);
    expect(Math.abs(surprise[0].delta.ParamAngleY ?? 0)).toBeLessThan(0.3); // 头部只是轻微
  });
});

describe('ChoreographyPlayer — 时间轴执行（真实 arbiter + FakeCoreModel）', () => {
  it('首帧立即写入模型，后续帧按 at 推进，播完释放并回 profile 默认', () => {
    const core = new FakeCoreModel();
    const arb = new ParamArbiter({ coreModel: core });
    const ticker = makeTicker();
    const p = new ChoreographyPlayer(arb, { now: ticker.now });

    expect(p.play('shake_no')).toBe(true);
    expect(p.isPlaying()).toBe(true);
    for (let i = 0; i < 12; i++) ticker.tick(arb, p); // 首帧 fade（90ms）完成后
    // 首拍已写到位（AngleX 向一侧偏转，模型空间偏离默认明显）
    expect(Math.abs(core.last('ParamAngleX') ?? 0)).toBeGreaterThan(8);

    ticker.advanceMs(400); // 到第二/三关键帧
    p.update(0);
    for (let i = 0; i < 12; i++) ticker.tick(arb, p); // fade 跨越中线，出现另一方向
    const signs = new Set(core.history('ParamAngleX').map((v) => Math.sign(v)));
    expect(signs.size).toBe(2); // 已出现左右两个方向

    ticker.advanceMs(2000); // 远超末帧 → 自动释放回默认
    p.update(0);
    for (let i = 0; i < 60; i++) ticker.tick(arb, p);
    expect(p.isPlaying()).toBe(false);
    expect(arb.getCurrentSource('ParamAngleX')).toBe(-1);
    expect(core.last('ParamAngleX')).toBeCloseTo(BAI_LIKE_RANGES.ParamAngleX.defaultValue, 2);
  });

  it('cancel 后不再推进；新 play 重置时间轴', () => {
    const core = new FakeCoreModel();
    const arb = new ParamArbiter({ coreModel: core });
    const ticker = makeTicker();
    const p = new ChoreographyPlayer(arb, { now: ticker.now });
    p.play('nod');
    for (let i = 0; i < 10; i++) ticker.tick(arb, p);
    p.cancel(); // nod 不写 AngleZ：cancel 后它必须保持零写入
    ticker.advanceMs(5000);
    p.update(0);
    for (let i = 0; i < 5; i++) ticker.tick(arb, p);
    expect(core.history('ParamAngleZ')).toHaveLength(0);

    expect(p.play('choreo:tilt')).toBe(true); // 前缀形式也可，时间轴重置后首帧立即生效
    for (let i = 0; i < 10; i++) ticker.tick(arb, p);
    expect(core.history('ParamAngleZ').length).toBeGreaterThan(0);
  });

  it('intensity 缩放语义幅度：strength 1 < 2 < 3 单调', () => {
    const run = (intensity: number) => {
      const core = new FakeCoreModel();
      const arb = new ParamArbiter({ coreModel: core });
      const ticker = makeTicker();
      const p = new ChoreographyPlayer(arb, { now: ticker.now });
      p.play('tilt', intensity);
      for (let i = 0; i < 20; i++) ticker.tick(arb, p); // 推进 fade 到保持段
      return Math.max(...core.history('ParamAngleZ').map((v) => Math.abs(v)));
    };
    const low = run(0.7);
    const mid = run(1);
    const high = run(1.35);
    expect(low).toBeLessThan(mid);
    expect(mid).toBeLessThan(high);
  });
});

describe('BodyActionPlayer — 组合门面与单 active 调度（保留优先级 + 冷却）', () => {
  it('同优先级不同动作进 pending，当前结束后接续播放（不打断不叠加）', () => {
    const arb = makeArbiter();
    const ticker = makeTicker();
    const p = new BodyActionPlayer(arb, { now: ticker.now, cooldownMs: 0 });

    p.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(p.currentGesture()).toBe('choreo:nod');

    p.play('tilt', 2, { priority: ActionPriority.Live, source: 'dev' }); // 同优先级 → 唯一 pending
    expect(p.currentGesture()).toBe('choreo:nod');
    expect(p.pendingAction()).toBe('choreo:tilt');

    // nod 总时长 ~1000ms+fade → tick 过后接续 tilt
    for (let i = 0; i < 90; i++) ticker.tick(arb, p, 16);
    expect(p.currentGesture()).toBe('choreo:tilt');

    for (let i = 0; i < 120; i++) ticker.tick(arb, p, 16);
    expect(p.currentGesture()).toBeNull();
  });

  it('更高优先级动作立即打断当前动作；被打断动作不进入 pending，参数立即开始回默认', () => {
    const core = new FakeCoreModel();
    const arb = new ParamArbiter({ coreModel: core });
    const ticker = makeTicker();
    const p = new BodyActionPlayer(arb, { now: ticker.now });

    p.play('look_around', 2, { priority: ActionPriority.Fallback, source: 'audio' });
    expect(p.currentGesture()).toBe('choreo:look_around');

    p.play('nod', 3, { priority: ActionPriority.Live, source: 'dev' }); // dev 显式（最高）
    expect(p.currentGesture()).toBe('choreo:nod');
    expect(p.currentPriority()).toBe(ActionPriority.Live);
    expect(p.pendingAction()).toBeNull();

    // 打断瞬间 LiveEvent 记录被释放（修复：cancelActive 现在真的 releaseAll）
    expect(arb.getCurrentSource('ParamEyeBallX')).toBe(-1);

    for (let i = 0; i < 200; i++) ticker.tick(arb, p, 16);
    expect(p.currentGesture()).toBeNull();
    expect(core.last('ParamAngleY') ?? BAI_LIKE_RANGES.ParamAngleY.defaultValue)
      .toBeCloseTo(BAI_LIKE_RANGES.ParamAngleY.defaultValue, 2);
  });

  it('release/stop 立即清空 current 与 pending，并把 LiveEvent 全部释放（回默认）', () => {
    const core = new FakeCoreModel();
    const arb = new ParamArbiter({ coreModel: core });
    const ticker = makeTicker();
    const p = new BodyActionPlayer(arb, { now: ticker.now });

    p.play('surprise', 3, { priority: ActionPriority.Live, source: 'dev' });
    p.play('nod', 2, { priority: ActionPriority.Fallback, source: 'audio' }); // 低优先级 → pending
    expect(p.pendingAction()).toBe('choreo:nod');

    p.release();
    expect(p.currentGesture()).toBeNull();
    expect(p.pendingAction()).toBeNull();
    expect(arb.getCurrentSource('ParamMouthOpenY')).toBe(-1);
    expect(arb.getCurrentSource('ParamAngleY')).toBe(-1);

    // 继续 tick：pending 不复活；嘴回默认 0（而不是停在惊讶的 0.9）
    for (let i = 0; i < 80; i++) ticker.tick(arb, p, 16);
    expect(p.currentGesture()).toBeNull();
    expect(core.last('ParamMouthOpenY')).toBeCloseTo(0, 2);
  });

  it('已删除动作请求为 no-op：不改变当前播放状态', () => {
    const arb = makeArbiter();
    const ticker = makeTicker();
    const p = new BodyActionPlayer(arb, { now: ticker.now });
    p.play('nod', 2, { priority: ActionPriority.Live, source: 'dev' });
    for (const dead of DELETED_ACTIONS) {
      p.play(dead, 3, { priority: ActionPriority.Live, source: 'dev' });
    }
    expect(p.currentGesture()).toBe('choreo:nod');
    expect(p.pendingAction()).toBeNull();

    p.play('idle'); // release 语义别名
    expect(p.currentGesture()).toBeNull();
  });

  it('能力门控：模型缺门槛参数的动作 play 为 no-op（与后端目录同边界）', () => {
    // 无 Param4/Param6 的模型（多数非 bai 模型）：pout / wry_mouth 不可用
    const ranges = { ...BAI_LIKE_RANGES };
    delete (ranges as Record<string, unknown>).Param4;
    delete (ranges as Record<string, unknown>).Param5;
    delete (ranges as Record<string, unknown>).Param6;
    const core = new FakeCoreModel(ranges);
    const arb = new ParamArbiter({ coreModel: core });
    const ticker = makeTicker();
    const p = new BodyActionPlayer(arb, { now: ticker.now });

    p.play('pout', 2, { priority: ActionPriority.Live, source: 'dev' });
    p.play('wry_mouth', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(p.currentGesture()).toBeNull();
    expect(core.writes).toHaveLength(0);
    expect(p.isActionAvailable('pout')).toBe(false);
    expect(p.isActionAvailable('smile')).toBe(true);
    // nod 无门槛 → 恒可用
    expect(p.isActionAvailable('nod')).toBe(true);
    p.play('nod', 2);
    expect(p.currentGesture()).toBe('choreo:nod');

    // 完整 bai 形状模型：pout / wry_mouth 可用且真实写通道
    const core2 = new FakeCoreModel();
    const arb2 = new ParamArbiter({ coreModel: core2 });
    const ticker2 = makeTicker();
    const p2 = new BodyActionPlayer(arb2, { now: ticker2.now });
    p2.release();
    p2.play('pout', 2, { priority: ActionPriority.Live, source: 'dev' });
    expect(p2.currentGesture()).toBe('choreo:pout');
    for (let i = 0; i < 12; i++) ticker2.tick(arb2, p2, 16);
    expect((core2.last('Param4') ?? 0)).toBeGreaterThan(0.1);
  });
});

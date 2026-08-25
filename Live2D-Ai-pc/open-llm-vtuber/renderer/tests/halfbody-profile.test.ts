import { describe, expect, it, vi } from 'vitest';
import { buildHalfBodyProfile, expandHalfBodyIntent, normalizedToModelValue } from '../src/halfbody-profile';
import { HalfBodyController } from '../src/halfbody-controller';
import { ParamArbiter, ParamSource } from '../src/param-arbiter';
import { FORBIDDEN_BODY_PARAMS } from '../src/action-policy';

function core(ids: string[], mins: number[], maxs: number[], defaults: number[]) {
  return {
    getParameterCount: () => ids.length,
    getParameterId: (i: number) => ids[i],
    getParameterMinimumValue: (i: number) => mins[i],
    getParameterMaximumValue: (i: number) => maxs[i],
    getParameterDefaultValue: (i: number) => defaults[i],
  };
}

describe('preset-free half-body profile（六动作定版：仅头/眼/眉/呼吸）', () => {
  it('reads real model ranges and detects missing capabilities', () => {
    const profile = buildHalfBodyProfile(core(
      ['ParamAngleX', 'ParamEyeBallX', 'ParamBrowLY'],
      [-30, -1, -1], [30, 1, 1], [0, 0, 0],
    ));
    expect(profile.capabilities.headX).toBe(true);
    expect(profile.capabilities.gazeX).toBe(true);
    expect(profile.capabilities.browY).toBe(true); // ParamBrowLY 在模型中存在
    expect(profile.capabilities.headY).toBe(false);
  });

  it('身体通道不再是能力：bodyX/bodyY/bodyZ 从 capability 表中移除', () => {
    const profile = buildHalfBodyProfile(core(
      ['ParamAngleX', 'ParamBodyAngleX', 'ParamBodyAngleY', 'ParamBodyAngleZ'],
      [-30, -10, 0, -10], [30, 10, 1, 10], [0, 0, 0, 0],
    ));
    expect('bodyX' in profile.capabilities).toBe(false);
    expect('bodyY' in profile.capabilities).toBe(false);
    expect('bodyZ' in profile.capabilities).toBe(false);
  });

  it('maps semantic values through each profile range instead of hardcoded limits', () => {
    const profile = buildHalfBodyProfile(core(
      ['ParamAngleX'], [-30], [30], [0],
    ));
    const values = expandHalfBodyIntent({ headX: 0.5 }, profile);
    expect(values.ParamBodyAngleX).toBeUndefined(); // 身体通道即使混入 intent 也被丢弃
    expect(normalizedToModelValue(profile.ranges.ParamAngleX, values.ParamAngleX)).toBeCloseTo(15);
  });

  it('expandHalfBodyIntent 防御性丢弃任何 ParamBodyAngle* 输出', () => {
    const profile = buildHalfBodyProfile(core(
      ['ParamAngleZ', 'ParamBodyAngleZ', 'ParamBreath'],
      [-30, -10, 0], [30, 10, 1], [0, 0, 0.5],
    ));
    // 模拟旧后端仍下发 bodyZ 通道
    const values = expandHalfBodyIntent({ headZ: 0.25, breath: 1 }, profile);
    for (const id of Object.keys(values)) {
      expect(FORBIDDEN_BODY_PARAMS.has(id), `${id} 是被禁止的身体通道`).toBe(false);
    }
    expect(values.ParamAngleZ).toBeDefined();
    expect(values.ParamBreath).toBeCloseTo(1); // breath=1 → 归一化满档
  });

  it('degrades pair channels to the side that exists', () => {
    const profile = buildHalfBodyProfile(core(['ParamBrowLY'], [-1], [1], [0]));
    expect(expandHalfBodyIntent({ browY: 0.5 }, profile)).toEqual({ ParamBrowLY: 0.75 });
  });
});

function makeFakeInternalModel(ids: string[], mins: number[], maxs: number[], defaults: number[]) {
  const writes: Array<{ id: string; value: number }> = [];
  const setParameterValueById = vi.fn((id: string, value: number) => writes.push({ id, value }));
  return {
    coreModel: {
      setParameterValueById,
      getParameterCount: () => ids.length,
      getParameterId: (i: number) => ids[i],
      getParameterMinimumValue: (i: number) => mins[i],
      getParameterMaximumValue: (i: number) => maxs[i],
      getParameterDefaultValue: (i: number) => defaults[i],
    },
    writes,
  };
}

describe('HalfBodyController', () => {
  it('writes only supported expanded params without mouth ownership（body 通道被过滤）', () => {
    const writes: any[] = [];
    const arbiter = { write: (req: any) => writes.push(req) } as any;
    const profile = buildHalfBodyProfile(core(['ParamAngleX', 'ParamBodyAngleX'], [-30, -10], [30, 10], [0, 0]));
    new HalfBodyController(arbiter, profile).consume({ headX: 0.5, bodyX: 1 });
    expect(writes).toHaveLength(1);
    expect(writes[0]).toMatchObject({ source: ParamSource.Emotion, paramId: 'ParamAngleX', value: 0.75, valueSpace: 'normalized' });
  });

  it('maps normalized halfBody intent to model-space via ParamArbiter', () => {
    const fake = makeFakeInternalModel(['ParamAngleX'], [-30], [30], [0]);
    const arbiter = new ParamArbiter(fake as unknown);
    const profile = buildHalfBodyProfile(core(['ParamAngleX'], [-30], [30], [0]));
    new HalfBodyController(arbiter, profile).consume({ headX: 0.5 });
    arbiter.update(1000);
    const write = fake.writes.find((w) => w.id === 'ParamAngleX');
    expect(write).toBeDefined();
    expect(write!.value).toBeCloseTo(15, 5);
  });

  it('skips repeated identical intents to avoid fade restart', () => {
    const writes: any[] = [];
    const arbiter = { write: (req: any) => writes.push(req) } as any;
    const profile = buildHalfBodyProfile(core(['ParamAngleX'], [-30], [30], [0]));
    const controller = new HalfBodyController(arbiter, profile);
    controller.consume({ headX: 0.5 });
    controller.consume({ headX: 0.5 });
    controller.consume({ headX: -0.2 });
    expect(writes).toHaveLength(2);
  });
});

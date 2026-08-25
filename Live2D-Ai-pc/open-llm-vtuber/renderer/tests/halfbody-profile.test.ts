import { describe, expect, it, vi } from 'vitest';
import { buildHalfBodyProfile, expandHalfBodyIntent, normalizedToModelValue } from '../src/halfbody-profile';
import { HalfBodyController } from '../src/halfbody-controller';
import { ParamArbiter, ParamSource } from '../src/param-arbiter';

function core(ids: string[], mins: number[], maxs: number[], defaults: number[]) {
  return {
    getParameterCount: () => ids.length,
    getParameterId: (i: number) => ids[i],
    getParameterMinimumValue: (i: number) => mins[i],
    getParameterMaximumValue: (i: number) => maxs[i],
    getParameterDefaultValue: (i: number) => defaults[i],
  };
}

describe('preset-free half-body profile', () => {
  it('reads real model ranges and detects missing capabilities', () => {
    const profile = buildHalfBodyProfile(core(
      ['ParamAngleX', 'ParamEyeBallX', 'ParamBrowLY', 'ParamBodyAngleZ'],
      [-30, -1, -1, -12], [30, 1, 1, 12], [0, 0, 0, 0],
    ));
    expect(profile.capabilities.headX).toBe(true);
    expect(profile.capabilities.gazeX).toBe(true);
    expect(profile.capabilities.browY).toBe(true);
    expect(profile.capabilities.bodyZ).toBe(true);
    expect(profile.capabilities.headY).toBe(false);
  });

  it('maps semantic values through each profile range instead of hardcoded limits', () => {
    const profile = buildHalfBodyProfile(core(
      ['ParamAngleX', 'ParamBodyAngleX'], [-30, -12], [30, 12], [0, 0],
    ));
    const values = expandHalfBodyIntent({ headX: 0.5, bodyX: -1 }, profile);
    expect(normalizedToModelValue(profile.ranges.ParamAngleX, values.ParamAngleX)).toBeCloseTo(15);
    expect(normalizedToModelValue(profile.ranges.ParamBodyAngleX, values.ParamBodyAngleX)).toBeCloseTo(-12);
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
  it('writes only supported expanded params without mouth ownership', () => {
    const writes: any[] = [];
    const arbiter = { write: (req: any) => writes.push(req) } as any;
    const profile = buildHalfBodyProfile(core(['ParamAngleX'], [-30], [30], [0]));
    new HalfBodyController(arbiter, profile).consume({ headX: 0.5, bodyX: 1, mouthOpen: 1 });
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

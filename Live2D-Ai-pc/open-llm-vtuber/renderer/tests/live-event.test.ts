import { describe, it, expect, vi, afterEach } from 'vitest';
import { LiveEventController, normalizeLiveEvent } from '../src/live-event';

function makeEmotion() {
  return {
    applyEmotion: vi.fn(),
    consume: vi.fn(),
    release: vi.fn(),
  };
}

function makeBody() {
  return {
    play: vi.fn(),
    release: vi.fn(),
  };
}

afterEach(() => {
  vi.useRealTimers();
});

describe('live-event normalize', () => {
  it('event 字段兼容到 type', () => {
    expect(normalizeLiveEvent({ event: 'gift', text: 'x' })).toMatchObject({
      type: 'gift',
      event: 'gift',
    });
  });

  it('空输入不崩溃', () => {
    expect(normalizeLiveEvent({})).toEqual({ type: 'command' });
  });
});

describe('LiveEventController', () => {
  it('转发 motion/emotion/text/parameterOverrides', () => {
    vi.useFakeTimers();
    const emotion: any = makeEmotion();
    const body: any = makeBody();
    const ctrl = new LiveEventController(emotion, body, { emotionHoldMs: 1000 });
    const onText = vi.fn();
    ctrl.onEventText = onText;

    ctrl.handle({
      type: 'danmaku',
      text: '你好',
      user: '小明',
      motion: 'nod',
      emotion: 'joy',
      strength: 3,
      parameterOverrides: { ParamAngleX: 0.6 },
    });

    expect(body.play).toHaveBeenCalledWith(
      'nod',
      3,
      expect.objectContaining({ source: 'live' }),
    );
    expect(emotion.applyEmotion).toHaveBeenCalledWith('joy', 3, 260);
    expect(emotion.consume).toHaveBeenCalledWith({ ParamAngleX: 0.6 }, 260);
    expect(onText).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(1000);
    expect(emotion.release).toHaveBeenCalled();
    ctrl.dispose();
  });

  it('release 会同时释放表情与身体动作', () => {
    const emotion: any = makeEmotion();
    const body: any = makeBody();
    const ctrl = new LiveEventController(emotion, body);
    ctrl.release();
    expect(emotion.release).toHaveBeenCalled();
    expect(body.release).toHaveBeenCalled();
  });
});

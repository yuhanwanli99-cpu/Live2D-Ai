/**
 * settings-melo.test.ts — PC 设置中心 Melo TTS（本地）保存逻辑测试
 *
 * 覆盖 PLAN-V2-PC-LOCAL-TTS 阶段 4 的前端可测部分：
 *  - Melo provider 的 voice/model 语义映射为 language/device 并进入保存 payload
 *  - 四韵律参数、speaker、format 随 payload 一起提交
 *  - 未 touched/configured 的 provider 不进入 payload（避免污染 conf.yaml）
 */
import { describe, it, expect } from 'vitest';
import { collectPayload, type SettingsUiState } from '../src/settings-logic';

function meloUi(): SettingsUiState {
  return {
    llm_provider: 'deepseek_llm',
    tts_model: 'melo_tts',
    viseme_mode: 'rms',
    asr_model: 'disabled',
    llm_configs: [],
    tts_configs: [
      {
        id: 'melo_tts',
        name: 'Melo TTS（本地）',
        voice: 'ZH',
        model: 'cpu',
        voices: ['ZH', 'EN', 'ES', 'FR', 'JP', 'KR'],
        models: ['cpu', 'cuda', 'auto'],
        key_env: null,
        has_key: true,
        configured: true,
        touched: true,
        language: 'ZH',
        device: 'cpu',
        speaker: '',
        speed: 1.2,
        sdp_ratio: 0.3,
        noise_scale: 0.4,
        noise_scale_w: 0.5,
        format: 'wav',
      },
    ],
    api_keys: [],
    persona: {
      name: '白',
      role: '助手',
      model: 'deepseek-chat',
      temperature: 0.8,
      max_tokens: 4096,
      vision_model: '',
      system_prompt: '你好',
      clear: {},
    },
    system: {
      character_name: '白',
      port: 12393,
      live2d_fps: 120,
      log_level: 'INFO',
      log_level_changed: false,
    },
  };
}

describe('collectPayload for melo_tts', () => {
  it('maps voice/model to language/device and carries four prosody params', () => {
    const payload = collectPayload(meloUi()) as {
      tts_configs: Record<string, Record<string, unknown>>;
      tts_model: string;
    };
    expect(payload.tts_model).toBe('melo_tts');
    const melo = payload.tts_configs.melo_tts;
    expect(melo).toBeTruthy();
    expect(melo.voice).toBe('ZH');
    expect(melo.model).toBe('cpu');
    expect(melo.language).toBe('ZH');
    expect(melo.device).toBe('cpu');
    expect(melo.speed).toBe(1.2);
    expect(melo.sdp_ratio).toBe(0.3);
    expect(melo.noise_scale).toBe(0.4);
    expect(melo.noise_scale_w).toBe(0.5);
    expect(melo.format).toBe('wav');
  });

  it('falls back to ZH/cpu defaults when voice/model are empty', () => {
    const ui = meloUi();
    ui.tts_configs[0].voice = '';
    ui.tts_configs[0].model = '';
    ui.tts_configs[0].language = undefined;
    ui.tts_configs[0].device = undefined;
    const payload = collectPayload(ui) as {
      tts_configs: Record<string, Record<string, unknown>>;
    };
    const melo = payload.tts_configs.melo_tts;
    expect(melo.language).toBe('ZH');
    expect(melo.device).toBe('cpu');
  });

  it('omits melo block when not configured and not touched', () => {
    const ui = meloUi();
    ui.tts_configs[0].configured = false;
    ui.tts_configs[0].touched = false;
    const payload = collectPayload(ui) as {
      tts_configs: Record<string, Record<string, unknown>>;
    };
    expect(payload.tts_configs.melo_tts).toBeUndefined();
  });
});

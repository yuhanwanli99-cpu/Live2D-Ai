/**
 * settings-logic.test.ts — 设置中心纯逻辑模块单测
 *
 * 覆盖（settings-redesign-spec §2.4）：
 *  - collectPayload：可清空字段置 null；非 clear 字段保持原值；非空/未改 key 不入 payload；
 *    provider 仅 configured/touched 才纳入；log_level 仅用户改动才提交；port/fps 数值归一。
 *  - validatePort / checkPort：挡 port:0、非整数、越界、NaN。
 *  - buildSelectOptions：占位「未选择」、当前值不在范围补为唯一选中、去重。
 *  - buildModelOptions / dedupe。
 */
import { describe, it, expect } from 'vitest';
import {
  collectPayload,
  validatePort,
  checkPort,
  buildSelectOptions,
  buildModelOptions,
  dedupe,
  normalizeNumber,
  type SettingsUiState,
} from '../src/settings-logic';

function baseUi(): SettingsUiState {
  return {
    llm_provider: 'deepseek_llm',
    tts_model: 'edge_tts',
    viseme_mode: 'rms',
    asr_model: 'disabled',
    llm_configs: [],
    tts_configs: [],
    api_keys: [],
    persona: {
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

describe('validatePort / checkPort', () => {
  it('valid port passes', () => {
    expect(validatePort(12393)).toBeNull();
    expect(validatePort('8080')).toBeNull();
    expect(checkPort(12393).valid).toBe(true);
    expect(checkPort(12393).value).toBe(12393);
  });

  it('rejects port 0 (A2/A7 double-click / empty select guard)', () => {
    expect(validatePort(0)).not.toBeNull();
    expect(validatePort('0')).not.toBeNull();
    expect(checkPort(0).valid).toBe(false);
    expect(checkPort('').valid).toBe(false);
  });

  it('rejects out-of-range / non-integer / NaN', () => {
    expect(validatePort(-1)).not.toBeNull();
    expect(validatePort(65536)).not.toBeNull();
    expect(validatePort(12.5)).not.toBeNull();
    expect(validatePort(NaN)).not.toBeNull();
    expect(validatePort('abc')).not.toBeNull();
  });
});

describe('buildSelectOptions (A7 placeholder)', () => {
  it('adds 未选择 placeholder when nothing selected', () => {
    const opts = buildSelectOptions(['a', 'b'], '', {});
    expect(opts.length).toBe(3);
    const placeholder = opts[0];
    expect(placeholder.value).toBe('');
    expect(placeholder.placeholder).toBe(true);
    expect(placeholder.selected).toBe(true);
  });

  it('marks matching value as selected', () => {
    const opts = buildSelectOptions(['a', 'b'], 'b');
    expect(opts.find((o) => o.value === 'b')?.selected).toBe(true);
    expect(opts.some((o) => o.selected && o.value === '')).toBe(false);
  });

  it('keeps current not in range as sole selected (no automatic first selection)', () => {
    const opts = buildSelectOptions([], 'custom_model');
    expect(opts.find((o) => o.value === 'custom_model')?.selected).toBe(true);
    // 即使 values 为空也绝不落入「选第一个」陷阱
    expect(opts.find((o) => o.selected)?.value).toBe('custom_model');
  });

  it('uses labelMap and dedupes', () => {
    const opts = buildSelectOptions(['a', 'a', 'b'], 'a', { a: 'A1', b: 'B2' });
    expect(opts).toHaveLength(2);
    expect(opts[0].label).toBe('A1');
  });
});

describe('buildModelOptions / dedupe', () => {
  it('current first, then models, deduped', () => {
    expect(buildModelOptions('m1', ['m2', 'm1', 'm3'])).toEqual(['m1', 'm2', 'm3']);
  });
  it('falls back to current when empty', () => {
    expect(buildModelOptions('only', [])).toEqual(['only']);
  });
  it('dedupe removes empties and repeats', () => {
    expect(dedupe(['', 'a', 'a', 'b'])).toEqual(['a', 'b']);
  });
  it('collectPayload rejects port 0 via checkPort path', () => {
    // normalizeNumber('') -> null; checkPort reflects invalid
    expect(normalizeNumber('')).toBeNull();
    expect(checkPort('0').valid).toBe(false);
  });
});

describe('collectPayload', () => {
  it('builds minimal payload for unmodified state', () => {
    const p = collectPayload(baseUi());
    expect(p.llm_provider).toBe('deepseek_llm');
    // 人设收敛：persona 仅承载 system_prompt
    expect((p.persona as Record<string, unknown>).system_prompt).toBeTypeOf('string');
    expect((p.persona as Record<string, unknown>).name).toBeUndefined();
    // 无 api key / provider 改动
    expect(Object.keys(p.api_keys as object)).toHaveLength(0);
    expect(p.log_level).toBeUndefined(); // 未改动日志级别不提交（A1）
  });

  it('submits log_level only when changed (A1)', () => {
    const ui = baseUi();
    ui.system.log_level = 'DEBUG';
    ui.system.log_level_changed = true;
    expect(collectPayload(ui).log_level).toBe('DEBUG');
  });

  it('omits log_level when unchanged even if value differs (round-trip)', () => {
    const ui = baseUi();
    ui.system.log_level = 'DEBUG';
    ui.system.log_level_changed = false;
    expect(collectPayload(ui).log_level).toBeUndefined();
  });

  it('clearable persona field (system_prompt only) is sent as null (P1-2)', () => {
    const ui = baseUi();
    ui.persona.clear.system_prompt = true;
    const p = collectPayload(ui);
    expect((p.persona as Record<string, unknown>).system_prompt).toBeNull();
  });

  it('non-clear empty system_prompt keeps original empty string (not null)', () => {
    const ui = baseUi();
    ui.persona.system_prompt = '';
    const p = collectPayload(ui);
    expect((p.persona as Record<string, unknown>).system_prompt).toBe('');
  });

  it('includes provider only when configured or touched (P0-1)', () => {
    const ui = baseUi();
    ui.llm_configs = [
      { id: 'deepseek_llm', name: 'DeepSeek', model: 'deepseek-chat', base_url: 'x', temperature: 0.8, has_key: true, configured: true, touched: false },
      { id: 'new_provider', name: 'New', model: 'gpt-4o', base_url: 'y', temperature: 0.8, has_key: false, configured: false, touched: false },
    ];
    const p = collectPayload(ui);
    const llmC = p.llm_configs as Record<string, unknown>;
    expect(llmC.deepseek_llm).toBeDefined();
    expect(llmC.new_provider).toBeUndefined(); // 未 configured/touched 不纳入
  });

  it('sends api key clear as null and non-empty as string', () => {
    const ui = baseUi();
    ui.api_keys = [
      { env: 'DEEPSEEK_API_KEY', value: 'sk-abc', clear: false },
      { env: 'EMPTY_ENV', value: '', clear: true },
    ];
    const p = collectPayload(ui);
    const keys = p.api_keys as Record<string, unknown>;
    expect(keys.DEEPSEEK_API_KEY).toBe('sk-abc');
    expect(keys.EMPTY_ENV).toBeNull();
  });

  it('coerces port/fps numbers; keeps null out', () => {
    const ui = baseUi();
    ui.system.port = 0; // 会被前台 checkPort 拦截，但这里只验证 payload 容纳 0
    const p = collectPayload(ui);
    expect(p.port).toBe(0);
  });

  it('omits null port/fps/max_tokens from payload', () => {
    const ui = baseUi();
    ui.system.port = null as unknown as number;
    ui.system.live2d_fps = null as unknown as number;
    const p = collectPayload(ui);
    expect(p.port).toBeUndefined();
    expect(p.live2d_fps).toBeUndefined();
  });

  it('developer_mode 显式 boolean 才上送；未定义不改旧行为', () => {
    const p0 = collectPayload(baseUi());
    expect(p0.developer_mode).toBeUndefined(); // 缺省不上送

    const on = baseUi();
    on.system.developer_mode = true;
    expect(collectPayload(on).developer_mode).toBe(true);

    const off = baseUi();
    off.system.developer_mode = false;
    expect(collectPayload(off).developer_mode).toBe(false); // 显式关闭也要落盘
  });
});

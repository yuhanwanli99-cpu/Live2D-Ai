/**
 * PLAN-V3 P0 Spike —— soullink-emotion-sdk 无头冒烟
 * 用法: node tools/soullink-p0-smoke.mjs  （cwd = renderer/）
 */
import { readFileSync, writeFileSync, mkdirSync, copyFileSync, readdirSync, statSync } from 'node:fs';
import { dirname } from 'node:path';
import { join, resolve } from 'node:path';
import { Live2DProfileAutoGenerator } from '@soullink-emotion/profile-generator';
import { SoullinkRuntime } from '@soullink-emotion/engine';

const REPO = resolve(process.cwd(), '../../..');
const MODELS_ROOT = join(REPO, 'Live2D-Ai-pc/open-llm-vtuber/live2d-models');
const MODEL_DIR = 'bai/runtime'; // bai 的 model3/cdi3 在 runtime 子层
const TICKS = 600;
const DT = 1 / 30;
const fail = (m) => { console.error('FAIL:', m); process.exit(1); };

const cdi = JSON.parse(readFileSync(join(MODELS_ROOT, MODEL_DIR, 'bai.cdi3.json'), 'utf8'));
const baiIds = new Set(cdi.Parameters.map((p) => p.Id));
console.log('[bai] cdi params:', baiIds.size);

// 生成器要求 modelDir 平铺：把 bai/runtime/* 拷到临时 staging（不改动仓库布局）
const SRC_DIR = join(MODELS_ROOT, MODEL_DIR); // MODEL_DIR 已含 runtime 子层
const STAGE_ROOT = '/tmp/soullink-p0-models';
const STAGE_DIR = join(STAGE_ROOT, 'bai');
mkdirSync(STAGE_DIR, { recursive: true });
for (const f of readdirSync(SRC_DIR)) {
  const s = join(SRC_DIR, f);
  if (statSync(s).isFile()) copyFileSync(s, join(STAGE_DIR, f));
}

const generator = new Live2DProfileAutoGenerator({ modelsRoot: STAGE_ROOT });
const result = await generator.ensure({ modelDir: "bai", force: true }); // staging 平铺名
console.log('[generator] keys:', Object.keys(result ?? {}).join(','));

const outPath = join(REPO, 'shared/model-profiles/bai.profile.json');
mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, JSON.stringify(result.profile, null, 2), 'utf8');
console.log('[profile] written:', outPath);
const cov = result.coverage ?? {};
console.log('[coverage] facs:', cov.facsKeyCount, 'mapped:', cov.mappedKeyCount,
  'capabilities:', JSON.stringify(cov.capabilities ?? {}));
console.log('[coverage] usedCdi:', cov.usedCdiParameterCount, '/', cov.cdiParameterCount,
  '| unmappedCdi:', (cov.unmappedCdiParameters || []).length);

const profile = JSON.parse(readFileSync(outPath, 'utf8'));
console.log('[profile] top keys:', Object.keys(profile).join(','));

const runtime = new SoullinkRuntime({ profile });
let t = 0;
const touched = new Set();
const eyeSeries = [];
const breathSeries = [];
for (let i = 0; i < TICKS; i++) {
  const snap = runtime.update(t, DT);
  const params = snap?.live2dParams ?? {};
  for (const k of Object.keys(params)) touched.add(k);
  if (params.ParamEyeLOpen !== undefined) eyeSeries.push(params.ParamEyeLOpen);
  if (params.ParamBreath !== undefined) breathSeries.push(params.ParamBreath);
  t += DT;
}

const unknown = [...touched].filter((k) => !baiIds.has(k));
console.log('touched:', touched.size, '| unknown:', unknown.length);
if (unknown.length) fail('输出包含 bai 不存在的参数: ' + unknown.join(','));

const variance = (arr) => Math.max(...arr) - Math.min(...arr);
if (eyeSeries.length && variance(eyeSeries) > 0.01) console.log('眨眼变化幅度 OK:', variance(eyeSeries).toFixed(3));
if (breathSeries.length && variance(breathSeries) > 0.01) console.log('呼吸波动 OK:', variance(breathSeries).toFixed(3));

const nonStandard = ['ParamBodyAngleX3','ParamBodyAngleX6','ParamBodyAngleX4','ParamBodyAngleX5','Param27','Param100','Param90'];
const inProfile = JSON.stringify(profile).includes;
const raw = JSON.stringify(profile);
console.log('非标参数入档检查:', nonStandard.map((p) => p + '=' + raw.includes(p)).join(' '));

console.log('P0 SMOKE PASS');
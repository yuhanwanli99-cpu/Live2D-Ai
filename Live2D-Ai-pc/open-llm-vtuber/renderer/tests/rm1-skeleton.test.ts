/**
 * RM1 · 工程骨架 — RED 测试
 *
 * 对齐契约 contract-pc-render.md §RM1 验收断言（RM1-01 ~ RM1-05）
 * 与架构 architecture-pc-render.md §3.1 RM1（工程骨架设计）。
 *
 * 性质：工程配置 / 文件存在性测试（静态断言）。
 * 方式：纯 node:fs + path 读文件断言，不 import 实现/配置文件本体，
 *      保证机械可执行且不依赖 renderer 工程当前的构建链路。
 *
 * ⚠️ 工程初始化状态提示：
 *   当前 `renderer/` 仅含 `public/libs/`（cubismcore 已就位），
 *   package.json / tsconfig.json / vite.config.ts / index.html / src/ 尚未创建。
 *   本测试在工程未初始化时（package.json 缺失 → RM1-01 断言失败 → 连带配置断言失败）
 *   天然为 **RED**，属预期；待 coder 完成工程骨架后逐步转 GREEN。
 */
import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolve, join, basename } from 'node:path';

/* ------------------------------------------------------------------ *
 * 工程根路径（以当前测试文件所在目录向上定位到 renderer/）
 * ------------------------------------------------------------------ */
const RENDERER_ROOT = resolve(__dirname, '..');
const PKG_PATH = join(RENDERER_ROOT, 'package.json');
const TSCONFIG_PATH = join(RENDERER_ROOT, 'tsconfig.json');
const VITE_CONFIG_PATH = join(RENDERER_ROOT, 'vite.config.ts');
const INDEX_HTML_PATH = join(RENDERER_ROOT, 'index.html');
const TYPES_TS_PATH = join(RENDERER_ROOT, 'src', 'types.ts');

/** 读取 UTF-8 文本；文件不存在时抛错（令该断言/该组 RED）。 */
function readText(relPath: string, absPath: string): string {
  if (!existsSync(absPath)) {
    throw new Error(`[RM1] 工程文件缺失（需先完成工程初始化）：${relPath}`);
  }
  return readFileSync(absPath, 'utf-8');
}

/* 契约命名的必要最小路径（RM1 骨架应存在的文件） */
const REQUIRED_FILES: Array<[string, string]> = [
  ['package.json', PKG_PATH],
  ['tsconfig.json', TSCONFIG_PATH],
  ['vite.config.ts', VITE_CONFIG_PATH],
  ['index.html', INDEX_HTML_PATH],
  ['src/types.ts', TYPES_TS_PATH],
];

/* 单个依赖版本比较：断言版本号满足给定的 major/minor（^x.y 或 x.y 前缀）。 */
function versionAtLeast(declared: string, expected: string): boolean {
  const v = declared.replace(/^[\^~>=<]/, '').split('.');
  const e = expected.split('.').map(Number);
  return v.length >= e.length && v.slice(0, e.length).every((n, i) => Number(n) >= e[i]);
}

describe('RM1 工程骨架', () => {
  /* ------------------------------------------------------------------ *
   * 前置：骨架文件存在性（任一缺失 → 各组断言均失败，保持 RED）
   * ------------------------------------------------------------------ */
  describe('RM1 骨架文件存在（工程已初始化）', () => {
    it.each(REQUIRED_FILES)('%s 存在', (_rel, abs) => {
      expect(existsSync(abs), `应为已初始化工程存在的文件：${_rel}`).toBe(true);
    });
  });

  /* ------------------------------------------------------------------ *
   * RM1-01 · 工程配置存在性与依赖
   *  ─ package.json 含依赖 pixi.js / pixi-live2d-display-lipsyncpatch
   *  ─ scripts: dev / build / preview
   * ------------------------------------------------------------------ */
  describe('RM1-01 package.json', () => {
    let pkg: any;
    beforeAll(() => {
      const raw = readText('package.json', PKG_PATH);
      pkg = JSON.parse(raw);
    });

    it('package.json 是合法 JSON 且定义 name/version/type=module', () => {
      expect(typeof pkg).toBe('object');
      expect(pkg.name).toBeTruthy();
      expect(pkg.type).toBe('module');
    });

    it('dependencies 含 pixi.js（^7.3.0 同级）', () => {
      const dep: string | undefined = pkg?.dependencies?.['pixi.js'];
      expect(dep, 'pixi.js 依赖缺失').toBeTruthy();
      expect(versionAtLeast(dep, '7.3.0'), `pixi.js 版本需 >= 7.3.0，实际 ${dep}`).toBe(true);
    });

    it('dependencies 含 pixi-live2d-display-lipsyncpatch', () => {
      const dep: string | undefined = pkg?.dependencies?.['pixi-live2d-display-lipsyncpatch'];
      expect(dep, 'pixi-live2d-display-lipsyncpatch 依赖缺失').toBeTruthy();
    });

    it('scripts 含 dev', () => {
      expect(pkg?.scripts?.dev, '缺少 scripts.dev').toBeTruthy();
    });
    it('scripts 含 build', () => {
      expect(pkg?.scripts?.build, '缺少 scripts.build').toBeTruthy();
    });
    it('scripts 含 preview', () => {
      expect(pkg?.scripts?.preview, '缺少 scripts.preview').toBeTruthy();
    });

    it('devDependencies 含 vite / typescript / vitest', () => {
      expect(pkg?.devDependencies?.['vite'], '缺少 vite').toBeTruthy();
      expect(pkg?.devDependencies?.['typescript'], '缺少 typescript').toBeTruthy();
      expect(pkg?.devDependencies?.['vitest'], '缺少 vitest').toBeTruthy();
    });
  });

  /* ------------------------------------------------------------------ *
   * RM1-02/03 · vite.config.ts 关键配置
   *  ─ base='./'
   *  ─ build.outDir 指向 ../frontend（相对 renderer/ 解析后 == <repo>/frontend）
   *  ─ build.emptyOutDir=true
   * ------------------------------------------------------------------ */
  describe('RM1-02/03 vite.config.ts', () => {
    let cfg: string;
    beforeAll(() => {
      cfg = readText('vite.config.ts', VITE_CONFIG_PATH);
    });

    it('配置为 vite 的 defineConfig 模块（import { defineConfig } from "vite"）', () => {
      expect(cfg).toMatch(/defineConfig/);
      expect(cfg).toMatch(/from\s+['"]vite['"]/);
    });

    it('base 设为相对路径 "./"', () => {
      expect(cfg).toMatch(/base\s*:\s*['"]\.\/['"]/);
    });

    it('build.outDir 指向 ../frontend', () => {
      expect(cfg).toMatch(/outDir\s*:\s*['"]\.\.\/frontend['"]/);
    });

    it('build.outDir 相对 renderer/ 解析后等于 <repo>/frontend', () => {
      // 解析 ../frontend → 定位到 open-llm-vtuber/frontend（后端静态托管目录）
      const outDirAbs = resolve(RENDERER_ROOT, '../frontend');
      expect(basename(outDirAbs)).toBe('frontend');
    });

    it('build.emptyOutDir=true', () => {
      expect(cfg).toMatch(/emptyOutDir\s*:\s*true/);
    });

    it('server.devPort=5173（吻合 RM1-05 的 fetching host）', () => {
      expect(cfg).toMatch(/port\s*:\s*5173/);
    });
  });

  /* ------------------------------------------------------------------ *
   * tsconfig.json · 编译配置合理性（TS5 工程必备）
   * ------------------------------------------------------------------ */
  describe('RM1 tsconfig.json', () => {
    let tsRaw: string;
    let ts: any;
    beforeAll(() => {
      tsRaw = readText('tsconfig.json', TSCONFIG_PATH);
      ts = JSON.parse(tsRaw);
    });

    it('是合法 JSON', () => {
      expect(typeof ts).toBe('object');
    });

    it('module 为 ESNext/module（配合 type=module 的产物）', () => {
      const m = ts?.compilerOptions?.module;
      expect(m === 'ESNext' || m === 'ES2020' || m === 'ES2022' || m === 'NodeNext', `module=${m}`).toBe(true);
    });

    it('target 为较新标准（ES2020+）', () => {
      const t = ts?.compilerOptions?.target;
      expect(/^ES(2020|2021|2022|2023|Next)$/.test(String(t)), `target=${t}`).toBe(true);
    });

    it('strict=true', () => {
      expect(ts?.compilerOptions?.strict, 'strict 应为 true').toBe(true);
    });
  });

  /* ------------------------------------------------------------------ *
   * RM1-04 · index.html
   *  ─ <script src="./libs/live2dcubismcore.min.js"> 在 main.ts 之前
   *  ─ 含 <div id="canvas"> 容器
   * ------------------------------------------------------------------ */
  describe('RM1-04 index.html', () => {
    let html: string;
    beforeAll(() => {
      html = readText('index.html', INDEX_HTML_PATH);
    });

    it('引用 ./libs/live2dcubismcore.min.js', () => {
      expect(html).toMatch(/<script[^>]*src\s*=\s*['"]\.\/libs\/live2dcubismcore\.min\.js['"]/);
    });

    it('含 <div id="canvas"> 容器', () => {
      expect(html).toMatch(/<div[^>]*id\s*=\s*['"]canvas['"]/);
    });

    it('cubismcore 的 script 标签出现在 main.ts 脚本之前（加载顺序正确）', () => {
      const coreIdx = html.indexOf('live2dcubismcore.min.js');
      const mainIdx = html.indexOf('src/main.ts');
      expect(coreIdx).toBeGreaterThanOrEqual(0);
      expect(mainIdx).toBeGreaterThanOrEqual(0);
      expect(coreIdx, 'cubismcore 脚本必须先于 main.ts 加载').toBeLessThan(mainIdx);
    });
  });

  /* ------------------------------------------------------------------ *
   * types.ts · 共享类型最小集（WS 消息相关，contract §RM1/RM7 定义）
   *  ─ 导出 ModelInfo（url/kScale/initialYshift/kXOffset）
   *  ─ 导出 WS 消息 type 的判别（type: 'set-model-and-conf' | 'audio' | 'control'）
   *  ─ 导出 parameterOverrides 承载类型（Record<string, number>）
   * ------------------------------------------------------------------ */
  describe('RM1 types.ts 共享类型', () => {
    let tsSrc: string;
    beforeAll(() => {
      tsSrc = readText('src/types.ts', TYPES_TS_PATH);
    });

    it('导出 ModelInfo 接口（含 url）', () => {
      expect(tsSrc).toMatch(/export\s+interface\s+ModelInfo/);
      expect(tsSrc).toMatch(/ModelInfo[\s\S]{0,120}url\s*:/);
    });

    it('导出 WsMessageType 判别（set-model-and-conf / audio）', () => {
      expect(tsSrc).toMatch(/set-model-and-conf/);
      expect(tsSrc).toMatch(/['"]audio['"]/);
    });

    it('导出 ParameterOverrides 承载类型（Record<string, number>）', () => {
      expect(tsSrc).toMatch(/ParameterOverrides/);
      expect(tsSrc).toMatch(/Record\s*<\s*string\s*,\s*number\s*>/);
    });
  });
});


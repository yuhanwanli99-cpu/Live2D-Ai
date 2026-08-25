// Live2D-Ai renderer 版本常量（单一事实来源，构建时通过 Vite define 注入）。
// product_version 与后端 src/open_llm_vtuber/versions.py 的 PRODUCT_VERSION 保持一致。
// frontend_version 对应 renderer/package.json version。
//
// Vite `define` 会在构建时把 __PRODUCT_VERSION__ / __FRONTEND_VERSION__ 替换为字面量；
// dev / vitest / 纯 JS 环境下该全局缺失时，用 typeof 守卫回退到这里的默认值，保证稳定。

const __PROD__ = typeof __PRODUCT_VERSION__ === 'string' ? __PRODUCT_VERSION__ : 'v0.2.0-preview.1';
const __FRONT__ =
  typeof __FRONTEND_VERSION__ === 'string' ? __FRONTEND_VERSION__ : '1.0.0';

export const PRODUCT_VERSION = __PROD__;
export const FRONTEND_VERSION = __FRONT__;

export const RENDERER_PACKAGE_VERSION = '1.0.0';

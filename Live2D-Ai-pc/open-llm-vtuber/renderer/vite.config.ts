import { defineConfig } from 'vite';

// Live2D-Ai PC 渲染层 Vite 配置（对齐 architecture-pc-render.md §3.1 RM1）
export default defineConfig({
  // 相对路径 base：兼容后端静态托管（后端把 frontend/ 挂为静态目录，零后端改动）
  base: './',
  build: {
    // 直接输出到 ../frontend/，替换旧 React SPA 产物
    outDir: '../frontend',
    emptyOutDir: true,
    // 不内联资源（live2d 模型资产走 /live2d-models/，不进 bundle）
    assetsInlineLimit: 0,
  },
  server: {
    port: 5173,
    proxy: {
      // 开发时反向代理模型文件（后端 :12393 提供）
      '/live2d-models': 'http://localhost:12393',
      // 开发时 WebSocket 代理（后端 WS 端点）
      '/ws': { target: 'ws://localhost:12393', ws: true },
    },
  },
});

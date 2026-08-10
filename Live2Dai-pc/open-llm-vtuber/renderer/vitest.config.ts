import { defineConfig } from 'vitest/config';

// vitest 独立配置（vite.config.ts 保持纯 Vite 配置，对齐架构 RM1）
export default defineConfig({
  test: {
    environment: 'node',
  },
});

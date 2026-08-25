// 构建时由 Vite `define` 注入的全局常量（见 vite.config.ts）。
// 运行时被替换为字面量；缺失（dev/未注入）时在 version-info.ts 回退默认值。
declare const __PRODUCT_VERSION__: string;
declare const __FRONTEND_VERSION__: string;

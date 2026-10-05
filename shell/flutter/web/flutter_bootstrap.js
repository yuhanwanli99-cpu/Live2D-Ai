{{flutter_js}}
{{flutter_build_config}}

// ── 字体回落结构性离线化（R4-T4）──────────────────────────────────────────
// 引擎在「已打包字体没有这个字形」时会去下载 Noto 回落字体。默认 base URL 是
//   https://fonts.gstatic.com/s/    （引擎 configuration.dart: fontFallbackBaseUrl 默认值）
// 本项目是本地优先的桌宠，不允许运行期依赖 Google CDN（断网即豆腐块、且出网本身
// 就是隐私/可用性缺口）。这里把它改成**同源相对路径** font-fallback/：
//
//  * 引擎用 Uri.parse(baseUrl).resolve(font.url)（font_fallback_service.dart），
//    所以 baseUrl **必须以 / 结尾**；
//  * 相对路径相对**文档 base**（web/index.html 的 <base href="/app/">）解析 ⇒
//    实际命中 /app/font-fallback/**，换 base-href（如 /）时这里无需改动；
//    写成绝对 "/app/font-fallback/" 反而会在别的 base-href 下失效；
//  * 镜像内容 = 5 个高价值整族（21 文件 / 约 2.69 MiB），由
//    scripts/font_fallback_mirror.sh 生成、MANIFEST.txt + --check 审计；
//    未镜像的字族同源 404 ⇒ 该字形豆腐块，这是**明确接受的取舍**
//    （见 docs/architecture/font-fallback-offline.md）。
//  * 为什么要自带 flutter_bootstrap.js：flutter_tools 支持用户自带模板
//    （build_system/targets/web.dart 读 web/flutter_bootstrap.js），而
//    本文件顶部那两行 flutter_js / flutter_build_config 占位符 **必须**原样保留，
//    否则构建/加载会失败（**注释里也不许再写出占位符字面量**：flutter_tools 是
//    全文 replaceAll，写在注释里会被注入一整份 flutter.js，把注释行切断）。
//    现代 API = load({config}) → initializeEngine(config)；
//    已废弃的 window.flutterConfiguration 写法**不要用**。
const flutterConfig = {
  fontFallbackBaseUrl: 'font-fallback/',
};

_flutter.loader.load({
  config: flutterConfig,
  serviceWorkerSettings: {
    serviceWorkerVersion: {{flutter_service_worker_version}}
  }
});

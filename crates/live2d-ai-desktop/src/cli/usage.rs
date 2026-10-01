//! CLI 用法文本（`--help` 与错误提示共用）。
//!
//! 拆分承载：原 `cli/mod.rs` 把 USAGE 内嵌 ~63 行，加 `--web/--http-port` 解析
//! 后逼近 500 行上限。USAGE 是纯字面量，独立成文件最自然。
//!
//! 2026-10-01（W2-B / D1 第二段）：原生壳（窗口/模型冒烟、桌宠模式、终端 chat、
//! benchmark）移出构建，相应用法行随之删除；恢复条件见
//! `docs/architecture/ARCHIVED-native-shell.md`。

/// 用法文本（`--help` 与错误提示共用）。
pub const USAGE: &str = "\
live2d-ai-desktop — Live2D-Ai PC 桌面应用（Web 主路径 + 声卡冒烟）

用法:
  live2d-ai-desktop                     打印架构/会话线索/能力后退出（不访问声卡）
  live2d-ai-desktop --audio-smoke       音频冒烟：0.8s、440Hz 低音量经声卡播放
  live2d-ai-desktop --audio-smoke-secs S [--audio-smoke-silence]
                                        自定义时长（S <= 30）；--audio-smoke-silence 改播静音
  live2d-ai-desktop --web [--http-port P] [--dev-mode]
                                        Web API 后台模式：监听 127.0.0.1:P（缺省 18080），
                                        暴露 GET /api/v1/app/{capabilities,status} 、
                                        GET/PATCH /api/v1/settings 、
                                        POST /api/v1/settings/test/{llm,tts} 、
                                        GET /api/v1/mods 、WS 事件流，并同源托管
                                        Flutter Web /app/；**不**需要 LLM/TTS 配置
                                        即可启动（无配置时端点返回 has_api_key=false、
                                        configured=false）

选项:
  -h, --help                 打印本帮助
  --audio-smoke              音频输出冒烟（访问默认声卡；无设备退出码 3）
  --audio-smoke-secs <S>     音频冒烟时长秒数（0 < S <= 30，可小数；隐含 --audio-smoke）
  --audio-smoke-silence      音频冒烟改播数字静音（链路照常，无声可听；隐含 --audio-smoke）
  --web                      启动 Web API 后台模式（产品主路径；与音频冒烟互斥）
  --http-port <P>            监听端口（>= 1024；隐含 --web；缺省 18080）
  --dev-mode                 本次启动覆盖 dev_mode（优先级最高；开放 /api/v1/logs* 端点）

说明:
  - 原生窗口壳 / 托盘 / 终端 chat / benchmark 已于 2026-10-01（W2-B / D1 第二段）
    随「原生壳岛」移出构建：`--window-smoke` / `--model-smoke` / `--smoke-frames` /
    `--smoke-timeout-secs` / `--pet-mode` / `--chat` / `--config` / `--benchmark*`
    一律**不再支持**（恢复条件见 docs/architecture/ARCHIVED-native-shell.md）；
    交互式桌面体验一律走 `--web` + Flutter `/app/`（./scripts/ignite.sh）。
";

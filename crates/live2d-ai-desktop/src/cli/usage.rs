//! CLI 用法文本（`--help` 与错误提示共用）。
//!
//! 拆分承载：原 `cli/mod.rs` 把 USAGE 内嵌 ~63 行，加 `--web/--http-port` 解析
//! 后逼近 500 行上限。USAGE 是纯字面量，独立成文件最自然。

/// 用法文本（`--help` 与错误提示共用）。
pub const USAGE: &str = "\
live2d-ai-desktop — Live2D-Ai PC 桌面应用（Linux 原生窗口 + Bai 实时渲染）

用法:
  live2d-ai-desktop                     打印架构/会话线索/能力后退出（不访问声卡）
  live2d-ai-desktop --window-smoke      纯透明无边框窗口壳，持续重绘直到关闭
  live2d-ai-desktop --model-smoke [P]   Bai 实时渲染冒烟：加载皮套上屏 + 六动作 +
                                        合成口型（不访问 LLM/TTS/声卡；P 缺省为仓库内 Bai；
                                        默认交互可关闭，不置顶不穿透）
  live2d-ai-desktop --pet-mode          桌宠模式（常驻）：默认置顶 + 点击穿透 + 托盘菜单
                                        （穿透/置顶/显隐/退出；托盘未就绪时禁自动穿透）
  live2d-ai-desktop --pet-mode --model-smoke [P]
                                        桌宠模式 + 模型实时渲染
  live2d-ai-desktop --smoke-frames N    渲染 N 帧后自动退出（CI 冒烟，60s 超时兜底；
                                        搭配 --model-smoke 即模型冒烟帧数）
  live2d-ai-desktop --window-smoke --smoke-timeout-secs S
                                        显式超时兜底秒数
  live2d-ai-desktop --audio-smoke       音频冒烟：0.8s、440Hz 低音量经声卡播放
  live2d-ai-desktop --audio-smoke-secs S [--audio-smoke-silence]
                                        自定义时长（S <= 30）；--audio-smoke-silence 改播静音
  live2d-ai-desktop --chat [--config <P>] [--pet-mode] [--smoke-timeout-secs S]
                                        终端 chat：stdin 输入 → LLM → TTS → 声卡 →
                                        动作/口型（/stop /release /quit；EOF 视同退出；
                                        P 缺省为 cwd 的 live2d-ai.toml，找不到提示复制模板）
  live2d-ai-desktop --benchmark [P] [--benchmark-mode blocking|submit]
                            [--benchmark-warmup N] [--benchmark-frames N]
                            [--benchmark-headless] [--benchmark-output <P>]
                                        benchmark-only A/B（C7/C8）：同一 binary 双模式
                                        渲染 600+60 帧；缺省 mode=submit。**不**成为生产
                                        默认；与三种冒烟/chat 互斥。--benchmark-headless
                                        跳过 winit（沙箱无 X server 时的回退）。
  live2d-ai-desktop --web [--http-port P]
                                        Web API 后台模式：监听 127.0.0.1:P（缺省 18080），
                                        暴露 GET /api/v1/app/{capabilities,status} 、
                                        GET/PATCH /api/v1/settings 、
                                        POST /api/v1/settings/test/{llm,tts}；**不**需要
                                        LLM/TTS 配置即可启动（无配置时端点返回 has_api_key=false、
                                        configured=false；与三种冒烟/chat/benchmark 互斥）

选项:
  -h, --help                 打印本帮助
  --pet-mode                 桌宠模式：默认置顶 + 点击穿透 + 托盘恢复入口
                             （托盘初始化失败/超时 → 禁自动穿透，窗口保持交互；
                             可单独使用或搭配窗口/模型冒烟；与音频冒烟/benchmark 互斥）
  --window-smoke             纯透明窗口壳（不加载模型；与模型/音频冒烟/benchmark 互斥）
  --model-smoke [<model3>]   模型实时渲染冒烟（可选 model3.json 路径，不以 - 开头；
                             与窗口/音频冒烟/benchmark 互斥；缺参走 Bai 相对路径默认值）
  --smoke-frames <N>         冒烟帧数目标（N >= 1；孤立出现时隐含纯窗口壳冒烟）
  --smoke-timeout-secs <S>   窗口/模型冒烟/chat 超时兜底（S >= 1 秒）
  --audio-smoke              音频输出冒烟（访问默认声卡；无设备退出码 3）
  --audio-smoke-secs <S>     音频冒烟时长秒数（0 < S <= 30，可小数；隐含 --audio-smoke）
  --audio-smoke-silence      音频冒烟改播数字静音（链路照常，无声可听；隐含 --audio-smoke）
  --chat                     终端 chat 模式（stdin 逐行输入；Say 待播容量 1，满提示忙碌；
                             与窗口/模型/音频冒烟/benchmark 互斥；/quit 或 EOF 退出）
  --config <path>            指定 chat 配置文件 live2d-ai.toml（隐含 --chat；缺省当前目录
                             查找，再找不到提示复制 live2d-ai.toml.example）
  --benchmark [<model3>]     benchmark-only A/B（C7/C8）；P 缺省为仓库内 Bai
                             （隐含启动 winit+wgpu+加载模型+双模式渲染；**不**为生产默认；
                             与三种冒烟/chat 互斥）
  --benchmark-mode <M>       benchmark 模式（blocking | submit；缺省 submit；
                             submit 调 render_to_view_submit 非阻塞、blocking 调
                             render_to_view 阻塞 wrapper）
  --benchmark-warmup <N>     warm-up 帧数（N >= 1；缺省 60；不计入正式统计）
  --benchmark-frames <N>     正式帧数（N >= 1；缺省 600）
  --benchmark-headless       跳过 winit 事件循环，经 GpuContext::headless() +
                             离屏纹理（沙箱无 X server 时的回退；缺 present_interval）
  --benchmark-output <P>     JSON 报告输出路径（缺省仅 stdout）
  --web                      启动 HTTP/WS 控制平面后台（127.0.0.1:<port>）；
                             **不**校验 LLM/TTS 配置（无配置时端点仍可正常响应，
                             标记 has_api_key=false、configured=false）；与
                             三种冒烟/chat/benchmark 互斥（D2 接线；
                             见 docs/plans/node-d-api-contract-2026-08-28.md §1）
  --http-port <P>            Web API 监听端口（1024..=65535；缺省 18080；
                             隐含 --web；不绑 0.0.0.0/::，仅 loopback；P0-3 安全红线）

退出码: 0 成功；1 代码/资产/模型错误；2 CLI 用法错误；3 运行环境不满足（GPU/显示/声卡）。
日志: RUST_LOG=info|debug|trace|off（默认 info）";

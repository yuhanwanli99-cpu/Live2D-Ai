# v0.3.0（稳定版，Rust 主线）— 通用人形皮套 AI 接入一体化平台（2026-09）

- **平台定位（文档治理）**：重构 README / AGENTS / AGENT 为「通用人形皮套 AI 接入一体化平台
  （非绑死单模型 / 非复杂上层）」；旧 Python/Android 文档加「已归档」banner（PLAN / PROGRESS）。
- **渲染纹理 / 离屏靶标档位 ADR 冻结**：`docs/architecture/renderer-texture-tier-adr.md`（4096/8192/16384），
  接档实现与真机验证按 ADR 排期。
- **未来路线图**：`docs/plans/future-roadmap-2026-09.md`（P0 滚项 / 外部验收 / 后续三档）。
- **迭代引用**：本版收编 dev/integrity 主线（S1/S2 事件桥与 director whitelist 修复、
  P3/P3.1 真实性收口、P1 5-Mod 开箱即用）映射为稳定版 v0.3.0；workspace 由 0.1.0 → 0.3.0。
- **验收门禁**：workspace 763 tests 通过；fmt / clippy `-D warnings` / rust-ratio（95.28%）全绿；
  详见 `docs/releases/v0.3.0.md`。外部验收项（S3 真机 16384、S4 档位视觉、F6 出声端到端）留待真机。

---

# Rust 重建：发布前测试增强 + 节点 C 前期准备（2026-08-27 第六批）

- **发布前测试增强**（节点 B 复审遗留三条，全部补齐）：新增 `supervisor/tests_stall.rs`——`stop_during_tts_in_flight_yields_no_completion_and_reopens`（TTS 在飞 stop ⇒ 零 TurnCompleted/Drained + 第二轮正常收口且请求体无被停轮历史）、`stop_during_pending_pcm_pump_advances_epoch_without_completion`（Stage B 泵驻留窗口 stop ⇒ 零收口事实 + 根推进 epoch=1）、`stall_timeout_fatal_terminates_after_5s_zero_progress`（StallProducer 永远 WouldBlock{0}+healthy ⇒ 5s 后 GenerationFinished{completed:false} + TurnCompleted{outcome_completed:false}）。support.rs 新增 `spawn_tts_mock_slow` / `StallProducer` 两个 helper。
- **行数合规**：tests_fault.rs 524→338（迁出 stop_during_drain 至 tests_stall.rs）；turn.rs 547 / tests_stall.rs 530 在 ≤1000 豁免区间并头注技术理由（三宏与 run_one_turn 局部状态强绑定，抽函数必撞 biased select 借用冲突）。
- **节点 C 前期准备**：`docs/plans/node-c-render-platform-audit-brief.md` 69→93 行——6 个定位点全部更新到拆分后位置（app.rs→app/ 目录：frame.rs:55-61 非阻塞 Poll / bootstrap.rs:66-71 request_adapter / surface.rs:5-8 契约注释等），新增「拆分后位置更新」对照表 + 「待裁决复核」标注；C1–C11 措辞未动。交叉验证证据落盘 `docs/verification/node-c-location-recheck-2026-08-27.md`（207 行，grep 证据+上下文+结论）。
- **门禁**：workspace **318 passed / 0 failed**（×2 稳定，desktop 101）；fmt --all 干净；clippy 仅 PetUserEvent Tray 前缀豁免；无 >1000 行源文件。节点 C 的 C1–C11 裁决交由高级 AI 执行（另调）。

---

# Rust 重建：节点 B 第四/五轮收口——关闭通道忙循环修复，节点 B 正式通过（2026-08-27 第五批）

- **B4-P0 关闭通道忙循环**（第五轮复审唯一阻断项）：运行中直接 drop `SupervisorHandle`（不调 quit）⇒ 三个 sender 全销毁 ⇒ `control_rx.recv()` 永久立即返回 `None`，biased select 中 control 臂排第一且永久就绪 ⇒ 每轮必赢 ⇒ `gen_fut` 永无轮询机会 ⇒ supervisor 线程忙循环无法回收。修复：turn.rs 三个阶段统一——通道关闭**按 Quit 处理**（`*quitting = true` + 完整 do_stop! 事务），`control_closed`/`finish_closed` 守卫**摘除永久就绪臂**（阶段 A 写后循环继续为真读；阶段 B/drain 写后即 return 为死赋值，删除并注释），gen_fut 始终 await 到自然返回（D2 契约保持）。
- **新增回归测试** `dropping_handle_during_generation_cancels_and_reaps_supervisor`：slow LLM 在飞 → 直接 drop handle（不 stop/quit）→ 有限时间内观察 ShutdownReady + 无第二个 LLM 请求；测试带超时保护不会自挂。
- **口径修正**：stop-during-drain「未提交历史」改硬证据——stop 后不 quit、发第二轮、断言第二轮请求体仅 system+user（无被停轮 assistant 历史）；tests_pcm 两条如实声明为组件链路测试（BL-2 才是 Stage B 整链路）。
- **节点 B 正式通过**（第五轮复审）：root epoch 单一权威 / stop 先推进 epoch 再取消清环 / 双闩锁顺序 / GenerationFinished 恰一次 / partial-write 即报 Started / 多次 WouldBlock 无损 / fault→Failed / fatal-after-started 必报 Cleared / stop-during-drain 三不 / action 完成保留原始 epoch / pending Say 被 stop 清除 / sender 断开不忙循环 / shutdown 有限时间回收——全部闭合。
- **门禁**：workspace **315 passed / 0 failed**（×2 稳定，desktop 98）；fmt --all 干净；clippy 仅 PetUserEvent Tray 前缀豁免；无 >1000 行源文件。遗留（不阻断）：stop-during-TTS / stop-during-pending-pump / 真实 stall 超时三条测试增强项列入发布前清单。

---

# Rust 重建：节点 B 第三轮 P0 收口 + 大文件拆分（2026-08-26 第四批）

- **B3-P0-1 pending 泵无损**（复审实锤：Stage B 仅重试一次即析构 prep → 尾段静默截断、5s 哨兵不可达）：`WouldBlock{accepted}` 后同一 prepared **放回 `pending_pcm`** 继续泵送直至完全入环 / stop / fault / 连续 5 秒零进展；`accepted > 0` 即重置 `stall_ticks`（计时口径＝连续零进展，非累计泵时）。
- **B3-P0-2 root outcome 归一化**：声卡 fault/stall 属内部致命 ⇒ Stage C 统一 `GenerationOutcome::Failed`——生成期 fault 引擎报 Cancelled 不再冒充用户取消；泵期 stall 不再先报 Completed 再打补丁。仅真实用户取消得 Cancelled，LLM 中途失败沿用引擎 Failed。
- **B3-P0-3 stop-after-drain 竞态切断**：drain-watch 中执行 do_stop! 后**立即 return**，旧 turn 的提交、fallback、一切生成侧副作用被禁止（否则 fallback 会被盖上新 epoch，形成「stop 复活动作」竞态）；提交门禁改用当前 `stopped` 权威值而非 Stage A 快照。
- **测试 truthing**（复审判定「测试声明不成立」的对应补正）：BL-2 增补 root 事实顺序与计数断言（Started<GenFinished<Drained<TurnCompleted(Completed)，GenFinished/Drained/TurnCompleted 各恰 1 次、Dropped==0）；partial-write 测试重写为经真实 `try_enqueue_prepared` 路径；新增 supervisor 全链路测试：多次 WouldBlock 样本无损、内部音频故障最终 TurnCompleted(Failed)、stop-during-drain 三不（不提交/fallback/新动作）、`report_action_finished` 经真实 finish 通道到达 supervisor。
- **大文件拆分**（用户新约束：经手文件 ≤500 行，豁免上限 1000）：supervisor.rs / app.rs / audio.rs / core lib.rs / dialogue.rs / conversation_engine.rs 集成测试全部模块化拆分。

---

# Rust 重建：节点 A 裁决 P0 契约修复 + 最终接线落地，真实对话闭环打通（2026-08-26）

- **步骤 1–7（P0 契约地基）**：core 双闩锁（`GenerationFinished/PlaybackStarted/PlaybackDrained/PlaybackCleared` 四事实 ⇒ `TurnCompleted` 恰一次，per-chunk 音频调度删除）；`PreparedPcm` 全有或重试入环（WouldBlock 零丢失、转换恰一次、整帧对齐；兼容壳保留冒烟）；cpal 错误回调置位 `fault` 健康闩 + `MouthSnapshot{level,audio_epoch,healthy}` 只读句柄（D5）；history 显式提交 `commit_completed_turn`（P0-4，run_turn 不再自行提交）；`ParameterMask` 稀疏所有权（P0-5：只写持有通道、释放即 clear、idle 可接管；诚实声明 v0 无自动眨眼）；`ActionPlaybackFinished{epoch,action}` 身份化自然完成（P0-6）；`EngineEvent::Terminal{status}` 三态统一（D8：LLM 中途失败不清已入队语音、残余不再 TTS；终态 3s 有界投递兜底）；stop_and_clear 的 audio epoch 自增移出重试循环（D7）。
- **步骤 8–11（最终接线，经审批推进）**：`AppEvent{Tray,Render,Conversation,ShutdownReady}` 统一事件入口（D4）；**supervisor**（独立 OS 线程 + current_thread Tokio runtime，独占 root reducer/engine/AudioOutputFacade）：EngineEvent 代次守卫消费、PCM 永不过 AppEvent（supervisor 内 prepare→有界入环，`WouldBlock` 触发 select 守卫停读下一事件 + 周期泵重试）、**四条件 Drained 门控**（生成终态∧无 pending∧确曾起播∧ring 排空健康——生成中途短暂排空绝不算数）、root Effect→RenderCommand 投影、render_epoch 镜像防御闸门（D9）、VoiceStarted/VoiceEnded 口型归零协议（非 Speaking 强制电平 0）、Say 有界容量 1 + Stop/Quit 无界控制通道（D14）、`/stop` 七步事务（D7 顺序）、统一 shutdown handshake（/quit=CloseRequested=TrayExit，500ms watchdog）（D13）、工具强度非法即拒绝不 clamp + 仅根接受（Start/Transition）抑制 fallback + fallback 输入用完整 assistant_text（D11）；REPL stdin 线程 + `--chat [--config] [--pet-mode]` CLI。
- **闭环证据**：新增 `closed_loop_two_turns_commit_history_via_drain_path` 集成测试——两轮真实 mock LLM/TTS 流式交互后，第二轮请求体携带第一轮 user/assistant 历史（历史只能经 supervisor 排空后的显式提交进入），quit 后 ShutdownReady 如期出现。`--chat` 缺配置 → 可见错误 + 模板指引 + 退出码 1（契约实测）。
- **门禁**：workspace `fmt --check` 干净；`clippy --workspace --all-targets` 仅剩既有的 PetUserEvent Tray 前缀警告（枚举更名属外观项，随下次托盘批次处理）；全量测试 **310 passed / 0 failed**（l2d 42+2 · core 33+4 · desktop 93 · runtime 107+12+9+8）。运行方式：`cp live2d-ai.toml.example live2d-ai.toml && cargo run -p live2d-ai-desktop -- --chat`。

---

# Rust 重建：平台能力实测证据回收 + WASM 发布门禁通过（2026-08-26 第二批）

- **WSLg 平台能力实测**（RFC 批次 6 验证回收）：X11 pet-mode 90 帧（置顶生效/穿透 API 生效/透明 available；无 D-Bus ⇒ 托盘降级可见且自动穿透正确禁用）；Wayland 60 帧（置顶如实 unavailable、穿透/透明 available）；全局定位两会话均被合成器拒绝（回读远偏离，如实记 unavailable——WSLg 行为，真实 WM 待节点 C 复测）；模型冒烟 120 帧六动作顺序执行 + 口型通道 writes=120 peak=0.871；音频冒烟无声卡退出码 3（环境分类正确）。证据矩阵落盘 `docs/verification/desktop-platform-smoke-2026-08-26.md`。
- **性能量化基线（节点 C 输入）**：llvmpipe 软渲染 frame_time avg≈116ms / max≈144ms（当前 `render_to_view` 每帧阻塞 wait 的实测代价，非阻塞化改造前后须同法对比）。
- **WASM demo 构建门禁 ✅**：安装 trunk 0.21.14 后 `trunk build --release` 通过，dist 产物验证落盘（index.html + JS glue 111KB + wasm 5.2MB）；源码要件此前已确认（`ModelPackage::from_memory_map` 内存构造、`BROWSER_WEBGPU | GL` 回退）。清单第 4 项收口。
- **desktop crate README 更新**：修复滞后两个批次的能力叙述（「没有任何初始化路径/request_feature 默认拒绝」→「有代码路径 ≠ 运行时生效」口径）、架构清单补全 6 个模块、追加两轮冒烟验证记录。
- **节点 C 审计交接文档前置**：`docs/plans/node-c-render-platform-audit-brief.md`——渲染热路径阻塞点已核实到行号（model_core.rs:176-179 wait_indefinitely；mod.rs:210 离屏合法阻塞；app.rs:942 壳路径已是非阻塞形态），C1–C11 裁决清单（API 拆分形状/异步失效映射/in-flight 上限/resize 时序/WASM poll 差异/benchmark 协议/平台承诺表终稿）。
- 门禁：workspace clippy 复跑仅剩既有 PetUserEvent 前缀警告（该枚举为节点 A D4 重构对象）；release 全 workspace 构建另见下批记录。

---

# Rust 重建：应用配置加载 + 终端 REPL 纯解析 + 接线前审计交接（2026-08-26）

- **应用配置 `live2d-ai.toml`**（`live2d-ai-runtime::settings`，新模块）：`[llm]`/`[tts]`/`[persona]` 三段反序列化 → `resolve()` 产出 `LlmConfig`/`TtsConfig`/`ConversationConfig`；密钥不进文件——文件写环境变量**名**（`api_key_env`），运行时读取后包 `ApiSecret`（未设置/空串=不发 Authorization 头）；未知字段拒绝解析（防拼写错误静默失效）、段整体缺省回退默认、URL 与 env 名在 resolve 阶段按段名报错。新增依赖 `toml = "0.9"`。模板落盘根目录 `live2d-ai.toml.example`（经 `include_str!` 成为 `AppSettings::example_toml()`，模板本身有解析测试锁定）；`.gitignore` 屏蔽本机 `live2d-ai.toml`。
- **终端 REPL 纯解析**（`live2d-ai-desktop/src/repl.rs`，新模块）：`parse_line` 一行文本 → `ReplCommand::{Say,Stop,Release,Quit,Unknown}`；空行/#注释忽略、命令大小写不敏感、`/exit`=`/quit` 别名、未知 `/` 词归 Unknown 由调用方提示；零 IO 零线程，接线留待最终接线批次。7 个表驱动测试全绿。
- **desktop 基线修复**：复跑发现 `backend::tests::description_declares_pet_capabilities_and_runtime_gate` 失败——根因是描述文本漂移（`WINIT_WGPU_NOTES` 缺「静态声明 ≠ 运行时承诺」gate 口径行），测试正确；补第六行 gate 说明 + 契约 doc 注释，测试零改动。desktop 恢复 **80 passed / 0 failed**。
- **高级节点 A 审计交接**：`docs/plans/node-a-wiring-audit-brief.md`——给接线前高级审计的完整交接文档：现状快照、关键契约速查（run_turn/EngineEvent/PlaybackHandle/PerformancePlayer/PetUserEvent 签名与行号）、14 个必须裁决问题（进程结构与所有权 D1-D5 / turn 生命周期与取消 D6-D9 / 动作口型仲裁 D10-D12 / REPL 与退出 D13-D14）、建议接线架构草案、审计输出格式。实现方将以裁决为唯一规则源开始接线。

---

# Rust 重建：Bakeoff 选定 Ayagami 并 pin 引入 crates/l2d（2026-08-26）

- **选型落地**（RFC D4）：依据双 bakeoff 报告（`docs/verification/rust-bakeoff-ayagami.md` / `-mocari.md`）选定 **Ayagami** 为 runtime/render 底座，git 依赖 pin rev `640ae4b10bad8def1adcacdada4f8241b484c169` 引入 `crates/l2d`（未复制源码、未 commit）；Mocari 不引入，保留为未引入参考与动画补充候选。决策记录新增 `docs/verification/rust-bakeoff-decision.md`；`SOURCES.md` 更新「已引入」并更正 Ayagami 真实许可为 **MIT OR Apache-2.0**（上游根 COPYRIGHT/LICENSE-MIT/LICENSE-APACHE 文件证据）。
- **crates/l2d 新增自有封装 API**（第三方类型不越过 crate 边界）：`asset::ModelPackage`（model3 包加载 + 兼容报告）、`model::ModelHandle`（moc3 运行时句柄，版本/画布/规模）、`renderer::OffscreenRenderer`（headless wgpu 离屏渲染：固定 dt update → RGBA 帧读回 → PNG 落盘）。既有 `format`/`report` 骨架不动。
- **示例** `crates/l2d/examples/render_model.rs`：CLI 参数 model3 路径/输出路径/尺寸，默认指向仓库内 Bai 皮套（相对路径）。实测运行生成 `verification/rust-bakeoff/selected-bai.png`（1024²，alpha coverage 8.23%），经独立 imgcheck 复核与 bakeoff 产物逐像素一致（visible 84,923 px / bbox x[350..676] y[294..822] / 46 色）。
- **RFC D5 修订**：Bai 实测 MOC raw 版本字节 = 4（Cubism 4.2），v0 必须先支持 raw 4 档位；raw v5/v6 为下一兼容方向（原文「raw v5/v6 优先」作废）；批次 2 与修订历史同步更新。
- 门禁：`cargo fmt --check` / `cargo check` / `cargo test -p l2d`（18 passed）/ `cargo clippy --all-targets` 全绿；未触碰 core/desktop/xtask。

---

# 原生工具 400 崩溃修复：函数名合规 + 错误链路隔离（2026-08-25）

- **根因**：DeepSeek/OpenAI function-calling 要求 `function.name` 匹配 `^[a-zA-Z0-9_-]+$`；工具 schema 曾用点号名 `live2d.perform_action` → provider 直接 HTTP 400，且旧代码把 `Error calling the chat endpoint...` 错误字符串当 assistant 正文送进 TTS/字幕/记忆。
- **命名双轨**（`live2d_action.py`）：API schema 改用合法名 `live2d_perform_action`；点号展示名保留给插件面板/日志。工具调用解析双名兼容（新 API 名 + 旧展示名仅内部兼容、schema 绝不再发送点号名）；协议回显（assistant.tool_calls）经 `canonical_tool_api_name` 归一为 API 名，防历史残留调用触发二次 400。模块加载即断言 schema 名恒满足字符集约束。
- **400 自动降级**（`openai_compatible_llm.py`）：携带 tools 的请求被 provider 以 HTTP 400 拒绝时，自动去掉 tools 重试一次并把本进程内 `support_tools` 置 False（后续轮次走纯文本简单路径，不再提供工具）；未带 tools 的 400 不误判、不重试。
- **错误链路隔离**：LLM API 错误改为抛 `LLMChatError` 异常（连接失败/限流/HTTP 错误各有单一可读中文文案），经 transformers pump 转成唯一一条 `stream_error` → single_conversation 转发前端 `{"type":"error"}`；错误绝不 yield 成正文，不进 TTS/字幕/记忆。
- **续聊熔断**（`single_conversation.py`）：本轮出现 LLM 流失败/stream_error 后不再触发额外多段续聊（此前默认还会再续 2 段放大错误）。
- **防回归测试**：新增 `tests/test_native_tool_400_fallback.py` 11 例（SDK 级 BadRequestError 模拟：命名正则端到端、400→无 tools 重试恰一次、重试仍失败报单一可读错误、错误不产句子/不入记忆、错误轮不续聊×2、降级后下轮无工具、health 计数）；`test_live2d_action_tool.py`/`test_native_tool_loop.py`/`test_plugin_health.py` 更新为双名契约并锁定旧名回显归一。PC 全量 pytest **677 passed**。

---

# 设置中心人设收敛 + 思考模式 + 默认视觉主模型（2026-08-22 晚）

- **人设面板瘦身**：仅保留 System Prompt；角色名/角色定位/模型/Temperature/Max Tokens/视觉模型 全部移除（后端 persona 读写同步收敛，旧字段不再进配置）。
- **LLM 设置新增**：Top P（provider 卡片内）、思考模式开关+思考强度（low/high/max，作用于当前选中 provider，DeepSeek 官方 thinking/reasoning_effort 契约）、「对话内展示思考」开关（纯前端偏好，localStorage showThinking）。
- **思维链全链路**：ReasoningDelta 流式旁路（LLM→agent→transformers→single_conversation→ws reasoning-delta），不混正文、不入记忆、不进 TTS；前端折叠思考面板，正文开始自动折叠。
- **默认主模型带视觉**：deepseek_llm.model → deepseek-v4-flash-vision-exp（官方实验模型，同 key 同端点）；视觉模型独立字段废除。
- **web-search 免费引擎切至 Bing 中国站**（DuckDuckGo 国内不可达 Errno 101）；Tavily 保留为 key 制拓展位。
- **防回归**：boot-guard 静态锁双保险可选链；test_shared 人设关键词断言改结构性检查（persona=用户数据）；settings_api 三处契约断言更新；System Prompt 显式清空 → 400 友好拒绝（空 persona 会让启动 FATAL）。

---

# 深度审计 + 工程优雅度重构完成（2026-08-22，无人值守轮次收尾）

- **审计落盘 AUDIT.md**：AST 死代码扫描（305 定义 × 全语料引用计数）+ 重复普查 + 核心链路通读；PC 6 项 / Renderer 2 项 / Android 1 项全带 文件:行号 证据；命名/依赖耦合/可测试性三维度未发现明显问题。
- **R1+R4 路径单源**：新增 utils/repo_paths.py，归一五处仓库根/shared 解析实现（含 run_server.py「硬编码上两层」脆弱假设消除）；deep_merge 私有化。
- **R2 死代码清除**：7 处零引用定义删除（AudioPayload/ConversationConfig/live 整包/NoOpTranslator/get_github_asset_url/has_punctuation/chat_history_manager×2）。
- **R3 print 归一**：fun_asr ×2 转 loguru；utils/install_utils.py 整文件废弃零引用 → 删除；cosyvoice `__main__` CLI 与 silero 演示 handler 豁免。
- **克制记录**：R5（routes helper 提升）、R6（settings-ui 拆分）评估后缓行——收益低于回归风险；Android 冻结面不动。全部理由见 REFACTOR_PLAN.md。
- **总结对账 REFACTOR_SUMMARY.md**：最终门禁 PC pytest **421 passed**、renderer vitest **408 passed**、tsc clean；工作树干净。

---

# P2 台词质量治理落地：SLOP 挖掘器 + 运行时规则表（2026-08-22）

- **离线挖掘器** `scripts/slop_miner.py`（stdlib-only）：递归扫 chat 历史（chat_history_manager 的 .json 数组与通用 .jsonl 双格式），按 role 过滤 → 高频子句 / 句首片段 / 内置五类套话家族命中三张榜；候选仅供人工策展，绝不自动生效。
- **策展规则表** `shared/slop_rules.json`：12 条高置信种子规则（总结腔开头×3、论文腔插入×2、时代背景、AI 自指、让步套话×2、权威腔开头、客服式收尾），每条带策展注记。
- **运行时治理** `utils/slop_rules.py` + `tts_filter` 尾部接线：懒加载缓存（进程读盘一次，`reload_runtime_rules()` 可热刷新）、`SLOP_RULES_PATH` 环境变量覆盖、只作用于 TTS 音频文本（字幕/记忆不动，沿用 tts_filter 既有语义）；永不静默崩溃——文件缺失→空规则 debug 可见、单条正则非法→warning 跳过不断链。
- 测试 `tests/test_slop_rules.py` **14 例**：加载边界（缺失/坏 JSON/坏条目/env 覆盖）、应用语义（命中移除/自定义替换/孤立行首标点清理）、tts_filter 集成、挖掘器合成数据统计与 role 过滤。PC 全量 **421 passed**；ruff（F/E/W/I）干净。

---

# P1 收尾修复：soullink profile 加载根因 + 引擎隔离（2026-08-22，接续无人值守轮次）

- **404 回退根因修复**：`loadSoullinkProfile` 此前对任何入参都按「同目录 soullink.profile.json」推导 URL，而 p0-followup 已把调用方改为传打包直链 `/soullink-profiles/<皮套>.profile.json`——两者错位导致 soullink 模式**必然** 404 回退 legacy（引擎从未真正生效）。现在 `*.profile.json` 直链原样使用、模型 URL 保持旧推导；新增 4 个回归单测锁死（直链/模型 URL/404 可见回落/幂等缓存）。
- **引擎按需加载隔离**：`@soullink-emotion/engine` 改为异步工厂 `createSoullinkPerformanceAdapter` 动态 import——legacy 默认路径不再下载 engine chunk（产物验证：引擎独立分块 ~109 kB，仅 soullink 模式拉取）。
- **永不静默崩溃补强**：renderer index.html 注入首错上屏脚本（window error / unhandledrejection 直接画到页面顶部），先于主包执行。
- 验证：renderer vitest 28files/**408 passed**；tsc strict clean；bundle 已重建；PC pytest 保持 407。

---

# P0-P1 落地：soullink 表演引擎双跑（2026-08-22）

- **P0 Spike GO**：profile-generator 确定性生成 bai ModelProfile（FACS 24→20 mapped；usedCdi 16/128）；headless 冒烟 600tick 输出 16 参数全 ⊆ bai 集、眨眼幅度 0.887/呼吸 0.900；SDK 能力检测与人工 cdi 盘点互证（缺 eyeSmile/blush/tear/sweat）。报告见 verification/soullink-p0-report.md。
- **P1 双跑开关落地**：renderer 新增 soullink-adapter.ts（8key→naturalVAD 意图映射 / 括注 cue 重音 / RMS 推拉桥喂引擎口型 / frame 覆盖写入）；ws-bridge 三转发回调；main.ts performanceEngine 开关（localStorage，默认 legacy）。
- **核心缺陷修复**：参数写入挂到 internalModel 的 afterMotionUpdate 相位——此前 arbiter 写入被模型每帧重置覆盖，表现就是"只有嘴动"。修复后眨眼/呼吸/表情/动作全部可见（待人工冒烟最终确认）。
- 验证：renderer vitest 28files/**404 passed**；tsc strict clean；bundle 已重建；PC pytest 保持 407。

---

# 实现计划定稿：表演引擎直接复用 soullink-emotion-sdk（2026-08-22）

- 决策：能引包就不自己写。7 个 @soullink-emotion/* 包全部已发布 npm（MIT，0.1.0-beta.1），锁精确版本引入；维护=评估上游 release+diff 后 bump。
- 落盘 docs/plans/PLAN-V3-SOULLINK-PERFORMANCE.md：复用清单 / 目标架构 / P0 Spike→P1 双跑开关→P2 默认切换退役→P3 插件扩展 四阶段（各含验收与回滚）/ 上游同步 SOP / 风险表 / 总验收口径。
- **硬原则入计划**：永不静默崩溃（失败必须可见+降级有出口）；用户侧 LLM+TTS 默认必配（设置界面承载），核心不为缺席场景造兜底——离线情绪分类器降为非核心可选插件。
- **新增 §7**：N.E.K.O 借鉴映射（Smart-Turn/RNNoise+Silero/SLOP 口头禅治理进核心语音与文本打磨；proactive/记忆五层分层采纳）+ skill-loader 与 mcp-bridge 可行性评估（pi-agent 式前置插件）+ PluginHost 缺口诚实评估（缺后台服务注册接口，直播接入前置改造另列）。
- **新增 §8 多 LLM 并行管线正式化**：现状已有 A 主对话脑/B 表演导演/D 记忆管家三角色在跑（C 心情导演为规则版）；对齐 PLAN-PC-V2 原三 LLM 设计与生态惯例（N.E.K.O 三服务器、soullink planner、VT-Orchestrator 等）。关键改造=P1 内 B 并行化（sentenceId 回填+迟到丢弃，TTS 不等导演）+ per-role 模型覆盖 + fallback 可见化；C 保持规则版（克制）。
- 合规注记重申：SoulLink_Live2D 无 LICENSE 仅对照；AGPL/GPL 插件只看思想。

---

# 调研落盘：无预设参数 Live2D 表情表演（soullink-emotion-sdk）（2026-08-22）

- **检索结论**：MIT 协议的 @soullink-emotion/engine（零运行时依赖纯 TS）提供 VAD 连续情绪空间 + FACS/AU 表情语义 + ModelProfile 跨皮套参数映射 + MotionMixer 分层混合 + SpeechPerformancePlanner 说话演出规划——正是"无预设参数、跨皮套通用"的成熟实现；同作者前作 SoulLink_Live2D **无 LICENSE 仅可对照**（外部资料误标 MIT，已纠正）。
- **采纳路线定稿（P1-P3）**：P1 renderer 引入 engine 替换自研 idle 四件套内部实现并用 profile-generator 对皮套出档；P2 planner 模式对接 DeepSeek + lifecycleToken 防竞态移植；P3 classifier-embedding 做无 Key 情绪分类兜底插件、LightRAG 升级知识库 v2。
- 明确不做（决策记录）：手臂/骨骼控制、3D 化、动捕映射——跨皮套不通用。
- 落盘 docs/research/live2d-halfbody-motion-research.md（仓库分诊/SDK 数据流/模块对照/许可证合规/引用）；索引已更新。

---

# 编排导演层：大幅度时间轴半身动作（2026-08-22）

- **从"单姿态手势"升级为"编排动作"**：新增 renderer/src/choreography.ts——CHOREOGRAPHY_LIBRARY 动作库（大幅挥手 wave_big / 鞠躬 bow / 蹦跳 bounce_happy / 用力摇头 shake_no / 凑近倾听 lean_in / 左右张望 look_around），每个动作是【多参数 × 时间轴关键帧】序列，幅度贴近参数两端（0.14~0.86），末帧回中性；ChoreographyPlayer 按真实时钟逐帧写入 Arbiter（fade 平滑过渡），打断安全、播完自动释放回 idle/mood 基线。
- **零改动接线**：BodyActionPlayer 组合门面（编排优先、其余回落单姿态手势），接口与 BodyMotionPlayer 完全一致——ws-bridge / live-event / main ticker 无感知切换。括注触发链路（motionTimeline 播放位置触发）直接复用：LLM 写（开心地挥手）即触发 wave_big 大幅挥手。
- **括注关键词扩展**：director.py 新增 鞠躬/摇头/凑近/张望/蹦跳/挥手打招呼 六组编排映射。
- renderer vitest **27 files / 401 passed**（含 choreography 6 项）、tsc strict 通过、产物已重建；PC pytest **401 passed**。

---

# LLM 记忆管家 + 前端管理页修复 + Vision 出链路（2026-08-22）

- **前端记忆/知识库管理不可见 → 根因修复**：上一轮改动后未重建前端产物，浏览器拿到的是旧 bundle。已重建并验证 bundle 含「记忆」页（长期记忆事实 / 知识库文档 / 联网查询试搜三区）；**需刷新页面**即可见。
- **LLM 记忆管家（对齐 N.E.K.O 的 LLM-arbitration 思路）**：新增 memory_curator.MemoryCurator——对话链结束后由独立小模型对照现有事实输出 add/remove 决策（去重/合并矛盾/清理过时），异步执行不阻塞返回；失败静默可见。与正则即时层互补：正则先记全、LLM 后管好。LIVE2DAI_MEMORY_CURATOR=off 可关。
- **Vision 移出核心链路**：shared/persona.yaml（含 Android 副本）删除 vision_model 覆盖；启动横幅不再显示 Vision model；仅当显式开启 vision_enabled 时才应用/提示。
- 新增 tests/test_memory_curator.py（7 项，离线桩）；PC pytest **400 passed**；一致性校验 33/33。

---

# 日志可读性升级（2026-08-22）

- **统一日志配置 logsetup**：新增 src/open_llm_vtuber/logsetup.py——控制台改为紧凑彩色单行「HH:mm:ss | 级别 | 标签 │ 消息」（默认 INFO，LIVE2DAI_LOG_LEVEL/设置中心可控），文件 logs/server.log 保留毫秒时间戳+模块:函数:行号全定位、10MB 轮转保留 5 份（logs/ 已 gitignore，不再在根目录堆 server.log）。
- **uvicorn 访问日志降噪**：逐请求 GET 刷屏在非 DEBUG 时压到 WARNING；DEBUG 排障时自动放开。
- **热路径标签化 + 降噪**：tts_manager→"tts"、stream_audio→"audio"、conversation_utils→"flow"；逐 chunk 的 [LATENCY] 多段耗时从 INFO 降为 DEBUG 单行中文格式。控制台运行态一眼能读，排障细节进文件。
- 新增 tests/test_logsetup.py（7 项：文件格式/级别过滤/标签/env 级别/uvicorn 降噪/幂等/app 兜底标签）；PC pytest **393 passed**。

---

# 知识库/联网查询做实 + 记忆可视化 + 字幕声音同步（2026-08-22）

> 参照同类项目 G:/git/Live2Dai 与本项目目标（docs/plans/PLAN-V2-PC-LOCAL-TTS.md）对齐：把"假实现"换成真实现，管理能力进设置中心。

- **联网查询做实（web-search stub→runtime）**：WebSearchPlugin 无 Key 走 DuckDuckGo HTML 检索（纯标准库、6s 超时、uddg 重定向还原、摘要去标签），配 TAVILY_API_KEY 自动升级 Tavily；任何失败可见降级（results=[]+error，不打断对话）。新增 REST POST /api/tools/web-search，设置中心「记忆」页可直接试搜。
- **知识库做实（knowledge stub→runtime）**：KnowledgePlugin 本地文档库（KNOWLEDGE_DIR，默认 cache/knowledge，.md/.txt）；llm.before 按字符重叠检索 top-2 注入「[知识库检索] (来源: 文档名)」；REST GET/POST/DELETE /api/knowledge/documents；设置中心可视化增删。
- **记忆管理可视化（设置中心新「记忆」页）**：长期记忆事实列表 + 手动添加/单条删除/清空全部，改动即时生效（REST /api/memory/facts*，PluginHost 新增 get_instance 访问器，MemoryPlugin 补 list/add/remove/clear 公开方法）。
- **字幕与声音同步**：此前流式气泡的干净句子文本要等整条对话链结束才收尾，观感"文字比声音晚很多"。现在每句首个音频块带 sentence_start 标记，前端在该句语音真正开播时即收尾上屏（finalizeStream），静默帧立即上屏——对话与声音一起出或提早出。
- **Live2D 动作联动补实**：BodyDirectorPlugin 从原样透传升级为确定性兜底——主脑没给 motion 时按情绪（8 key）→手势映射（joy→happy、sadness→listen…），再退心情分层（happy/ecstatic→excited），保证"有情绪就有肢体表达"；渲染层动作已是真实的参数驱动半身动作（上一版已接播放位置触发）。
- 插件注册表：knowledge/web-search 分类 stub→runtime（runtime 共 6 个），设置中心插件页不再显示"未实现，不可启用"。
- 新增 tests/test_knowledge_websearch.py（19）、test_memory_api.py（12，含真实 MemoryPlugin 路由集成）、test_body_director_fallback.py（5）；PC pytest **386 passed**、renderer vitest **26 files / 389 passed**、tsc strict 通过、前端产物已重建。

---

# 括注动作按播放位置触发 motionTimeline（2026-08-22）

- **动作与 TTS 时间轴绑定（my-neuro 同款）**：此前 PC 端括注动作随 audio 帧到达即整句一次性触发；现在 director.stage_gesture_timeline() 解析句中**全部**动作括注为带文本位置的帧，handle_sentence_output 按 split_tts_chunks 分块切片成 actions.motionTimeline（at=块内归一化播放进度），前端 motion-timeline.ts 挂在真正播放的音频元素上，进度到点才 BodyMotionPlayer.play——「说到『点头』才点头」。
- **兼容与降级**：时间轴存在时清掉一次性 actions.motion 防双触发；无括注/投影失败/旧前端均完整保留旧行为；打断（stopAudio/interrupt）自动清理未触发帧。Actions.to_dict() 对 None 自动省略字段，协议向后兼容。
- **顺手修复存量 bug**：conversation_utils.py 的展示文本括注清洗分支引用了未导入的 DisplayText（一出现括注即 NameError 进异常兜底），已补导入——该路径此前从未真正生效。
- stage_gesture() 保留为首命中兼容入口（内部改走 timeline）；DirectorResult 增加 motion_timeline 字段。
- 协议文档：shared/emotion-protocol.md 新增 §J（motionTimeline 数据形态/生成链路/消费语义/双端落点，Android 可对齐 M7 scheduleKeywordMotions）。
- 新增 tests/test_motion_timeline.py（15 项）+ renderer tests/motion-timeline*.test.ts（11 项）；PC pytest **350 passed**、renderer vitest **26 files / 386 passed**、tsc strict 通过。

---

# 括注舞台说明 + 导演半身动作 + 多段对话（2026-08-22）

- **括注不读不显示**：filter_parentheses 现在同时滤除英文 () 与中文全角 （）；handle_sentence_output 同步清洗展示文本，LLM 的「（看到热字，扇风）…」不再被朗读，也不会显示在字幕/历史里。
- **导演半身动作**：新增 director.stage_gesture()，把（）内的动作词映射到参数驱动手势（扇风→sway、点头→nod、挥手→happy、鼓掌→excited 等），随 audio actions.motion 下发，前端 BodyMotionPlayer 执行（bai 无 motion3，走参数形变）。
- **多段对话**：一次用户输入默认触发额外 2 段续聊（MELO_MULTI_TURN_EXTRA=2，可设 0~5，回复 [DONE] 提前结束），每段独立走字幕/TTS 队列并按序播放；用户新消息/打断仍取消整个 conversation task。
- 新增 tests/test_stage_directions.py；PC pytest **335 passed**，verify_all.py --pc 5/5。

---

# 本地 Melo 合成链路加速 + CPU 确认（2026-08-22）

- **CPU 确认**：conf.yaml 的 melo_tts.device=cpu，worker 加载 TTS(device=cpu)；链路慢是 CPU 合成固有耗时（约 1~3 秒/句），并非误走 GPU 或云端 TTS。
- **LLM 回答小节化**：handle_sentence_output 现在用 split_tts_chunks()（默认每块约 36 字、优先句/逗号断点）把每句切成小块依次入 TTSTaskManager 队列，首块更早开始合成与播放，缓解长句等待。
- 新增 tests/test_tts_chunks.py；PC pytest 升到 **330 passed**，verify_all.py --pc 5/5。

---

# 本地 Melo TTS Provider + REST 接口（2026-08-21 · PC 端本地 TTS 计划收口）

> **范围**：仅 PC 端。执行口径 `docs/plans/PLAN-V2-PC-LOCAL-TTS.md`，Android 冻结不涉及。

## 已落地
- **Melo 引擎与配置**：`melo_tts.py` 对齐本地 API（ZH 优先、`spk2id` 解析、speed/sdp_ratio/noise_scale/noise_scale_w/format 全量传递与钳制）；`MeloTTSConfig` 补齐字段与中英文说明。
- **热重载修复**：`service_context.py` 把 TTS/ASR/VAD 变化检测改为基于最近一次生效配置，修复「保存并应用」后仍用旧 TTS 引擎的问题；新增回归测试 `tests/test_config_reload.py`。
- **插件失效调查**：`knowledge`、`web-search` 为原生占位 stub（无 entrypoint，属设计状态而非崩溃）；其余 11 个原生插件正常加载。
- **外部 Melo 环境对接**：新增 `melo_worker.py` + `melo_external.py`，在本仓库 Python 3.12 不装 Melo 的情况下，通过常驻子进程调用 `~/miniconda3/envs/melo/bin/python`（可用 `MELO_PYTHON` 覆盖），复用 TTS 实例、支持完整四参数；已用真实 ZH 模型跑通端到端合成。
- **CPU 与拆分限制**：默认 `device=cpu`；worker 对单次合成限制 `MELO_MAX_TEXT_LEN`（默认 200 字）与 `MELO_MAX_SENTENCES`（默认 4 句），避免 Melo 把长文本拆成过多小句。
- **设置中心接入**：TTS 目录/默认值加入 `melo_tts`，`GET /api/config` 返回 Melo 语言/设备/韵律参数，保存时把 UI 的 `voice/model` 映射落盘为 `language/device`，`/api/test-tts` 支持 Melo 真实试听。
- **本地 REST 接口**：新增 `POST /api/tts/local/synthesize`（懒加载引擎 + 并发锁 + 500 字上限 + WAV 时长返回）与 `GET /api/tts/local/audio`（仅 `cache/tts_local_*.wav` 白名单，拒绝路径穿越）。
- **前端设置 UI**：`Melo TTS（本地）` 进入「本地 / 免费」分组，语言/设备下拉 + 四韵律滑杆 + speaker，试听按钮真实合成。
- **conf/template**：`conf.yaml`、`conf.yaml.template`、`config_templates/conf.default.yaml`、`conf.ZH.default.yaml` 均补 `melo_tts` 默认块（默认仍 `cosyvoice_cloud` / `edge_tts`，不破坏旧配置）。
- **基线修复**：`test_tts_bargein.py` 不再依赖 `.pi` 目录标记即可收集；`test_shared.py` 的 S1-03 对齐 PC v4「persona 不内置静态 emotion key」。

## 验证
| 项 | 结果 |
| --- | --- |
| `scripts/verify_all.py --pc` | **5/5 PASS** |
| PC 后端 pytest | **326 passed**（新增 Melo 引擎/接口/外部 client/限制/热重载回归） |
| PC 前端 vitest | **24 files / 375 passed**（新增 settings-melo 3 项） |
| PC 前端 tsc + build | 通过，产物已重建 |
| 跨端一致性 / registry | PASS |

## 真机冒烟
- **外部 worker 真实合成 PASS**：用 `~/miniconda3/envs/melo/bin/python` 运行 `melo_worker.py`，ZH/cpu 合成 `/tmp/melo_smoke_worker.wav` 成功；再经 `MeloTTSEngine.generate_audio` 端到端生成 `cache/melo_e2e.wav` 成功。

## 未验证 / 诚实登记
- **其他语言模型下载**：当前只验证了已缓存的 ZH；EN/ES/FR/JP/KR 首次调用会自动从 hf-mirror 下载，尚未逐个真机合成。
- **cuda 路径**：外部环境显存仅 3GB，代码支持 `device=cuda`，但未做多进程并发 OOM 验证；并发建议保持单实例 + 队列串行。

---

# 流式上屏 + 语音链路统一（2026-08-20 · 第四轮）

> **范围**：PC 端。用户指示「都做」→ 本轮补齐 P1-2（逐 token 流式上屏）与 P0-2（语音/VAD 打断链路统一）。
> 缺口编号沿用 `docs/research/neko-ui-alignment-and-gaps.md`。

## P1-2 逐 token 流式上屏 ✅
- **后端只加一路"旁听"，主链一字未改**：`emo_interceptor` 在把干净文本交给
  `sentence_divider`/TTS 的同时，多产出一个 `{"type":"text_delta"}` 事件；
  `single_conversation` 转成 WS `text-delta`。分段、TTS、口型链路完全不受影响。
- 前端 `chat-ui.streamDelta()`：首个增量创建流式气泡（带闪烁光标 + "正在说…"），后续追加。
- **不重复上屏**：流式期间句子级 `subtitle` 不再单独建气泡，而是累积为**权威最终文本**；
  收到 `control: conversation-chain-end` 时用它替换流式内容（消除流式残留的 TTS 过滤前噪声）后定稿。
- 错误/打断路径也会收掉流式气泡，不留"半句悬着"。

## P0-2 语音输入与 VAD 打断链路统一 ✅（真机麦克风待验）
- **修掉的根因**：前端麦克风此前走**浏览器 Web Speech API**，识别在浏览器做完再当普通文本发出 ——
  后端 ASR 配置对用户毫无作用、VAD 永远收不到音频、打断也就永远不会发生；
  同时前端**完全忽略 `control` 消息**，所以后端一直在发的 `interrupt` / `start-mic` / `mic-audio-end` 无人接收。
- 新增 `renderer/src/voice-input.ts`：getUserMedia → AudioContext → 单声道 Float32 →
  **线性重采样到 16kHz**（后端 ASR/VAD 约定）→ 2048 样本/片（128ms）经既有协议
  `mic-audio-data` 上行，停止时 `mic-audio-end` 触发后端 ASR + 对话。
  含录音上限 60s、尾帧 flush（否则最后半个字丢）、轨道停止（否则标签页一直显示"正在使用麦克风"）、
  **每帧拷贝 AudioBuffer**（浏览器会复用通道缓冲，不拷贝会让已入队帧被覆写）。
- `ws-bridge` 新增 `sendMicAudio/sendMicAudioEnd/sendInterrupt` 与 **`control` 消息处理**；
  收到 `interrupt` 时 `stopAudio()` 真正**暂停当前音频元素并清队列**（光清队列不够）。
- 可见反馈：麦克风按钮聆听态红色波纹 / 识别态蓝色转圈，头部新增语音状态胶囊
  （聆听中 · 识别中 · 已打断），VAD 打断另给 Toast。
- **降级保留**：环境不支持 getUserMedia（非 https 非 localhost、老浏览器）时自动回落 Web Speech，
  能力不减、只是链路退化；按钮 title 会注明"后端识别 / 浏览器识别"。

## 验证
| 项 | 结果 |
| --- | --- |
| PC 后端 pytest | **297 passed**（上轮 288 → 新增 9：流式旁听 5 / 语音上行 4） |
| PC 前端 vitest | **16 files / 340 passed**（上轮 320 → 新增 20：voice-input 重采样/分片/状态机） |
| PC 前端 tsc | 通过（顺带修掉 TS 5.7+ `Float32Array` 泛型化引发的类型分歧） |
| 前端产物 | 重新构建；产物内含「正在说 / 聆听中 / 识别中 / 已打断」 |
| **真机 E2E** | **7/7 通过** —— 回合收尾 `control: conversation-chain-end` 到达（流式气泡据此定稿）；无 Key 时以配置指引 `error` 结束且**不凭空产出 text-delta**；合成 16kHz 正弦 PCM 按 2048 分片上行 10 片 → `mic-audio-end` → 服务端日志出现 **`Transcribing audio input...`**（ASR 确实被调用）→ 该轮同样有收尾信号 |
| 装配级 E2E | 20/20 仍通过（外部接入 + 背景图无回归） |
| 跨端一致性 | 32/33（唯一失败仍是 Android `assets/model_registry.json` 产物缺失，非代码问题） |

### 本轮**未**验证到的部分（诚实登记）
- **真实麦克风采集**：本环境无音频设备与浏览器，`getUserMedia`/`AudioContext`/权限弹窗/回声消除
  这一段**只做了静态实现与纯逻辑单测**，未真机跑过；ASR 的**识别质量**也无法用正弦波判断
  （只证明"音频送达并触发了 ASR"）。首次在真机使用时请重点看：授权弹窗、聆听波纹、
  识别耗时（faster-whisper large-v3-turbo 首次调用较慢）、以及说话打断是否即时停声。
- **VAD 打断的真实触发**：当前 `conf.yaml` 的 `vad_config.vad_model` 为空（VAD 关闭），
  故 `control: interrupt` 的**发送端**未被激活；前端接收与停声逻辑已就绪并单测覆盖。
  要启用需在 conf.yaml 配 `silero_vad` 并走 `raw-audio-data` 上行（下一轮可做）。

---

# M 量级缺口补齐：聊天历史持久化 + 首启引导 + 形象热切换（2026-08-20 · 第三轮）

> **范围**：PC 端。用户定调「安卓暂时不管，先把 P0-3 这类 M 量级补齐」。
> 缺口编号沿用 `docs/research/neko-ui-alignment-and-gaps.md`。

## P0-3 聊天历史持久化 + 回看 ✅
- **修掉的真实缺陷**：`chat_history_manager.py` 早就能落盘，但 `ServiceContext.history_uid` 初值为空串，
  而 `process_single_conversation` 只在它非空时 `store_message` —— 此前只有前端显式发 `create-new-history`
  才会绑定，新 renderer 从不发，于是**一条消息都没落过盘**（`chat_history/` 目录都不存在）。
- 后端 `WebSocketHandler._attach_session_history()`（新连接即绑定，在 `start-mic` 之前）：
  **续接最近一段**历史（刷新页面 = 接着聊），没有则新建；同时 `set_memory_from_history` 把 LLM 记忆接上
  （避免"界面有上文、模型不记得"）；把该段消息（过滤 metadata/system）以 `history-data`（新增 `history_uid` 字段）回灌前端。
  任一步失败都只降级为"本次不落盘"，**不让连接建立失败**。
- 前端新增 `renderer/src/history.ts`：连接即恢复上文 + 历史抽屉（会话列表 / 切换 / 新建 / 删除，
  标题取末条消息、时间显示今天`HH:MM`／昨天／`M月D日`）；`ws-bridge` 补齐 4 个上行方法与 4 个下行回调
  （**复用后端既有 WS 协议，未新增后端消息类型**）；聊天面板新增 `restoreMessages()`（整段恢复不重复播动画）。
- `chat_history/` 已加入 gitignore（本机私密对话内容）。

## P0-5 首次启动引导 ✅
- 新增 `renderer/src/onboarding.ts`：首次打开时给一张**缺什么就说什么**的清单
  （选 LLM → 填 Key → 选 TTS → 人设），每项一键跳到设置中心对应分区。
- **只在阻塞项（Provider / Key）缺失时才弹**；缺 TTS/人设不打扰。支持「稍后再说」与「不再提示」（localStorage）。
- 刻意**不重复造表单**：引导只做"诊断 + 跳转"，真正配置仍在设置中心完成（避免第二套 Key 输入逻辑）。
- 配套：`SettingsPanel.open(tab?)` 支持直接落到指定分区。

## P1-4 形象热切换即时生效 ✅
- 新增 `WebSocketHandler.broadcast_model_change()`：更新共享 cache + 逐会话 `init_live2d` 并主动推
  `set-model-and-conf`（前端 `main.ts` 早就支持热切换，此前缺的就是"服务端推这一下"）。
- `POST /api/models/select` 不再回 `requires_restart: true`，改回 `{applied, clients_updated, message}`；
  热切换异常时**诚实**回退为"已保存，重启后生效 + 原因"。设置页文案同步（不再误导用户去重启）。
- 单会话 socket 断开或 `init_live2d` 失败**不影响其余会话**（返回成功计数）。

## 验证
| 项 | 结果 |
| --- | --- |
| PC 后端 pytest | **288 passed**（上轮 269 → 新增 19：聊天历史 10 / 形象热切换 9） |
| PC 前端 vitest | **15 files / 320 passed**（上轮 291 → 新增 29：history 16 / onboarding 13） |
| PC 前端 tsc | 通过 |
| 前端产物 | `npm run build` 成功；产物内含「聊天历史 / 开始新会话 / 已恢复上次对话 / 不再提示」 |
| **真机 E2E：聊天历史** | **12/12 通过** —— 连接即收到 `history-data`；外部注入一句话后**人类消息确实落盘**（LLM 无 Key 失败也不丢）；**断开重连续接同一段并带回上文**；`fetch-history-list` 列表含该段（带末条消息与时间）；`create-new-history` 拿到不同 uid；`delete-history` 成功且文件从磁盘移除 |
| **真机 E2E：形象热切换** | **5/5 通过** —— `select` 回 `applied:true / requires_restart:false / clients_updated:1`，WS 端**确实收到主动推送**的 `set-model-and-conf`（带 `model_info.url`） |
| 运行态卫生 | 验证产生的 `chat_history/`、`external_input.json`、`background_state.json` 已清理；`conf.yaml` 经比对未被测试改写 |

## 仍未做（诚实登记）
- **P0-1 桌面壳**（透明置顶 / 点击穿透 / 托盘 / 开机自启 / 全局快捷键）——L 量级，需 Electron/Tauri 独立立项。
- **P0-2 语音与 VAD 打断链路统一**——前端 mic 仍走浏览器 Web Speech，与后端 Silero VAD 两条链路未合并（M，且部分依赖桌面壳权限）。
- **P1-2 流式逐 token 上屏**——需后端把 token 级事件透传到 WS（M，会触碰句子切分管线，风险高于本轮其它项）。
- **P1-5 多人设卡**（L）、**P2-1 情绪调试面板**（M）、**P2-3 i18n**（M）。
- **E-5 / R-15 渲染热路径优化与死代码**——需真机视觉确认，留待有 GPU/浏览器的轮次。
- **Android**——按用户指示本轮不动。

---

# 外部接入 + 背景图 + 去免费模型 + UI 对标 NEKO（2026-08-20 · 第二轮）

> **范围**：PC 端（`Live2D-Ai-pc/open-llm-vtuber`）为主；Android 端**未改代码**（本环境无 SDK 无法编译验证，见文末）。
> 相关文档：`docs/audit/2026-08-20-readability-efficiency-audit.md`（可读性/高效性审计）、
> `docs/research/neko-ui-alignment-and-gaps.md`（UI 对标 + 缺口清单）、
> `docs/architecture/external-text-input.md`（外部接入接口契约）。

## ① 常规代码审计（可读性 / 高效性）
- 产出报告 `docs/audit/2026-08-20-readability-efficiency-audit.md`：20 条可读性 + 9 条高效性发现，
  每条含 `文件:行` / 现象 / 影响 / 最小 diff 建议 / 风险 / 成本，附 Top10 修复顺序与「本轮不建议动」清单。
- **本轮已落地的修复**（其余登记在报告中）：
  - **E-1（高危）** `routes.py`：`discover-models` / `providers/check` / `test-llm` 三个 async 端点里的同步
    `requests` 全部移出事件循环（`asyncio.to_thread`）；`check_providers` 从**串行 ~16 个 provider ×
    timeout 8s（最坏 ~128s 卡死全站 WS）** 改为 `asyncio.gather` + `Semaphore(4)` 并发，总耗时由 Σ 变 max。
  - **E-2/E-3** `routes.py`：`.env` 由「每个 provider 各全量重读一次（单请求近 30 次）」改为 `_load_env_map()` 一次读入。
  - **E-4（中高危）** `websocket_handler.py`：音频缓冲由每 chunk `np.append`（O(n²) 重分配 + 拷贝）改为
    **按 chunk 存 np 数组 + 取用时一次 concatenate**（O(n)）；新增 `tests/test_audio_buffer.py` 锁定
    拼接顺序 / 取后清空 / 空缓冲兜底 / 多会话隔离。
  - **R-2/R-3/R-4** `routes.py`：TTS id 归一化字典（曾 4 处各写一遍）收敛为模块级 `normalize_tts_id()`；
    `openai_compatible` 别名判断统一走 `_normalize_provider_id()`；「列模型」的 URL/header 构造与响应解析
    抽出 `_models_endpoint()` / `_models_from_response()`（三端点复用，堵住 anthropic `/v1` 已出现的漂移苗头）。
  - **死代码** 删除 `websocket_handler.MessageType`（定义后从未使用）与 `routes._has_key`（已被 env_map 取代）。
- **明确撤回（诚实记录）**：`E-5 ParamArbiter dirty 标志` —— 单测 29/29 可过，但它改变「仲裁器每帧重申参数
  所有权」的语义，与 Cubism 内建 blink/breath/motion 的写入竞争只能靠真机视觉确认；本环境无 GPU/浏览器，
  **故改完撤回、不合并**，留待真机验证。

## ② 后端暴露接口：外部文字 → 内部 LLM 链路
- 新增 `src/open_llm_vtuber/external_input.py`（网关 + 4 端点）与 `WebSocketHandler.inject_text_input()`。
- **复用同一条链路**：注入直接走 `_handle_conversation_trigger`（前端 `text-input` 的同一入口）→
  同一 persona / 记忆 / mood 触发 / TTS / 口型，不存在「外部消息走简化链路」。
- 端点：`GET /api/external/status`、`POST /api/external/settings`（仅本机）、`GET /api/external/token`（仅本机）、
  `POST /api/external/chat`。支持 `client_uid` 指定窗口、`interrupt` 打断当前发言、`wait_reply` 等回复文本
  （`asyncio.shield`：超时只结束等待、**不取消**正在进行的对话）。
- **安全默认**：默认关闭 → 403；开启后**仅回环可用**；放开局域网必须**同时**勾选 allow_remote 且设置令牌
  （否则 400，拒绝落成「开着门没锁」）；令牌用 `secrets.compare_digest` 比较；单条 ≤2000 字符；
  开关状态落 `external_input.json`（含令牌，已 gitignore）。
- 前端会收到 `{"type":"external-input"}` 回显 → 渲染为带「外部接入」角标的用户气泡 + 顶部提示，
  用户始终知道角色为什么突然开口。

## ③ 前端设置：外部接入可自主开启 / 断开（仅电脑端）
- 设置中心新增「外部接入」分区（`renderer/src/external-input.ts`）：主开关 + **立即断开** +
  允许局域网 + 令牌生成/清除/回显 + **一键复制的 curl 示例** + 在线窗口数 / 接收与拒绝计数 /
  最近 10 条记录（面板打开时 5s 轮询，关闭即停）。
- 开关**即时生效**：不需要「保存全部」，不需要重启后端；左侧状态简报与聊天面板头部胶囊同步显示当前范围。
- 前端与后端共用同一条前置规则（局域网必须先有令牌），避免提交后才报错。

## ④ 移除免费 LLM 模型与内置共享 Key
- **删除 `free_glm_defaults.py`**（旧行为：Key 缺失时回退到**内置共享免费 Key**）→ 新增
  `api_key_resolver.py`：占位符 `${VAR}` 与空 Key 一律拒绝并给出 `.env` / `conf.yaml` 指引；
  本地自托管 Provider（`openai_compatible_llm`/`lmstudio_llm`/`ollama_llm`/`llama_cpp_llm`）允许空 Key。
- **`zhipu_free_llm`（免费 GLM-4.7-Flash 兜底 Provider）整体移除**：`conf.yaml`、`conf.yaml.template`、
  `routes.py`（LLM_DEFAULTS / MODEL_CATALOG）、`config_manager/stateless_llm.py`（字段 + 描述）、
  `stateless_llm_factory.py`、`renderer/src/settings-ui.ts` 标签全部清理；两份 conf 里「内置共享免费 Key」的注释同步改写。
- **不牺牲「用户能自助修好」**：新增 `agent/stateless_llm/misconfigured_llm.py` —— Key 缺失时**不阻断启动**
  （否则用户连设置页都打不开、无法填 Key），而是返回占位 LLM：服务照常启动、设置页可用，
  首次对话抛出带指引的错误并经 `{"type":"error"}` 显示在聊天面板。
- 前端补齐 `ws-bridge` 的 `error` 消息处理（此前后端错误被前端**完全忽略**，用户只看到「角色不说话」）。
- **真机 E2E 抓到并修掉的真实缺陷**：`agent/transformers.py` 的 `emo_interceptor` 把上游异常（未配置 Key /
  401 / 网络不可达）按「P7-05 降级」**只记一条 warning 就吞掉**，前端一个消息都收不到 —— 现在 pump/consume
  仍不崩溃，但会向下游发 `{"type":"stream_error"}`，由 `single_conversation` 转成 `{"type":"error"}` 推给前端；
  新增 `tests/test_stream_error_visibility.py` 锁定该行为（含"message 缺失不发空气泡"）。
- Android 端复核：免费兜底 Provider 早已移除（`LLMProviderManager.kt:141`），仓库内**无硬编码真实 Key**
  （`BuildConfig.*_API_KEY` 来自 gitignore 的 `local.properties` / 环境变量）；残留的 `freeQuotaNoticeDismissed`
  等死标志因**本环境无 Android SDK 无法编译验证**，登记在审计报告中未改。

## ⑤ 角色背景图上传更换（设置页）
- 后端新增 `src/open_llm_vtuber/backgrounds.py`：`GET /api/background/list`、
  `POST /api/background/{upload,select,delete}`；文件存 `backgrounds/`（已挂 `/bg` 静态目录）。
- **安全**：写操作**仅本机**；扩展名白名单（png/jpg/jpeg/webp/gif）+ **文件头魔数校验**（改后缀的可执行文件被拒）；
  单张 ≤12MB；文件名只取 basename 并清洗（`../` 逃逸不可能）；同名自动 `-1/-2` 不覆盖。
- 选择落盘 `background_state.json`；选中项被手工删除后下次启动**自愈**为「无背景」。
- 前端：设置页「形象 / 背景」区缩略图网格（上传即选中 / 点选 / 删除 / 恢复透明），
  背景铺在 PixiJS 画布**之下**（画布 `backgroundAlpha:0`）→ 换背景不影响模型渲染、无需重启；
  有背景图时叠一层暗渐变保证角色与气泡可读；刷新后自动恢复上次选择。
- 新增 `src/open_llm_vtuber/local_state.py`（`JsonState` 原子写），外部接入与背景图共用，去掉两份重复落盘逻辑。

## ⑦ 前端 UI 对标 N.E.K.O + 高级感缺口调研
- 产出 `docs/research/neko-ui-alignment-and-gaps.md`：同类 UI 结构拆解、逐项对比表、
  「看起来廉价」的 8 条具体成因（均落到 `文件:行`）、P0/P1/P2/明确不做四档缺口清单 + 16 项常见缺口逐一核查。
- **本轮落地的 UI 重设计**：
  - **P0-4 统一 design token**：新增 `renderer/src/theme.css`（表面 / 文本 / 强调 / 语义 / 边框 / 字号 6 档 /
    间距 / 圆角 / 阴影 3 阶 / 动效 / 层级）；`settings.css` **零裸色值零裸字号**全量改引用；
    删除聊天面板私有靛蓝 `#6366f1` → 全应用单一强调色；修正「分区标题 12.5px 比正文 13px 还小」的反层级。
  - **样式出栈**：聊天面板 CSS 从 `index.html` 内联（170 行）迁到 `renderer/src/chat.css`，index.html 只留页面骨架。
  - **P1-1 动效体系**：覆盖层淡入 + 弹窗上浮 + tab 交叉淡入 + 消息渐入 + 状态胶囊脉冲 + 聆听波纹，
    统一 `--dur-*` / `--ease-out`，整体遵守 `prefers-reduced-motion`。
  - **P1-2 内容态**：聊天区欢迎空状态（含操作提示）+「白正在思考…」三点指示（发送/注入即亮，首句或 TTS 播放即灭）。
  - **P1-3 图标集**：新增 `renderer/src/icons.ts`（14 个统一线宽内联 SVG），替换 `⚙ / 🎤 / 🔊` 等 emoji。
  - **P1-6 Toast**：新增 `renderer/src/toast.ts`；断线（sticky）/ 重连 / 后端错误 / 外部接入变化 / 重置位姿均可见。
  - **P1-8 无障碍**：设置面板 `role=dialog` + `aria-modal` + **Esc 关闭**；聊天区 `role=log` + `aria-live`；
    按钮补 `aria-label`；统一 `:focus-visible` 焦点环。
  - **P2-2 气泡升级**：头像 + 时间戳 + 毛玻璃气泡 + 外部注入角标。
- **仍未做（诚实登记）**：P0-1 桌面壳（透明置顶 / 点击穿透 / 托盘 / 开机自启 / 全局快捷键，需 Electron/Tauri，L 量级）、
  P0-2 语音与 VAD 打断链路统一、P0-3 聊天历史持久化回看、P0-5 首启引导、P1-4 模型热切换即时生效、
  P1-5 多人设卡、P2-3 i18n。

## 验证
| 项 | 结果 |
| --- | --- |
| PC 后端 pytest | **269 passed**（基线 174 → 新增 95：外部接入 / 背景图 / Key 解析 / 音频缓冲 / 流错误可见性）。`--ignore=tests/test_tts_bargein.py`：该文件硬编码 `.pi` 仓库标记，属**基线既有环境问题**，非本轮引入 |
| PC 前端 vitest | **13 files / 291 passed**（基线 252 → 新增 39：external-input 18 / background 21） |
| PC 前端 tsc | `npx tsc --noEmit` 通过 |
| 前端产物 | `npm run build` 成功，`../frontend/` 产物已更新（含 `libs/live2dcubismcore.min.js` 自动拷回）；产物内含 `外部接入`/`角色背景图`/`--surface-0`/`app-background` |
| 装配级 E2E | 用真实 `server.py` 装配（不加载引擎）跑 20 项：`/health`、`/` 页面、外部接入默认关闭→403、开关落盘、无会话→409、局域网无令牌→400、令牌生成/校验、一键断开、背景 列表/上传/静态 `/bg` 取回/伪装图片拒绝/恢复透明/删除 —— **20/20 通过** |
| **真机 E2E（真实 `run_server.py` 起服务 + 真实 WS 客户端）** | **16/16 通过**：无 Key 时服务正常启动并 `/health` 200；WS 握手；`POST /api/external/chat` → **WS 端确实收到 `external-input` 回显（文本/来源一致）** → **内部对话链路确实被触发**（日志 `New Conversation Chain started` + `User input: 你好，我是外部程序`）→ 无 Key 时以带 `.env`/`conf.yaml` 指引的 `error` 消息结束；断开后立即 403；真实上传背景 + `/bg` 取回 + 删除回透明 |
| 启动韧性 | 无 `.env`（零 Key）时后端**照常启动**（日志给出占位 LLM 警告 + 配置指引），设置页可用 → 用户可自助填 Key |
| 跨端一致性 | `shared/check_cross_platform_consistency.py` **32/33**（唯一失败 C1 = Android `assets/model_registry.json` 产物缺失，Gradle clean 后未重跑构建，非代码问题） |
| 模型注册表 | `shared/validate_registry.py` 全部通过 |
| 运行态卫生 | `external_input.json` / `background_state.json` 均已 gitignore；验证产生的临时状态与上传图片已清理 |
| Android | **未改代码**：本环境无 Android SDK/Gradle 链，Kotlin 改动无法编译回归，故只做静态复核并登记 |

---

# PC 设置中心重构：WSL2 断链修复 + 逻辑链全量重做 + UI 重设计（2026-08-20）

> **范围**：仅 PC 端（open-llm-vtuber Web 设置中心）。权威规格：`docs/plans/settings-redesign-spec.md`；
> 优先依据审计：`docs/plans/settings-fix-plan.md` + `Live2D-Ai-pc/open-llm-vtuber/SETTINGS_SYSTEM_AUDIT.md`。

## WSL2 迁移断链修复
- **`Live2D-Ai-pc/start.sh`**：venv 查找优先级 修复（之前默认 `$HOME/Live2D-Ai/.venv` 在本机不存在 → 启动即退）：
  现在按 `LIVE2DAI_WSL_VENV` → 仓库根 `.venv` → 旧文档路径 三级探测，已验证本机可一键启动。
- **`GET /health`**（新增）：`200 {"status":"ok","version","uptime_s"}`，补上 WSL2 迁移计划 DoD 验收项。
- **端口生效（B5/B6）**：`run_server.py` 不再写死 12393，改读 `config.system_config.port`；`server.py` `/proxy-ws` 同源；
  `_start_server.py` 分叉启动器收敛为复用 `run_server.main`（env `${VAR}` 替换 + 人设注入 + MCP path + 按 port 监听）。
- **文档清理**：README 遗留 Windows 说法（测试/架构图/许可/链接）改为「PC 端（WSL2/Linux）主力」；PC README 环境准备改为仓库根 `.venv`。

## 设置逻辑链（settings-fix-plan 全量推进）
- **P0**：`POST /api/config` 与 `POST /api/config/tree` **先校验后落盘 + 失败回滚**（原子写 tmp+rename）；
  TTS 选中 provider 缺必填(voice/model) 自动补目录默认值，补不出→400 指明字段；设定树写盘前跑 `AppConfig` schema 校验。
- **P1**：POST 成功回传 normalize 后完整 `config`（前端 re-populate，消除 UI=磁盘 漂移）；
  显式清空语义（persona 字段与 `.env` key 传 null → 真删除；区分「未传」与「显式清空」）；
  `log_level` round-trip（从 `.env` 读回，仅用户改动才提交，不再每次保存静默重置 INFO）；
  `.env` 新 key 对运行引擎可见（apply/reload 重读）；`apply=true` 先 load 共享 cache 再遍历已连会话重跑，
  无法覆盖时诚实返回 `requires_restart:true`；check/test/discover 按 provider 协议感知（Anthropic/Ollama 等）+ 取「saved ∪ UI」合成视图。
- **P2/运行层**：引擎/agent `close()` + reload 替换前关旧引擎；`handle_disconnect` 先取 context 再 pop 并 close；
  `init_vision` 支持热换；proxy 播放确认超时补发 `send_conversation_end_signal`（B12）；
  `load_from_config` 顺序修正（system/character_config 在 `init_agent` 前）。

## 设置 UI 重设计（NEKO 内容结构 + Codex/DSH 深色精练风）
- 左侧 6 分区导航（对话/LLM、语音/TTS、形象/Live2D、密钥、人设/提示词、系统/高级）；Modal 覆盖层 + 底部状态条 + 保存全部/保存并应用。
- Provider 卡折叠行→展开编辑；Key 管理中心（明文切换/测试/清除）；形象中心（扫描导入+选中态）；人设编辑器（含清除）；系统区（端口/FPS/log_level + 高级设定树 + 校验错误红框）。
- 行为：打开面板**不再自动 discover**（改手动+缓存）；保存成功 re-populate；busy/disable + 最后一次点击胜出（请求 token/AbortController）；
  `fillSelect`「未选择」占位 + `port:0` 挡下；TTS 检查诚实标注；test 带 saved∪UI；`Audio.play()` 拒绝捕获；`requires_restart`/`field_errors` 文案诚实。
- 结构：纯逻辑抽到 `renderer/src/settings-logic.ts`（可 vitest 直测），`settings-ui.ts` 只做 DOM 装配；样式抽到 `settings.css`（DSH 暗色配色调色板）；`.settings-*` 类名确认无外部引用。
- 构建自愈：`renderer/public/libs/live2dcubismcore.min.js` 补齐（官方 live2dcubismcore@1.0.2），
  Vite 每次构建自动拷回 `frontend/libs/`（修复「重建会把 gitignored 的 Live2D Core 清掉导致模型不渲染」的构建回归）。

## 验证
- 后端 `pytest tests/ -p no:cacheprovider --ignore=tests/test_tts_bargein.py`：**144 passed**
  （注：`test_tts_bargein.py` 硬编码 `.pi` 目录标记无法定位仓库根，为**基线既有环境问题**，非本次引入）。
- 前端 vitest：**11 files / 252 passed**（含新增 `settings-logic.test.ts` 20 例）。
- 实机链路：`start.sh` 一键启动成功；`/health` 200；`/api/config` 200（port/log_level 读回）；
  `POST /api/config` 返回 `{ok,config,applied,requires_restart,message}`；新 bundle + `libs/live2dcubismcore.min.js` + Live2D 模型 均 200。

---

# WSL2 迁移 + 项目更名 Live2D-Ai（2026-08-19）

> **平台定调（用户）：PC 端主力定位 WSL2，**放弃**原生 Windows；项目更名为 `Live2D-Ai`。**
>
> - **仓库迁移**：项目从 Windows G 盘（原 `Live2Dai` 仓库，位于本机旧路径）迁移至 **WSL2 原生盘**（`git clone` 保留全部历史，排除可再生成的构建产物），并新增 `backup` remote 指向原 G 盘仓库。
> - **更名 Live2D-Ai**：目录 `Live2D-Ai-Android` / `Live2D-Ai-pc`；Kotlin 包 `com.live2dai.android` → `com.live2d.ai.android`（含 JNI `Java_com_live2d_ai_android_*`、Gradle、清单）；标识符 `Live2Dai*` → `Live2DAi*`；文档/标题改为 `Live2D-Ai`。模块/产物 slug `live2dai-*` → `live2d-ai-*`。补充清理：文件名 `Live2DaiApp.kt`→`Live2DAiApp.kt`、`Live2DaiPlugin.kt`→`Live2DAiPlugin.kt`。
> - **放弃原生 Windows**：移除 PC 端 Windows 启动入口 `Live2D-Ai-pc/start.ps1` / `启动Live2D-Ai.bat`；移除 Windows SAPI 专属依赖 `pyttsx3`（pyproject + requirements）；PC 端新增 **WSL2 主力启动脚本 `Live2D-Ai-pc/start.sh`**（含 venv + 核心依赖自检）。
> - 保留不变（状态/配置兼容边界）：Android SharedPreferences 键 `live2dai_settings` / `live2dai_ui_mode` / `live2dai_interaction`、记忆库 `live2dai_memory.db`、localStorage 前缀 `live2dai.interaction.`、环境变量 `LIVE2DAI_*`。
> - 文档：`docs/architecture/linux-dev.md`、`docs/plans/wsl2-migration-plan.md`（标记为已执行），PC README/AGENT 改为 WSL2 主力。
> - **WSL2 运行验证（2026-08-20）**：uv 装 Python 3.12.14 + venv，安装后端核心依赖后 `run_server.py` 在 WSL2 启动成功，persona/model 配置加载正常，`/`、`/api/config`、`/api/models`（返回 bai 模型）、`/api/persona` 均 HTTP 200。注：本代码版本无 `/health` 路由；默认 ASR（faster-whisper→torch）与 PC `live2d-models/`、Android `model_registry.json` 为可再生成/需拉取产物，冒烟阶段以 `asr_model: disabled` 验证。

---

# 常规代码审计与管理收尾（2026-08-19）

> 全量代码审计（PC + Android + 一致性）+ 低风险修复 + 仓库收尾（文档归档 / 清理 / 提交）。
> **平台轨迹（同日定调）：PC 端主力迁移 WSL2，放弃 Windows 作为主力（Windows 降为遗留降级）**
> ——`docs/plans/wsl2-migration-plan.md` 已据此更新，审计遗留项按 Linux/WSL2 优先级重排。

## 审计基线（本次实跑）
- PC renderer：`npx tsc --noEmit` 通过；Vitest 232/232 通过。
- PC 后端：`python3 -m py_compile routes.py` 通过（完整 pytest 因环境缺依赖未跑）。
- 一致性：`shared/check_cross_platform_consistency.py` 33/33 通过。
- Android：本环境无 Android SDK/AGP 编译链，改动为静态实现 + 静态审查，**需在 Windows/CI 用 gradlew 编译/单测回归确认**（与既有 handoff 一致）。

## 低风险修复（本会话落地）
### Android
- `HttpLlmLink.rebuild()` 同步重建情绪分类桥（`HttpEmotionClassifier` 新 baseUrl/apiKey）——
  修复「主请求用新 Provider、情绪分类仍打旧端点」的错配/降级（审计 M-1）。
- `HttpLlmLink.buildRequest()` 单次快照 `config`，payload 与 URL 取自同一快照（endpoint 传入 cfg.baseUrl），
  消除 rebuild 与 in-flight send 并发 torn-config（审计 M-3）。
- `VoiceIoController.refreshProviders()` 换链前先 `stopSpeaking()` 停掉旧 Provider 在播音频，
  修复双音频 / `_isSpeaking` 失步（审计 A-1，High）。
- `SpeechSinkAdapter.speak()` 的 `spoke` 改 `AtomicBoolean`，修复跨线程内存可见性导致的误判失败信号（审计 A-4）。
- `LoopCoordinator.llmLink` 标 `@Volatile`，修复 replaceLlm 跨线程可见性（审计 A-5）。
### PC
- `update_config_tree`（原生 YAML 直写路径）改原子写：同目录临时文件 + `os.replace`（`_write_raw_atomic`），
  与 `_save_yaml` 一致，避免写盘失败留下半份 conf.yaml 导致下次启动失败（审计 P-1）。

## 遗留（报告见 `docs/audit/*`，建议后续处理）
- PC 高危：`.env` / `persona.yaml` 写仍非原子（含密钥，崩溃会一次性毁掉所有 Key）；`discover-models` / `test-llm`
  会把存储的 Key 发给客户端任意 `base_url`（SSRF/泄漏）；`allow_origins:*` + 0.0.0.0 无鉴权。
- PC 中危：`log_level` 无服务端枚举校验；"保存并应用"对 log_level 谎报 `applied:True`（运行时 level 不变）；
  `system_config.port` 是死配置（监听仍硬编码 12393）；`_save_yaml` 权限（mkstemp 0600→replace）与目录 fsync 未处理。
- Android 中危：`resetAll()` 未复位内存态（当前未接线，latent）；非 HttpLlmLink 兜底每次重建 OkHttpClient（测试态）。
- Android 未编译，务必在 CI 回归 `rebuild/refreshProviders/replaceLlm/sendMessage("")` 新增用例。

---

# 设置链路修复落地（2026-08-19）

> PC 与 Android「设置写了但运行没生效」的调用链断链修复。

## PC（open-llm-vtuber）
- conf.yaml 原子保存（临时文件 + `os.replace`），写盘失败不再留下半份配置。
- 设定树（`/api/config/tree`）写盘前做 `AppConfig` schema 校验，结构非法返回 400 且不落盘。
- `GET /api/config` 回传 `log_level`，"保存全部"不再把 `.env` 日志级别静默重置为 INFO。
- apply 热重载前重载 `.env`，新写入的 `LIVE2DAI_*` Key 对运行引擎可见。
- 此前提交 d75dd96 已含：先校验后落盘、Provider 别名/旧 id 迁移、测试走当前 UI 值、TTS/LLM 选中自动补默认。

## Android
- `HttpLlmLink` 改为可重建（`rebuild(LlmEndpointConfig)`）：改 Provider/base_url/api_key/聊天模型/温度后下一次请求即生效，不再需重启。
- `LoopCoordinator` 新增 `replaceLlm` 运行时替换 LLM 链接；空文本发送前拦截，不再卡死 SENDING。
- `VoiceIoController` 新增 `refreshProviders()`：设置修改后重读 TTS Provider 注册表（新 Key/新引擎运行时可见）；`SpeechSinkAdapter` 透传首选 `ttsProviderId`，引擎选择真正生效。
- `resetAll()` 同时清空 `llm_provider` 与 `live2dai_ui_mode` 独立偏好；`LoopStateMachine.state` 标 `@Volatile`。
- 设置页保存后经 `onRuntimeSettingsChanged` 通知 MainActivity 应用运行时配置。

## 验证
- PC：`py_compile`、renderer `tsc --noEmit`、Vitest 232/232 通过（实现时已跑）。
- Android：因环境无 JDK 未能编译，需在 Windows/CI Gradle 构建后以 JVM 测试回归确认（见 handoff.json 备注）。

---

# 地基整理（2026-08-16）

> 不改变 v0.1 功能边界，只做减法与工程化收口。

- 删除废弃：根 `tts/` 独立模块、Android 无引用 `assets/model_dict.json`。
- 目录与文档：新增 `docs/README.md`、`docs/architecture/core-contracts.md`、`docs/architecture/directory.md`、`docs/architecture/observability.md`、`docs/architecture/dependencies.md`、`docs/architecture/plugin-sdk.md`。
- 共享同步：Android Gradle 新增 `copyMcpTools`，`scripts/sync.ps1` 修正 PC 路径并移除已废弃 model_dict 同步。
- 一致性：`shared/check_cross_platform_consistency.py` 适配 Wave 5 后的当前代码（EmotionController 已删），新增 SM-03 mcp_tools 字节一致检查；当前 33/33 通过。
- 插件骨架：恢复 `Live2D-Ai-Android/plugin-sdk/` 最小接口模块（ChatHook / PluginHost / ToolDefinition / Live2DAiPlugin）。
- 无 Key 自检：新增 `scripts/health_check.py`、`scripts/verify_all.py`、`scripts/clean.sh`、`scripts/clean.ps1`、`tests/test_foundation_no_key.py`。
- 修复根 pytest 中 Wave5 后的陈旧断言：EmotionController/ProactiveGreetingClient 已删的用例改为当前实现校验，一致性汇总从 32/32 更新为 33/33，`cache/` 不再判为失败（运行时目录，由 clean 脚本清理）。
- PC 延迟性能断言从 5ms 放宽到 50ms（Windows/CI 计时器与事件循环调度会偶发 10-20ms 尖峰；保留防明显性能回归的作用）。
- 打包：修复 `check_resources.py` / `verify_package.py` 中已删除的 niziiro_mao 哨兵路径为 bai；新增 `scripts/finish_pc_package.py` 用于在已有 staging 上续跑 Windows 打包。
- Linux：新增 `scripts/setup_linux.sh` 与 `docs/architecture/linux-dev.md`，定位为开发/测试环境，不作为发布平台。
- P0-P3 第一批落地：Android 语音输入（系统 SpeechRecognizer + 麦克风 UI + 权限）、PC 启用 Faster-Whisper 与浏览器语音输入 UI、Android TTS 播放状态/引擎显示、官方 C++ ENGINE 模型加载接线（保留 LEGACY 回退）。
- P0-P3 细化：Android 设置页新增“语音输入开关 + 识别引擎选择（系统/SenseVoice）”；PC 前端新增 TTS 播放状态栏；新增 SenseVoice 离线 ASR 模型下载项；新增 `scripts/smoke_check.py` 与 `docs/verification/smoke-checklist.md`。
- 多厂商支持链路：Android LLM Provider 新增 通义千问/智谱/OpenAI 兼容；Android 新增 Qwen TTS Provider；PC 新增 Qwen LLM/TTS 配置与 Provider；`shared/model_dict.json` 增加 Qwen 模型。

---

# v0.1.0 版本说明（2026-08-15）

> 定位：Live2D + AI + TTS 最小验证项目的第一个可用版本。Android 为主端，PC（Open-LLM-VTuber）为桌面端。

## 本版本核心能力

**Android 端（核心环路，全部真机验证）**：
- TTS + AI + Live2D 闭环：文本输入 → DeepSeek LLM → 回复 → CosyVoice 云端 TTS（主用，minimp3 解码）/ sherpa-onnx 离线兜底 → 嘴型同步（RMS 幅度驱动）→ Live2D 渲染
- 情绪联动（N.E.K.O. 架构）：独立 LLM 情绪分类器 → 单一标签 → 参数帧表 → 表情 + 5 秒自动收敛（冻结免疫）
- 渲染：官方 CubismNativeFramework + PurismCore（MIT）替换自研渲染器；三档画质设置（流畅 1080p / 均衡 MSAA / 高清 2K 超采样）
- 结构免疫三大历史 bug：双播报（单消费链状态机）、对话后冻结（表情收敛）、嘴型无帧（PCM 直出/minimp3）
- 待机间歇轻晃（头部 ±5°、动 5s 停 10s）、FPS 角标、眨眼正常

**PC 端**：
- Open-LLM-VTuber + CosyVoice 云（同 Key/专属服务）+ 人设联动（shared/persona.yaml）
- 修复打断崩溃（PC-L1）+ mood_tier 情绪分层接通（PC-L4）

**质量**：JVM 663/663 绿、PC pytest 150/150 绿、真机验收 9/10 项 PASS（帧率项受 vivo 智能刷新策略影响，需手机设固定 120Hz）

## 已知限制（v0.1）
1. 实际呈现帧率 ~50fps（渲染循环 120fps）——官方 C++ 渲染器模型加载未接线（"最后一公里"），v0.2 计划
2. sherpa 音色默认 zf_001（100 中文音色选择器 v0.2 计划）
3. 声音克隆未做（CosyVoice 条件已具备）
4. 手机屏幕刷新率需手动设"高"（vivo 智能切换会降 60Hz）

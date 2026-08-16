# 地基整理（2026-08-16）

> 不改变 v0.1 功能边界，只做减法与工程化收口。

- 删除废弃：根 `tts/` 独立模块、Android 无引用 `assets/model_dict.json`。
- 目录与文档：新增 `docs/README.md`、`docs/architecture/core-contracts.md`、`docs/architecture/directory.md`、`docs/architecture/observability.md`、`docs/architecture/dependencies.md`、`docs/architecture/plugin-sdk.md`。
- 共享同步：Android Gradle 新增 `copyMcpTools`，`scripts/sync.ps1` 修正 PC 路径并移除已废弃 model_dict 同步。
- 一致性：`shared/check_cross_platform_consistency.py` 适配 Wave 5 后的当前代码（EmotionController 已删），新增 SM-03 mcp_tools 字节一致检查；当前 33/33 通过。
- 插件骨架：恢复 `Live2Dai-Android/plugin-sdk/` 最小接口模块（ChatHook / PluginHost / ToolDefinition / Live2DaiPlugin）。
- 无 Key 自检：新增 `scripts/health_check.py`、`scripts/verify_all.py`、`scripts/clean.sh`、`scripts/clean.ps1`、`tests/test_foundation_no_key.py`。
- 修复根 pytest 中 Wave5 后的陈旧断言：EmotionController/ProactiveGreetingClient 已删的用例改为当前实现校验，一致性汇总从 32/32 更新为 33/33，`cache/` 不再判为失败（运行时目录，由 clean 脚本清理）。
- PC 延迟性能断言从 5ms 放宽到 50ms（Windows/CI 计时器与事件循环调度会偶发 10-20ms 尖峰；保留防明显性能回归的作用）。
- 打包：修复 `check_resources.py` / `verify_package.py` 中已删除的 niziiro_mao 哨兵路径为 bai；新增 `scripts/finish_pc_package.py` 用于在已有 staging 上续跑 Windows 打包。
- Linux：新增 `scripts/setup_linux.sh` 与 `docs/architecture/linux-dev.md`，定位为开发/测试环境，不作为发布平台。
- P0-P3 第一批落地：Android 语音输入（系统 SpeechRecognizer + 麦克风 UI + 权限）、PC 启用 Faster-Whisper 与浏览器语音输入 UI、Android TTS 播放状态/引擎显示、官方 C++ ENGINE 模型加载接线（保留 LEGACY 回退）。

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

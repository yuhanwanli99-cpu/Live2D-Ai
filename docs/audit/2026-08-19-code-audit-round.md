# Live2D-Ai — 代码审计与管理收尾（2026-08-19）

> 本轮 = 「常规代码审计和管理」：全量审查 PC（Python/TS）+ Android（Kotlin）+ 跨端一致性，
> 落地低风险修复，并完成仓库收尾（文档归档 / 清理 / 逻辑提交）。
> 相关分项报告：`docs/audit/CONFIG_SCHEMA_AUDIT.md`、`docs/audit/SETTINGS_SYSTEM_AUDIT.md`、
> `docs/audit/tts_config_audit_report.md`。

> **平台轨迹（2026-08-19 用户定调，见 `docs/plans/wsl2-migration-plan.md`）：**
> **PC 端主力迁移到 WSL2，放弃 Windows 作为主力（Windows 降为遗留降级）。**
> 本报告遗留项的**平台优先级**按此重排：Linux/WSL2 相关项（原子写的权限/目录 fsync、`.env`/`persona.yaml`
> 原子化）**升高**；Windows 专属项（`start.ps1`、`pyttsx3`(SAPI)、`_start_server.py` 分叉启动器）**降为遗留**，
> 仅在「不删除、不主动维护」层面处理。

## 1. 审计基线（本次实际执行）

| 项目 | 命令 / 结果 |
| --- | --- |
| PC renderer 类型 | `npx tsc --noEmit` 通过 |
| PC renderer 单测 | Vitest **232/232** 通过（含 ws-bridge/idle/skeleton/bai-layout 等 10 文件） |
| PC 后端语法 | `python3 -m py_compile routes.py` 通过（完整 pytest 因环境缺 Python 依赖未跑） |
| 跨端一致性 | `shared/check_cross_platform_consistency.py` **33/33** 通过（C1-C32 + SM-03） |
| Android | 本环境无 Android SDK/AGP 编译链 → 静态审查 + 静态实现，**未编译**，需 Windows/CI 回归 |
| 密钥 | 仓库无泄露：`.env`、`local.properties` 均 gitignore 且未跟踪；未发现硬编码真实 Key 入库 |

## 2. 已落地修复（本会话低风险项）

### Android（`app/src/main/java/com/live2d/ai/android/`）
1. **M-1（重启情绪分类桥）** `core/HttpLlmLink.kt#rebuild()`：`rewrite` 后同步重建
   `EmotionClassifierBridge(HttpEmotionClassifier(newBaseUrl, newApiKey, client))`，
   消除「主请求用新 Provider、情绪分类仍打旧端点/旧 Key」的错配与静默降级。
2. **M-3（torn-config）** `HttpLlmLink.buildRequest()`：`endpoint(cfg.baseUrl)` 传入已快照 baseUrl，
   保证同一次请求的 payload 与 URL 取自同一 `config` 快照，修复 rebuild 与 in-flight send 并发竞态窗口。
3. **A-1（refreshProviders 在播处理，High）** `VoiceIoController.refreshProviders()`：替换列表前先
   `stopSpeaking()` 停掉旧 Provider 在播音频，避免双音频与 `_isSpeaking` 失步（伪 SpeakFinished）。
4. **A-4（spoke 可见性）** `core/SpeechSinkAdapter.kt#speak()`：`spoke` 由裸 `var` 改 `AtomicBoolean`，
   修复后台 observer 线程写 / 主线程读之间的内存可见性，消除误判失败信号。
5. **A-5（llmLink 可见性）** `core/LoopCoordinator.kt`：`llmLink` 标 `@Volatile`，与
   `config`/`state`/`boundHost` 风格一致，保证 `replaceLlm` 后「下一次请求用新链接」跨线程成立。

### PC（`Live2D-Ai-pc/open-llm-vtuber/src/open_llm_vtuber/routes.py`）
6. **P-1（树写原子化）** `update_config_tree` 直写路径改用 `_write_raw_atomic()`（临时文件 + `os.replace`），
   与 `_save_yaml` 一致性——避免合法 YAML 校验通过后写盘失败留下半份 conf.yaml 导致下次启动失败。

## 3. 遗留项（建议后续，未在本轮落地以防超范围/需真实构建回归）

### PC 高危
- `.env`（`routes.py` ~L641）与 `persona.yaml`（~L267）写仍非原子；`.env` 含 LLM/TTS 密钥，
  崩溃中途写盘会一次性毁掉全部 Key。→ 复用同一原子写工具。
- `discover-models`（~L803）与 `test-llm`（~L1001）会把**存储的 Key** 作为 `Bearer` 发给客户端
  提供的任意 `base_url` → SSRF / 密钥外泄；叠加 `0.0.0.0` 绑定 + `allow_origins=["*"]` + 无鉴权。
  → Key 只对配置化/白名单 URL 下发；收紧 CORS / 加轻量鉴权。
- `log_level` 无服务端枚举校验（任意字符串进 `.env` 再喂 `loguru`）；且「保存并应用」对 log_level
  返回 `applied:True` 但运行时 level 并未重配（`run_server.py` 启动即冻结）。

### PC 中危
- `system_config.port` 是死配置：UI 写、读回，但 `run_server.py` 仍硬编码 `12393`。
- `_save_yaml`/`_write_raw_atomic` 用 `mkstemp`（0600）`os.replace` 不保留原文件权限；未做目录 fsync（Windows 影响小，verify）。
- `update_config` 与 `update_config_tree` 校验逻辑重复，易漂移。
- 自定义 `_env_value` 解析器 vs 已依赖的 `dotenv` 重复且语义不全。

### Android 中危
- `SettingsRepository.resetAll()` 清 prefs 但未复位内存 `RenderQualityConfig.current`（当前未接线，latent）。
- `MainActivity.applyRuntimeSettings()` 非 `HttpLlmLink` 兜底分支每次新建 OkHttpClient（仅测试态）。
- `local.properties` 含真实 Key —— 已 gitignore，但属开发机密，勿提交。

### 其它
- Android 新运行时传播面（`rebuild`/`LlmEndpointConfig`/`replaceLlm`/`refreshProviders`/
  `onRuntimeSettingsChanged`/`sendMessage("")`）当前无对应单元测试 → 建议在 CI 构建后补并回归。

## 4. 仓库管理收尾（本会话完成）

- 新增 `docs/audit/`：将散落在 `Live2D-Ai-pc/open-llm-vtuber/` 的 3 份审计报告归档至此并提交。
- 新增 `.gitignore` 规则（一次性交接/审计工作产物不入库）：
  `**/handoff*.json`、`**/SETTINGS_*_HANDOFF.json`、`analysis/tmp/`、`analysis/**/tmp/`。
- 一次性 `handoff*.json` / `analysis/tmp/*` 交易产物现被忽略，不再污染 `git status`。
- 提交规划：代码改动 + 文档/报告 + `.gitignore` 分逻辑提交（见 git log）。

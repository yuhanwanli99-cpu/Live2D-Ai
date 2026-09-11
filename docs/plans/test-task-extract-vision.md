# 测试报告 — task-extract-vision（视觉插件提取）

> 测试日期: 2026-07-31
> 测试范围: `live2d-ai-plugin-vision` 新模块 + core 侧接入（VisionService.kt 移除 / MainActivity / build 配置）
> 设计依据: `docs/plans/plan-task-extract-vision.md`（§1.3 验收标准、§4/§5 接口定义、§6 文件清单）
> 环境: WSL（`/mnt/f/live2dai/Live2D-Ai-Android`），JDK 17，Android SDK `/opt/android-sdk`，Gradle 8.x

---

## 1. 测试概况

| 类别 | 用例数 | 通过 | 失败 |
| ------ | ------- | ------ | ------ |
| 构建验证（vision 插件 assemble / app assembleDebug / plugin-sdk build） | 3 | 3 | 0 |
| 单元测试（vision 插件: GlmVisionClientTest 5 + VisionPluginTest 9） | 14 | 14 | 0 |
| 单元测试（app 回归: ChatServiceTest 7 + EdgeTts 10 + Emotion 14 + Persona 6） | 37 | 37 | 0 |
| 集成验证（tester 补充: 真实 PluginManager→VisionPlugin→MockWebServer 全链路） | 2 | 2 | 0 |
| LSP 静态诊断（8 个改动文件） | 8 | 8 | 0 |
| **合计** | **64** | **64** | **0** |

**结论: PASS**（6 维验证全部通过，无 P0/P1 问题）

---

## 2. 详细结果

### 维度 1: 功能正确性 — ✅ PASS

| # | 验证点 | 结果 | 证据 |
| --- | -------- | ------ | ------ |
| 1.1 | VisionPlugin 实现 Live2DAiPlugin（id/version/onLoad/onUnload） | PASS | 源码审查：`VisionPlugin.kt` 完整实现 4 个成员；`onUnload` 不抛异常（测试 `onUnload_doesNotThrow`） |
| 1.2 | onLoad 注册 1 个 ChatHook + 1 个 ToolDefinition | PASS | `VisionPluginTest.onLoad_registersOneHookAndCaptureScreenTool`（FakeHost 断言 hooks=1, tools=1, name="capture_screen"） |
| 1.3 | GlmVisionClient 正确调用 GLM API | PASS | 请求体含 model/messages(image_url base64)/stream=false/max_tokens=500；`Authorization: Bearer $apiKey`；路径 `/chat/completions`（`describeImage_sendsAuthorizationHeaderAndModel`） |
| 1.4 | hook 注入格式符合设计 §4.4 | PASS | `beforeSend_injectsScreenDescription` 断言精确等于 `"[系统: 当前屏幕内容]\n屏幕截图描述: 屏幕上显示聊天界面"` |
| 1.5 | tool 返回 GLM 描述 | PASS | `captureScreenTool_returnsDescription` |
| 1.6 | 全链路（真实 PluginManager→VisionPlugin→ChatService.hooks→GLM） | PASS | tester 补充集成测试 2/2：`fullChain_loadPlugins_registersHookAndInjectsDescription` / `fullChain_blankKey_registersButInjectsNothing`（验证后已删除临时测试文件） |

### 维度 2: 代码质量 — ✅ PASS（2 条 P2 建议）

| # | 验证点 | 结果 | 证据 |
| --- | -------- | ------ | ------ |
| 2.1 | 与 plugin-sdk 接口签名一致 | PASS | `Live2DAiPlugin`/`ChatHook`/`ToolDefinition`/`PluginHost` 4 个接口逐一对照，VisionPlugin/VisionDescribeHook/ScreenCaptureTool 完全匹配 |
| 2.2 | 无冗余/反模式 | PASS（附建议） | 无死代码、无重复逻辑；迁移逻辑与旧 VisionService 逐行对比一致（仅参数化 + 空 key 早退）。P2-1: `VisionPlugin.onLoad` 创建 2 个 GlmVisionClient（各持 1 个 OkHttpClient 连接池）——符合设计 §5.1，但可共享 1 个 client。P2-2: 异常仅 `printStackTrace()` 后返回空串，无日志分级 |
| 2.3 | 插件不持有 Android Context / 截图能力 | PASS | `ScreenshotProvider` 接口注入（设计 §2.3）；`AndroidScreenshotProvider` 空壳在 core 侧 |
| 2.4 | LSP 静态诊断 | PASS | `lsp_diagnostics` 对 8 个改动文件: **0 诊断** |

### 维度 3: 边界情况 — ✅ PASS

| # | 场景 | 结果 | 证据 |
| --- | ------ | ------ | ------ |
| 3.1 | API Key 为空 | PASS | hook 返回 null 且**不触发截图**（`beforeSend_returnsNullWithoutCapturingWhenApiKeyBlank` 断言 captureCalled=false）；tool 返回 `{"error": "未配置 Zhipu API Key"}`；client 不发请求（`describeImage_returnsEmptyOnBlankApiKey` 断言 requestCount=0） |
| 3.2 | 截图失败（capture()=null） | PASS | hook 返回 null（`beforeSend_returnsNullWhenScreenshotUnavailable`）；tool 返回 `{"error": "无法截取屏幕"}`（`captureScreenTool_returnsErrorWhenScreenshotUnavailable`） |
| 3.3 | GLM API 返回 HTTP 错误 | PASS | `describeImage_returnsEmptyOnHttpError`（401 → 空串 → hook null → 对话不受影响） |
| 3.4 | 响应缺失 choices/message | PASS | `?: ""` 兜底（源码审查） |
| 3.5 | 非法 tool 参数 JSON | PASS | 不崩溃，回退默认提示词（`captureScreenTool_toleratesInvalidArgsJson`） |
| 3.6 | 兼容 message 与 delta 两种响应格式 | PASS | `describeImage_handlesStreamingDeltaFormat` + `describeImage_returnsContentFromMessage` |

### 维度 4: 安全性 — ✅ PASS

| # | 验证点 | 结果 | 证据 |
| --- | -------- | ------ | ------ |
| 4.1 | API Key 未硬编码在插件 | PASS | 插件内无任何硬编码密钥（grep 扫描 `api[_-]?key | secret | token` 16+ 位字符串：插件源码 0 命中）；key 经 `VisionPluginConfig.apiKey` 构造注入 |
| 4.2 | 配置源留在 core | PASS | `BuildConfig.ZHIPU_API_KEY`（app/build.gradle.kts:70 + `buildConfig=true`）、`SettingsRepository.zhipuApiKey`（SharedPreferences→BuildConfig 回退）、`PersonaConfig.getVisionModel()`（默认 "glm-4.6v"）均未迁移 |
| 4.3 | local.properties 密钥不进 git | PASS | `git check-ignore local.properties` → 已忽略 |
| 4.4 | 插件清单最小权限 | PASS | AndroidManifest 为空（无 Activity/Service/权限声明） |
| 4.5 | 截图数据泄露风险 | PASS（设计内已知） | 截图经 Base64 发送至智谱云端 API（GLM-4V 云模型所必需），设计 §8 R5 已文档化为 TODO（本地 OCR 预过滤/用户可关闭）。当前 `AndroidScreenshotProvider.capture()` 返回 null，实际不产生任何截图数据流。异常路径无截图内容落日志 |

### 维度 5: 兼容性 — ✅ PASS

| # | 验证点 | 结果 | 证据 |
| --- | -------- | ------ | ------ |
| 5.1 | core 移除 VisionService.kt 后无残留引用 | PASS | `git grep -n "VisionService" HEAD -- '*.kt'`（HEAD 基线）= 0 代码引用；工作树 grep = 仅 3 处注释提及（build.gradle.kts 注释、插件 KDoc） |
| 5.2 | app/build.gradle.kts 依赖正确 | PASS | `implementation(project(":plugin-sdk"))` + `implementation(project(":live2d-ai-plugin-vision"))`；settings.gradle.kts 含两个 include |
| 5.3 | plugin-sdk 纯 JVM 模块被 Android Library compileOnly 引用（设计 §8 R3） | PASS | plugin-sdk/build.gradle.kts 为 `org.jetbrains.kotlin.jvm`（无 Android）；`:plugin-sdk:build` BUILD SUCCESSFUL |
| 5.4 | 无插件时行为不变（向后兼容） | PASS | `ChatService.hooks` 默认 `emptyList()`；`ChatService(apiKey, baseUrl)` 兼容次构造保留；app 37 测试全过无回归 |
| 5.5 | ChatService hook 隔离（单插件异常不打断对话） | PASS | 逐 hook try/catch 静默跳过（源码审查） |
| 5.6 | PluginManager.hooks 活引用（晚注册可见） | PASS | `ChatService(hooks = pluginManager.hooks)` 持有 CopyOnWriteArrayList 引用；集成测试证明 LaunchedEffect 装载后 hook 可见 |

### 维度 6: LLM-as-Judge（目标达成度）— ✅ PASS

| 设计 §1.3 验收标准 | 判定 | 证据 |
| --------------------- | ------ | ------ |
| ① Core 移除 VisionService.kt 后编译通过，无插件行为不变 | PASS | `:app:assembleDebug` BUILD SUCCESSFUL；37 测试通过 |
| ② app 添加插件依赖后 loadPlugins 装载 → 截图描述自动注入 | PASS | `:app:assembleDebug` 通过；集成测试验证注入格式正确 |
| ③ 插件独立仓库可独立编译 | PASS | `:live2d-ai-plugin-vision:assemble`（debug+release）BUILD SUCCESSFUL |
| ④ BuildConfig.ZHIPU_API_KEY / SettingsRepository.zhipuApiKey 留在 core | PASS | 见维度 4.2 |
| 「极简 core + 插件扩展」架构达成 | PASS | VisionService（37 行 core 类）→ 独立模块 5 个文件，core 仅剩 PluginManager + 空壳 ScreenshotProvider + MainActivity 3 行装配代码 |

---

## 3. 问题清单

| 级别 | 问题 | 判定 |
| ------ | ------ | ------ |
| P0 | 无 | — |
| P1 | 无 | — |
| P2-1 | `VisionPlugin.onLoad` 创建 2 个 GlmVisionClient → 2 个 OkHttpClient 连接池（每轮对话 hook+tool 各持有连接、线程）。功能正确，但建议共享单个 client 实例（后续任务可优化，符合设计 §5.1 原样） | 不阻塞 |
| P2-2 | `GlmVisionClient` 异常处理仅 `printStackTrace()` + 返回空串，无日志级别/可观测性；API 调用失败原因对用户不可见 | 不阻塞 |
| P2-3 | app 侧 ChatServiceTest 未覆盖 hooks 注入路径（hook 行为测试均在插件模块内，集成路径由本次 tester 临时验证覆盖）。建议后续在 app 模块补一条 ChatService+hook 集成测试 | 不阻塞 |

**测试环境备注（非产品缺陷）**: WSL 仅 3.8GB 内存，默认 Gradle daemon（-Xmx2048m）+ CMake 原生编译会触发 daemon OOM 崩溃；改用 `GRADLE_OPTS="-Xmx1536m" --no-daemon` 后全部构建稳定通过。另曾出现一次 `bundleLibRuntimeToDirDebug` 状态目录读取失败（9p 文件系统增量缓存竞态），重跑即恢复。

---

## 4. 改进建议

1. **共享 OkHttpClient**：`VisionPlugin.onLoad` 中创建 1 个 GlmVisionClient 同时注入 hook 与 tool，并在 `onUnload()` 中 `client.connectionPool.evictAll()`（或关闭 client），减少连接/线程开销。
2. **可观测性**：GLM 调用失败时区分「HTTP 错误码」「网络异常」「响应解析失败」，必要时通过 PluginHost 上报或写入日志，便于排障。
3. **节流策略**（设计 §8 R4）：每轮对话都截图+调 GLM 会产生费用与延迟，建议后续加「N 轮一次 / 屏幕变化检测」节流。
4. **隐私预过滤**（设计 §8 R5）：截图上传云端前可加敏感信息检测或用户可关闭开关。
5. **app 模块补 hook 集成测试**：将本次 tester 验证的全链路场景（真实 PluginManager + VisionPlugin + MockWebServer）固化为 app 模块的正式测试，防止后续重构回归。

---

## 5. 教训候选

- [工具] 教训：WSL（3.8GB RAM）上跑 Android 构建，默认 Gradle daemon 堆 2GB 会与 CMake/Kotlin 编译争内存导致 daemon 崩溃，误判为构建失败。标准：Windows 磁盘上的 Android 工程在内存受限 WSL 中构建，若 daemon 无故消失或 OOM，先 `GRADLE_OPTS="-Xmx1536m" --no-daemon` 重试再下结论。建议：把内存受限环境的 Gradle 内存参数与「daemon 崩溃≠代码错误」的排查路径写入项目测试备忘。
- [测试] 教训：MockWebServer 单响应队列 + 链路内多次 HTTP 调用（hook 一次 + tool 一次）会导致第二次请求超时，误报产品缺陷。标准：凡是被测链路会发起多次 HTTP 请求，必须按调用次数逐一 enqueue 响应。建议：编写集成测试时先数清被测路径的请求次数。

---

## 附: 本次实际执行的关键验证命令（证据）

```bash
# 1. 插件独立构建 + 单元测试（14 用例）
cd /mnt/f/live2dai/Live2D-Ai-Android
./gradlew :live2d-ai-plugin-vision:assemble :live2d-ai-plugin-vision:testDebugUnitTest
# → BUILD SUCCESSFUL in 2m 1s；GlmVisionClientTest tests=5 failures=0 errors=0；VisionPluginTest tests=9 failures=0 errors=0

# 2. app 回归（37 用例）
./gradlew :app:testDebugUnitTest
# → BUILD SUCCESSFUL；ChatServiceTest 7 + EdgeTtsServiceTest 10 + EmotionControllerTest 14 + PersonaConfigTest 6，均 failures=0 errors=0

# 3. core 全量编译（已无 VisionService.kt）
./gradlew :app:assembleDebug
# → BUILD SUCCESSFUL in 1m 22s

# 4. plugin-sdk 独立构建（纯 JVM，R3 兼容）
./gradlew :plugin-sdk:build
# → BUILD SUCCESSFUL in 31s

# 5. tester 补充集成测试（真实 PluginManager→VisionPlugin→MockWebServer 全链路）
./gradlew :app:testDebugUnitTest --tests "com.live2d.ai.android.VisionPluginIntegrationTempTest"
# → tests=2 failures=0 errors=0（验证后临时文件已删除，git status 无残留）

# 6. 残留引用检查
git grep -n "VisionService" HEAD -- '*.kt'   # → 0 代码引用（仅定义文件自身）
grep -rn "VisionService" --include="*.kt" --include="*.kts" . # → 仅注释提及

# 7. 密钥硬编码扫描
grep -rniE "(api[_-]?key|secret|token|password)\s*=\s*[\"'][A-Za-z0-9._-]{16,}" live2d-ai-plugin-vision/src/  # → 0 命中

# 8. LSP 静态诊断（8 个改动文件）
lsp_diagnostics → Files checked: 8, Total diagnostics: 0
```

# plan-verify-e2e — Android 真机端到端验证计划

> 版本: 1.0.0  
> 日期: 2026-07-22  
> 目标: 构建 APK → 部署到真机 (192.168.0.102:5555) → 验证四大功能  
> 前置条件: 项目目录为 `Live2D-Ai-Android`（已从旧目录名完成重命名）

---

## 1. 系统状态扫描结果

### 1.1 构建链完整性检查

| 检查项 | 状态 | 详情 |
| -------- | ------ | ------ |
| `gradlew` / gradle wrapper | ❌ **MISSING** | 整个 `gradle/` 目录不存在；项目不可构建 |
| Android SDK | ✅ 就绪 | `/opt/android-sdk` (在 local.properties 中正确引用) |
| NDK 27.0.12077973 | ✅ 就绪 | `/opt/android-sdk/ndk/27.0.12077973` |
| Java 17 | ✅ 就绪 | `OpenJDK 17.0.19` |
| Gradle 缓存 | ✅ 可用 | `~/.gradle/wrapper/dists/` 含 gradle-8.10/8.12/8.13 |
| `CMakeLists.txt` | ✅ 正确 | 引用全部 19 个 Purism Core .c 文件 |
| `build.gradle.kts` | ✅ 正确 | AGP 8.7.3, NDK 27, Kotlin 2.1.0, CMake 3.22.1 |
| `local.properties` | ✅ 正确 | SDK/NDK 路径 + API Keys |

### 1.2 源码文件完整性

| 文件 | 行数 | 状态 |
| ------ | ------ | ------ |
| `Live2DRenderer.kt` | ~1095 | ✅ 主渲染管线 (Purism Core + CubismJavaFramework) |
| `AnimationSystem.kt` | ~1206 | ✅ 动画系统 (Motion/Pose/EyeBlink/Breath/Physics) |
| `PurismClippingManager.kt` | ~700 | ✅ FBO 裁剪管理器 |
| `PurismModel.kt` | ~400 | ✅ 顶点数据处理 |
| `Live2DNative.kt` | ~200 | ✅ JNI 声明接口 |
| `native-lib.cpp` | ~679 | ✅ JNI 桥接实现 |
| `Live2DView.kt` | ~150 | ✅ GLSurfaceView + Compose 封装 |
| `ChatService.kt` | ~195 | ✅ DeepSeek SSE 流式调用 |
| `EdgeTtsService.kt` | ~350 | ✅ Edge TTS + 系统 TTS 降级链 |
| `VoiceInputService.kt` | ~78 | ✅ Android SpeechRecognizer |
| `VisionService.kt` | 存在 | ✅ GLM-4V 截图分析 |
| `EmotionController.kt` | 存在 | ✅ 情感标签解析 |
| `ShaderManager.kt` | 存在 | ✅ 着色器加载/编译 |
| `PersonaConfig.kt` | 存在 | ✅ YAML 人设解析 |
| `ApiModels.kt` | 存在 | ✅ API 数据模型 |
| `MainActivity.kt` | ~413 | ✅ Compose UI + 生命周期 |

### 1.3 资产文件

| 资产 | 状态 | 详情 |
| ------ | ------ | ------ |
| `assets/live2d/niziiro_mao/` | ✅ | 主模型 Niziiro Mao (Cubism 5.0, 260 drawables) |
| `assets/live2d/shizuku/` | 🗑️ 已删除 (K1) | 不再分发 |
| `assets/shaders/` | ✅ | 7 个 GLES 2.0 着色器 |
| `assets/persona.yaml` | ✅ | 人设配置 |
| `assets/mcp_tools.json` | ✅ | MCP 工具定义 |

### 1.4 API Keys

| Key | 位置 | 状态 |
| ----- | ------ | ------ |
| `DEEPSEEK_API_KEY` | `local.properties` / `BuildConfig` | ✅ sk-4b31... |
| `ZHIPU_API_KEY` | `local.properties` / `BuildConfig` | ✅ a1e842... |
| 桌面端 API Keys | `Live2D-Ai-pc/open-llm-vtuber/conf.yaml` | ✅ 一致 |

---

## 2. 阻断问题清单

### 🔴 P0 — 必须修复

| # | 问题 | 影响 | 修复方案 |
|---|------|------|----------|
| P0-1 | **Gradle wrapper 缺失** | 无法构建 APK | 生成 `gradlew` + `gradle/wrapper/gradle-wrapper.jar` + `gradle-wrapper.properties`，使用 Gradle 8.10+ (从缓存) |

### 🟡 P1 — 需要注意

| # | 问题 | 影响 | 说明 |
| --- | ------ | ------ | ------ |
| P1-1 | 部署 IP 变更 | `deploy_android.sh` 写死 192.168.0.108 | 本次目标 IP 是 **192.168.0.102:5555** |
| P1-2 | vivo 安全守护 | `adb install` 可能被拦截 | FuntouchOS 弹出"已了解风险"复选框，需手动确认 |
| P1-3 | 覆盖安装限制 | `adb install -r` 可能失败 | vivo 需要 `adb uninstall` → `adb install` 全流程 |

### 🟢 P2 — 低风险

| # | 问题 | 影响 | 说明 |
|---|------|------|------|
| P2-1 | `android/` 目录残留 | 无功能影响 | 旧目录已空，可后续清理 |
| P2-2 | 系统 gradle 4.4.1 | 无法直接使用 | 但缓存中有 8.10+ 版本可用于生成 wrapper |

---

## 3. 构建流程

### 3.1 Step 1: 修复 Gradle Wrapper

```bash
# 进入项目目录
cd /f/Live2D-Ai/Live2D-Ai-Android

# 方案 A: 从 Gradle 缓存生成 wrapper (推荐)
# 使用缓存中的 gradle-8.12
~/.gradle/wrapper/dists/gradle-8.12-bin/*/gradle-8.12/bin/gradle wrapper --gradle-version 8.12

# 方案 B: 手动创建 gradle-wrapper.properties
# 创建 gradle/wrapper/gradle-wrapper.properties:
#   distributionBase=GRADLE_USER_HOME
#   distributionPath=wrapper/dists
#   distributionUrl=https\://services.gradle.org/distributions/gradle-8.12-bin.zip
#   networkTimeout=10000
#   zipStoreBase=GRADLE_USER_HOME
#   zipStorePath=wrapper/dists
#
# 然后从其他项目拷贝 gradle-wrapper.jar 和 gradlew 脚本

# 验证
ls -la gradlew gradlew.bat gradle/wrapper/gradle-wrapper.jar
```

### 3.2 Step 2: 验证 local.properties

```bash
# 确认 SDK 路径
cat Live2D-Ai-Android/local.properties
# 预期:
#   sdk.dir=/opt/android-sdk
#   ndk.dir=/opt/android-sdk/ndk/27.0.12077973
#   DEEPSEEK_API_KEY=sk-***REDACTED-ROTATED***
#   ZHIPU_API_KEY=***ZHIPU-KEY-REDACTED-ROTATED***
```

### 3.3 Step 3: 构建 APK

```bash
cd /f/Live2D-Ai/Live2D-Ai-Android

# 设置环境变量
export ANDROID_HOME=/opt/android-sdk
export ANDROID_NDK_HOME=/opt/android-sdk/ndk/27.0.12077973

# 清理并构建
./gradlew clean assembleDebug

# 验证产出
ls -lh app/build/outputs/apk/debug/app-debug.apk
# 预期大小: ~280KB (纯代码) 或 ~20MB (含模型资产)
```

### 3.4 Step 4: 验证 APK 结构

```bash
# 检查 ABI 和 native 库
unzip -l app/build/outputs/apk/debug/app-debug.apk | grep -E "\.so$|\.moc3$|\.json$"

# 确认 liblive2dcubismcore.so 在 arm64-v8a 下
# 确认模型文件在 assets/live2d/niziiro_mao/ 下
```

### 3.5 Step 5: 运行单元测试 (可选但推荐)

```bash
cd /f/Live2D-Ai/Live2D-Ai-Android
./gradlew test
# 预期: 5 个测试类全部通过
```

---

## 4. 部署流程

### 4.1 连接测试手机

```bash
# Step 1: 连接 (目标 IP: 192.168.0.102:5555)
adb connect 192.168.0.102:5555

# Step 2: 验证连接
adb devices
# 预期输出:
#   192.168.0.102:5555    device

# Step 3: 如果连接失败，检查 ADB 服务
adb kill-server && adb start-server
adb connect 192.168.0.102:5555
```

### 4.2 安装 APK

```bash
# Step 1: 完全卸载旧版 (vivo 必须这样做)
adb -s 192.168.0.102:5555 uninstall com.live2d.ai.android

# Step 2: 安装新版
adb -s 192.168.0.102:5555 install app/build/outputs/apk/debug/app-debug.apk

# ⚠️ 注意: vivo FuntouchOS 会弹出"安全守护"对话框
# 如果 install 返回 INSTALL_FAILED_ABORTED:
#   → 检查手机屏幕是否有安全弹窗
#   → 手动勾选"已了解风险" + 点击"继续安装"
#   → 或使用 input tap 模拟点击 (坐标因机型而异)

# Step 3: 验证安装
adb -s 192.168.0.102:5555 shell pm list packages | grep -i "live2d"
# 预期: package:com.live2d.ai.android
```

### 4.3 启动应用

```bash
# 启动
adb -s 192.168.0.102:5555 shell am start -n com.live2d.ai.android/.MainActivity

# 等待启动 (约 1-3 秒)
sleep 3

# 验证前台 Activity
adb shell dumpsys window | grep mCurrentFocus
# 预期: ...com.live2d.ai.android/.MainActivity
```

---

## 5. 功能验证矩阵

### 5.1 Live2D 渲染验证

| ID | 验证项 | 方法 | 通过标准 | 优先级 |
| ---- | -------- | ------ | ---------- | -------- |
| R1 | 模型加载 | 启动应用后等待 1s，检查 logcat | 无 `UnsatisfiedLinkError`，无 crash | P0 |
| R2 | 模型可见性 | 截图分析 | 非黑屏，有显著像素内容 (>10,000 非黑像素) | P0 |
| R3 | 4 手 bug 修复 | 截图 + crop face region | `PartArmLB=0.0` 且 `PartArmRB=0.0` | P0 |
| R4 | 动画运行 | 连续截图 2 帧 (间隔 200ms)，比较像素差异 | 像素差异 > 10,000 (说明动画在动) | P0 |
| R5 | 渲染性能 | 开发选项 GPU 呈现模式 | 无掉帧条超过 16ms 红线 | P1 |
| R6 | 着色器编译 | logcat 过滤 ShaderManager | 无 `GL_COMPILE_STATUS` 错误 | P1 |

**验证命令**:

```bash
# R1: 检查启动日志
adb -s 192.168.0.102:5555 logcat -d | grep -iE "Live2D|native|UnsatisfiedLink|FATAL"

# R2: 截图 + 像素分析
adb -s 192.168.0.102:5555 exec-out screencap -p > /tmp/e2e-screenshot-r2.png
# 分析工具: python -c "from PIL import Image; ..."

# R3: 截图分析 (专注 Face/Eyes/Arms 区域)
# 使用 diff_render 工具比较新旧截图

# R4: 连续截图
for i in 1 2; do
  adb -s 192.168.0.102:5555 exec-out screencap -p > /tmp/e2e-frame-$i.png
  sleep 0.2
done
```

### 5.2 DeepSeek 对话验证

| ID | 验证项 | 方法 | 通过标准 | 优先级 |
| ---- | -------- | ------ | ---------- | -------- |
| C1 | API 连通性 | logcat 中查找 DeepSeek 请求日志 | 响应码 200，无 `API请求失败` | P0 |
| C2 | 流式响应 | 发送测试消息"你好"，观察 UI | 对话框显示 AI 回复内容 | P0 |
| C3 | 人设生效 | 检查回复内容与 persona.yaml 一致性 | 回复以"喵"结尾，语气符合猫娘 | P1 |
| C4 | 表情标签 | 回复中包含 [emotion] 标签 | 猫娘表情切换 (如 `[joy]` → 笑脸) | P1 |
| C5 | 对话连续性 | 发送第二条消息，检查上下文 | 回复能引用上文内容 | P1 |

**验证命令**:

```bash
# C1: 检查 API 日志
adb -s 192.168.0.102:5555 logcat -d | grep -iE "ChatService|DeepSeek|API|Bearer|401|403|200"

# C2-C5: 需要手动 UI 操作
# 使用 adb input 模拟文本输入或观察屏幕
```

### 5.3 Edge TTS 语音验证

| ID | 验证项 | 方法 | 通过标准 | 优先级 |
| ---- | -------- | ------ | ---------- | -------- |
| T1 | Edge TTS 连通性 | logcat 中查找 EdgeTts 日志 | WebSocket 连接成功，音频流接收 | P0 |
| T2 | 音频播放 | 发送消息触发回复，听手机扬声器 | 听到中文语音 (XiaoxiaoNeural 女声) | P0 |
| T3 | 降级链 (Edge→系统TTS) | 断网后测试 | 自动降级到 Android TTS | P1 |
| T4 | LipSync 联动 | 观察猫娘嘴型 | 说话时嘴型参数变化 | P1 |

**验证命令**:

```bash
# T1: 检查 TTS 日志
adb -s 192.168.0.102:5555 logcat -d | grep -iE "EdgeTts|TTS|WebSocket|speak"

# T2: 需要实际听到音频输出
# T3: 开启飞行模式后重测
```

### 5.4 语音识别 (ASR) 验证

| ID | 验证项 | 方法 | 通过标准 | 优先级 |
| ---- | -------- | ------ | ---------- | -------- |
| A1 | 权限授予 | 首次启动检查权限弹窗 | RECORD_AUDIO 权限已授予 | P0 |
| A2 | 识别触发 | 点击麦克风按钮 | SpeechRecognizer 开始监听 | P1 |
| A3 | 中文识别 | 说话后检查识别文本 | 识别结果正确显示 | P1 |
| A4 | 语音→对话串联 | 说话→识别→发送→回复→TTS | 完整对话闭环 | P1 |

> **注意**: Android SpeechRecognizer 需要 Google 语音服务或厂商语音引擎。  
> vivo 设备通常预装 Jovi 语音，可能不支持 `EXTRA_LANGUAGE=CHINESE`。  
> 如果 ASR 不可用，属于已知限制 — VoiceInputService 已有空结果降级。

**验证命令**:

```bash
# A1: 检查权限状态
adb -s 192.168.0.102:5555 shell dumpsys package com.live2d.ai.android | grep -A5 "granted=true"
# 预期: android.permission.RECORD_AUDIO: granted=true

# A2-A4: 需要手动语音交互
```

---

## 6. 回归测试检查表

> 基于 HANDOVER.md §4 的 7 个历史 Bug，确保在新构建中不回归。

| Bug # | 描述 | 验证方法 | 状态 |
| ------- | ------ | ---------- | ------ |
| B1 | `buildPartIndex()` 晚于动画系统初始化 | 检查 AnimationSystem.kt 初始化顺序 | ⬜ |
| B2 | Pose 在 model.update() 之前执行 | 检查 AnimationPipeline.update() 顺序 | ⬜ |
| B3 | `initParameters()` 强行覆盖 part opacity | 检查后肢/尾部 opacity ≠ 1.0 | ⬜ |
| B4 | Pose JSON 解析错误 | Pose 初始化日志无异常 | ⬜ |
| B5 | Pose 全开不隐藏 | 手臂 part 只有第一个=1.0 | ⬜ |
| B6 | Pose 修改 Java 副本 | `setPartOpacity()` 通过 JNI 写入原生内存 | ⬜ |
| B7 | `modelPtr()` 返回 0L | 动画系统参数调用非空指针 | ⬜ |

**验证命令**:

```bash
# 综合 logcat 检查（启动后 5 秒内的日志）
adb -s 192.168.0.102:5555 logcat -c  # 清空
adb -s 192.168.0.102:5555 shell am start -n com.live2d.ai.android/.MainActivity
sleep 5
adb -s 192.168.0.102:5555 logcat -d | grep -iE "Pose|PartArm|FATAL|Exception|Error|FAILED"
```

---

## 7. 文件清单

### 需要创建的文件

| 文件 | 用途 | 说明 |
| ------ | ------ | ------ |
| `Live2D-Ai-Android/gradlew` | Gradle wrapper 启动脚本 | Unix shell 脚本 |
| `Live2D-Ai-Android/gradlew.bat` | Gradle wrapper 启动脚本 | Windows 批处理 |
| `Live2D-Ai-Android/gradle/wrapper/gradle-wrapper.jar` | Wrapper JAR | 从 Gradle 分发版提取 |
| `Live2D-Ai-Android/gradle/wrapper/gradle-wrapper.properties` | Wrapper 配置 | distributionUrl 指向 gradle-8.12-bin.zip |

### 可能需要修改的文件

| 文件 | 修改内容 | 触发条件 |
| ------ | ---------- | ---------- |
| `Live2D-Ai-Android/local.properties` | 无需修改 (SDK 路径正确) | — |
| `scripts/deploy_android.sh` | IP 从 108→102 (可选) | 如需脚本化部署 |
| `Live2D-Ai-Android/app/build.gradle.kts` | 无需修改 | — |

### 输出文档

| 文件 | 用途 |
|------|------|
| `docs/plans/plan-verify-e2e.md` | 本文档 |
| `docs/plans/plan-verify-e2e.schema.json` | 验证项结构化定义 |

---

## 8. 实施顺序

```
Phase 0: 修复构建链 (预计 5 分钟)
  ├── 0.1 生成 gradle wrapper (gradle 8.12)
  ├── 0.2 验证 ANDROID_HOME
  └── 0.3 快速构建测试 (compileDebugKotlin 只检查语法)

Phase 1: 构建 & 部署 (预计 10 分钟)
  ├── 1.1 ./gradlew clean assembleDebug
  ├── 1.2 验证 APK 结构
  ├── 1.3 adb connect + install + launch
  └── 1.4 确认应用在前台

Phase 2: 功能验证 (预计 20 分钟)
  ├── 2.1 Live2D 渲染 R1-R4 (P0)
  ├── 2.2 DeepSeek 对话 C1-C2 (P0)
  ├── 2.3 Edge TTS T1-T2 (P0)
  ├── 2.4 ASR A1 (P0)
  └── 2.5 回归检查 B1-B7

Phase 3: 深度验证 (预计 15 分钟)
  ├── 3.1 渲染性能 R5-R6
  ├── 3.2 对话功能 C3-C5
  ├── 3.3 TTS 降级 T3-T4
  └── 3.4 ASR 完整 A2-A4

Phase 4: 报告 (预计 5 分钟)
  └── 4.1 输出 e2e-results.json (按 schema)
```

---

## 9. 风险与注意事项

### 9.1 高风险项

1. **Gradle wrapper 生成可能失败** — 如果缓存中的 gradle 无法执行，备选方案是手动从 gradle.org 下载 gradle-8.12-bin.zip 并解压。

2. **vivo 安全守护可能多次弹出** — FuntouchOS 已知在 debug APK 安装时弹出安全对话框。可能需要用 `adb shell input tap` 模拟点击，但坐标因竖屏/横屏而异。

3. **CMake/NDK 编译错误** — Purism Core 使用 `posix_memalign` (需要 `_GNU_SOURCE`)，Android NDK r27 可能还需要 `-DANDROID_PLATFORM=android-26`。如果编译失败，检查 CMakeLists.txt 的编译选项。

### 9.2 已知限制

1. **Android SpeechRecognizer** 对中文支持依赖 Google 语音服务 / vivo Jovi 语音。如果设备未安装，ASR 验证 (A2-A4) 预期失败，属于平台限制而非代码 bug。

2. **截图分析** 依赖 Python + Pillow。如果环境缺少 Pillow，可使用 `diff_render` 工具代替。

3. **APK 大小** — 含模型资产的 Debug APK 约 20MB，不含 ProGuard 压缩。

### 9.3 回退策略

如果构建失败无法修复:

1. 尝试回退到上次已知可构建的 commit
2. 检查 Gradle/AGP 版本兼容性表
3. 使用 `--info` 或 `--stacktrace` 获取详细编译错误

---

## 10. 验证结果模板

验证完成后，按 `plan-verify-e2e.schema.json` 格式输出结果：

```json
{
  "timestamp": "2026-07-22T...",
  "device": "192.168.0.102:5555",
  "apk": "app-debug.apk",
  "build": { "success": true, "duration_s": 45, "size_bytes": 20000000 },
  "checks": [
    { "id": "R1", "name": "模型加载", "result": "PASS", "evidence": "..." },
    { "id": "R2", "name": "模型可见性", "result": "PASS", "evidence": "..." },
    { "id": "C1", "name": "API 连通性", "result": "PASS", "evidence": "..." },
    { "id": "C2", "name": "流式响应", "result": "PASS", "evidence": "..." },
    { "id": "T1", "name": "TTS 连通性", "result": "PASS", "evidence": "..." },
    { "id": "T2", "name": "音频播放", "result": "PASS", "evidence": "..." },
    { "id": "A1", "name": "权限授予", "result": "PASS", "evidence": "..." }
  ],
  "summary": {
    "total": 18,
    "pass": 18,
    "fail": 0,
    "conditional_pass": 0,
    "blocked": 0,
    "overall": "PASS"
  }
}
```

---

*本文档面向 SPOQ Wave Dispatch 系统。执行代理应严格按 Phase 0→1→2→3→4 顺序推进，遇到 P0 阻断立即报告。*

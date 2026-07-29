# Live2Dai — 项目交接文档

> 更新日期: 2026-07-22  
> 项目定位: **通用 Live2D 桌宠** — Live2D 角色 + LLM 语音对话  
> 架构: **两端独立直连云端 API**（无中心化后端）  
> 当前阶段: **MVP 验收通过 ✅**

---

## 一、项目结构

```
F:\Live2Dai\
├── shared/                  ← 跨平台共享配置（单源真理）
│   ├── persona.yaml         ← 人设（name/system_prompt/model/temperature）
│   └── mcp_tools.json       ← MCP 工具定义
│
├── android-native/          ← ★ Android 原生应用 (Kotlin + Compose)
│   ├── app/
│   │   ├── build.gradle.kts ← AGP 8.7.3, compileSdk 35, NDK 27
│   │   ├── src/main/
│   │   │   ├── cpp/
│   │   │   │   ├── native-lib.cpp     ← JNI 桥接层（所有模型操作）
│   │   │   │   ├── CMakeLists.txt     ← NDK 构建（编译 Purism Core）
│   │   │   │   └── purism/            ← Purism Core (MIT) 源码
│   │   │   ├── java/com/live2dai/android/
│   │   │   │   ├── MainActivity.kt          ← Compose UI + 生命周期
│   │   │   │   ├── Live2DRenderer.kt        ← ★ 主渲染管线 (45KB)
│   │   │   │   ├── Live2DView.kt            ← GLSurfaceView + Compose 封装
│   │   │   │   ├── Live2DNative.kt          ← JNI 声明接口
│   │   │   │   ├── PurismModel.kt           ← 模型顶点数据处理
│   │   │   │   ├── PurismClippingManager.kt ← FBO 裁剪管理器
│   │   │   │   ├── AnimationSystem.kt       ← ★ 动画系统 (40KB)
│   │   │   │   ├── ShaderManager.kt         ← 着色器加载/编译
│   │   │   │   ├── ChatService.kt           ← DeepSeek API SSE 调用
│   │   │   │   ├── EdgeTtsService.kt        ← 语音合成（降级链）
│   │   │   │   ├── VoiceInputService.kt     ← 语音识别
│   │   │   │   ├── VisionService.kt         ← GLM-4V 截图分析
│   │   │   │   ├── EmotionController.kt     ← 情感标签 → 表情映射
│   │   │   │   ├── PersonaConfig.kt         ← 人设 YAML 解析
│   │   │   │   └── ApiModels.kt             ← API 数据模型
│   │   │   ├── assets/
│   │   │   │   ├── live2d/mao_pro/runtime/  ← 当前使用的 Live2D 模型
│   │   │   │   ├── live2d/shizuku/runtime/  ← 第二模型（备用）
│   │   │   │   ├── shaders/                 ← GLES 2.0 着色器（7个文件）
│   │   │   │   └── persona.yaml             ← 人设（从 shared/ 同步）
│   │   │   └── res/
│   │   └── libs/                 ← 不再需要 Cubism SDK AAR
│   └── settings.gradle.kts
│
├── desktop/                  ← Windows 桌面端
│   ├── open-llm-vtuber/       ← Open-LLM-VTuber (git clone)
│   │   └── conf.yaml          ← 桌面端配置（API Key, 人设）
│   ├── start.ps1              ← 一键启动
│   ├── 启动桌宠.bat
│   └── README.md              ← 搭建指南
│
├── docs/
│   ├── ARCHITECTURE.md        ← 架构说明（2026-07-18）
│   └── SUMMARY.md             ← 项目总结（2026-07-14）
│
├── tests/                     ← 公共测试
├── scripts/                   ← 辅助脚本
├── archive/                   ← 旧架构文件归档
└── HANDOVER.md                ← ← 本文档
```

**关键文件大小**: (主力文件)

| 文件 | 大小 | 重要性 |
| --- | --- | --- |
| `Live2DRenderer.kt` | 45KB | ★ 渲染管线核心 |
| `AnimationSystem.kt` | 40KB | ★ 动画系统（Pose/Motion/EyeBlink/Breath/Physics） |
| `PurismClippingManager.kt` | 23KB | ★ 裁剪 FBO 管理 |
| `PurismModel.kt` | 13KB | 顶点数据处理 |
| `native-lib.cpp` | 20KB | JNI 桥接 |
| `MainActivity.kt` | 15KB | Compose UI |

---

## 二、架构详解

### 2.1 数据流

```
Android 端（当前 MVP）：

onDrawFrame() 每帧调用:
  ┌─────────────────────────────────────┐
  │ pipeline.update(deltaTime)          │
  │  ├─ AnimationPipeline               │
  │  │  ├─ MotionManager.update(dt)     │ ← 当前播放的 motion
  │  │  ├─ EyeBlinkManager.update(dt)   │ ← 自然眨眼
  │  │  ├─ BreathController.update(dt)  │ ← 胸部起伏
  │  │  ├─ PhysicsManager.update(dt)    │ ← 物理效果
  │  │  ├─ 保存参数快照                   │
  │  │  ├─ model.update()               │ ← 第一次更新（Cubism Core）
  │  │  ├─ Pose.updateParameters()      │ ← 隐藏后臂
  │  │  └─ model.update()               │ ← 第二次更新（反映 Pose 效果）
  │  └─                                 │
  ├─ refreshCachedAttributes()          │ ← 读取原生状态
  ├─ buildDrawableList()                │ ← 按 renderOrder 排序
  ├─ preDraw()                          │ ← 重置 OpenGL 状态
  └─ drawObjectLoop()                   │ ← 遍历渲染每个 drawable
     ├─ mask FBO (需要时)
     ├─ setShaderParameter
     └─ glDrawElements
```

### 2.2 渲染管道

| 阶段 | 功能 | 关键技术 |
| --- | --- | --- |
| 着色器 | 7个 GLES 2.0 着色器 | PremultipliedAlpha 管道 |
| 混合模式 | 3种: 正常/合成/相乘 | GL_ONE / GL_SRC_ALPHA / GL_ZERO+GL_ONE_MINUS_SRC_COLOR |
| 裁剪掩码 | 4张 FBO (512×512) | 多图渲染至 MRT |
| 抗锯齿 | MSAA 4x | 自定义 EGLConfigChooser |
| 纹理过滤 | 三线性 + Mipmap | GL_LINEAR_MIPMAP_LINEAR |

---

## 三、Live2D 引擎选型

### 方案对比

| | 官方 Cubism SDK (已废弃) | 当前: Purism Core |
| --- | --- | --- |
| 原生层 | Cubism Core AAR (闭源) | **Purism Core** (MIT, GitHub) |
| Java层 | 反射调用+透明桥 (~600行) | **CubismJavaFramework** + 自定义 renderer |
| 许可 | 需注册 Live2D 账号 | 无需许可 |
| 集成 | 手动下载 AAR | CMake + NDK 自动编译 |

**Purism Core**: <https://github.com/SakuraMotion/PurismCore> (MIT license)  
由 Cubism Core 逆向工程实现，完全兼容 Cubism 5.0 MOC3。性能对等。

### JNI 桥接

`native-lib.cpp` 是 Java 和 Purism Core 之间的唯一桥梁。

核心接口 (`Live2DNative.kt`):

| 函数 | 用途 | 备注 |
| --- | --- | --- |
| `loadModel(path)` | 加载 MOC3 文件 | 返回 modelPtr (long) |
| `updateModel(ptr)` | 执行模型更新 | 内部先 reset dynamic flags |
| `getPartOpacities(ptr)` | 读取 part opacity | **返回 Java COPY!!** |
| `setPartOpacity(ptr, idx, val)` | 写入 part opacity | ★ 写入原生内存 |
| `getParameterValue(ptr, idx)` | 读取参数值 | |
| `setParameterValue(ptr, idx, val, weight)` | 写入参数值 | |
| `getDrawableDynamicFlags(ptr)` | 读取 dynamic flags | Java COPY |
| `getDrawableVertices(ptr, idx)` | 读取顶点数据 | |
| `getDrawableVertexIndices(ptr, idx)` | 读取索引数据 | |

> **⚠️ 关键陷阱**: `getPartOpacities()` 返回的 `FloatArray` 是 JNI 副本！**修改它不会影响模型状态**。必须用 `setPartOpacity()` 写入。  
> 详见 "四、Bug 修复记录" 第6条。

---

## 四、Bug 修复记录

> 这是项目最宝贵的隐形资产，新接手者必读。

### Bug 1: `buildPartIndex()` 晚于动画系统初始化

- **现象**: Pose 永远不生效，角色始终 4 只手
- **根因**: `Live2DRenderer.loadModel()` 中步骤 8 `setupAnimationSystem()` 在步骤 9 `buildPartIndex()` 之前，Pose 收到的 `partNameToIndex` 是空 map
- **修复**: 交换调用顺序 → `buildPartIndex()` 先于 `setupAnimationSystem()`
- **教训**: 初始化顺序决定功能有效性
- **文件**: `Live2DRenderer.kt` line ~448

### Bug 2: Pose 在 `model.update()` 之前执行

- **现象**: Pose 似乎运行了，但效果被 `model.update()` 覆盖
- **根因**: Pose 修改的 part opacity 在后续的 `model.update()` 中被重置
- **修复**: Pose 移到 `model.update()` 之后，并再跑一次 `model.update()` 让 drawable visibility 重新计算
- **教训**: `csmUpdateModel()` 会使用当前 part opacity 重新计算 drawable 可见性
- **文件**: `AnimationSystem.kt` AnimationPipeline.update()

### Bug 3: `initParameters()` 强行覆盖所有 part opacity

- **现象**: 后肢和尾部等不该全开的部分 opacity=1.0
- **根因**: 初始化时遍历所有 part 设 opacities[i]=1.0，覆盖了 MOC3 原始值
- **修复**: 删除该操作，只初始化 PARAMETERS（眼睛等）
- **文件**: `Live2DRenderer.kt` initParameters()

### Bug 4: Pose JSON 解析错误

- **现象**: Pose 初始化永远失败
- **根因**: JSON 结构是 `{"Type": "Pose", "Groups": [[...], [...]]}`，但用了 `JSONArray()` 解析 Object 根节点
- **修复**: `poseRoot.getJSONArray("Groups")`
- **文件**: `AnimationSystem.kt` PoseManager

### Bug 5: Pose 全开不隐藏

- **现象**: Pose 组内所有 part 都被设 1.0
- **根因**: 遍历组内所有 part 都 setOpacity(1.0)
- **修复**: 组内第一个 1.0，其余 0.0
- **文件**: `AnimationSystem.kt` PoseManager.updateParameters()

### Bug 6: Pose 修改 Java 副本而非原生内存

- **现象**: Pose 明明调用了 `setPartOpacity()`，但 `getPartOpacities()` 仍返回 1.0
- **根因**: `getPartOpacities()` JNI 返回 `NewFloatArray` + `SetFloatArrayRegion` —— 这是一个**COPY**，修改它不通知原生层
- **修复**: 新增 `setPartOpacity()` JNI 函数，直接写 `csmGetPartOpacities()` 返回的指针
- **教训**: JNI 的 `Get/SetFloatArrayRegion` 永远是 COPY 语义，不是 PTR 语义
- **文件**: `native-lib.cpp` + `Live2DNative.kt`

### Bug 7: `modelPtr()` 返回 0L

- **现象**: MotionManager/Breath/Physics 等虽然代码执行了，但参数调用全走空指针
- **根因**:

  ```kotlin
  // 错误
  fun modelPtr(): Long = purismModel?.modelPtr ?: 0L  // 返回 0!
  // 正确
  fun modelPtr(): Long = model.modelPtr  // 直接取值
  ```

  因为 class 不同，`purismModel` 属性在子类为 null
- **修复**: 全局统一用 `model.modelPtr`
- **文件**: `AnimationSystem.kt` 各 inner class

---

## 五、动画系统架构

`AnimationSystem.kt` (~1206 行) 是动画控制中枢。

### 核心类

| 类 | 行数 | 职责 |
| --- | --- | --- |
| `MotionManager` | ~200 | 播放/交叉淡入淡出/队列 |
| `MotionData` | data | 参数曲线数据 |
| `EyeBlinkManager` | ~80 | 自然眨眼（随机间隔） |
| `BreathController` | ~60 | 呼吸起伏 |
| `PhysicsManager` | ~100 | 物理效果（辫子、衣服等） |
| `PoseManager` | ~100 | 肢体可见性控制（Pose JSON） |
| `AnimationPipeline` | ~30 | 编排上述所有组件顺序 |

### 动画更新顺序

```kotlin
AnimationPipeline.update(deltaTime):
  1. MotionManager.update(dt)      ← 参数曲线驱动
  2. EyeBlinkManager.update(dt)    ← 眨眼
  3. BreathController.update(dt)   ← 呼吸
  4. PhysicsManager.update(dt)     ← 物理
  5. 参数 clamp 到 [0,1]
  6. model.setParameterValue()     ← 写入参数
  7. saveParameters()              ← 保存快照（Motion 需要计算 delta）
  8. model.update()                ← ★ 第一次更新
  9. Pose.updateParameters()       ← ★ 修改 part opacity
  10. model.update()               ← ★ 第二次更新（让 drawable visibility 正确）
```

> ⚠️ **此顺序严格绑定 Cubism Core 的更新机制**：
>
> - `saveParameters()` 必须紧跟在所有参数写入之后、`model.update()` 之前
> - Pose 必须在第一次 `model.update()` 之后（因 Pose 需要知道当前参数状态）
> - 第二次 `model.update()` 让 visibility flag 反映 Pose 的修改

---

## 六、构建 & 部署

### 前提条件

- Android Studio 或命令行 Gradle
- NDK 27.0.12077973（自动下载）
- JDK 17+

### 构建命令

```bash
cd F:/Live2Dai/android-native
ANDROID_HOME=/opt/android-sdk ANDROID_NDK_HOME=/opt/android-sdk/ndk/27.0.12077973 \
  ./gradlew assembleDebug
```

APK 路径: `app/build/outputs/apk/debug/app-debug.apk` (~20MB)

### 真机部署（无线 ADB）

```bash
# 连接（目标 IP: 192.168.0.x:5555）
adb connect 192.168.0.x:5555

# 卸载旧版（vivo 必须完全卸载）
adb uninstall com.live2dai.android

# 安装（需要 tap 安全对话框）
adb install app/build/outputs/apk/debug/app-debug.apk
# 然后手机上 tap 确认按钮:
adb shell input tap 540 2082   # 勾选"未知应用"
adb shell input tap 540 2229   # 确认安装

# 启动
adb shell am start -n com.live2dai.android/.MainActivity
```

> **重要**: vivo FuntouchOS 不允许 `adb install -r` 覆盖安装  
> 必须 `adb uninstall` → `adb install` 完整流程

### 截图验证

```bash
adb shell screencap -p /sdcard/shot.png
adb pull /sdcard/shot.png /tmp/
# 分析像素: python3 -c "from PIL import Image; print(sum(1 for r,g,b in Image.open('/tmp/shot.png').convert('RGB').getdata() if r>10 or g>10 or b>10))"
```

---

## 七、调试日志系统

| 日志标签 | 级别 | 内容 |
| --- | --- | --- |
| Live2DRenderer | INFO | 初始化、空闲运动启动 |
| Live2DNative | INFO | Canvas 信息、mask 统计 |
| Pose | INFO | Pose 初始化日志（首次） |
| AnimationPipeline | VERBOSE | 关键动画数据变化 |
| setPartOpacity | VERBOSE | ★ 调试用，已移除 |

> **当前日志策略**: 生产模式只保留 INFO 级初始化日志。所有 VERBOSE 调试日志已在 MVP 验收前移除。

---

## 八、已知问题 & 待办

### 技术债 / 需修复

| # | 问题 | 影响 | 建议修复 |
| --- | --- | --- | --- |
| 1 | `initParameters()` 硬编码了 `PARAM_EYE_L_OPEN`/`PARAM_EYE_R_OPEN` 等参数 ID | 换模型需改代码 | 从 model.cdi3.json 读取参数默认值 |
| 2 | `buildPartIndex()` 写死 `"PartArm"` 前缀匹配 | 换模型若臂部命名不同则不工作 | 从 pose3.json 自动解析组名 |
| 3 | 角色摸不到/交互无反馈 | 无法触摸互动 | 实现 touch 碰撞检测 |
| 4 | 无悬浮窗/后台模式 | 应用前台才可见 | Service + 悬浮窗权限 |
| 5 | 无表情/LipSync 集成 | [emotion] 标签有效但不联动 | EmotionController + Live2DView 绑定 |
| 6 | 无多模型切换 UI | 只有代码中切换 | Compose UI 模型选择器 |
| 7 | 无对话文本气泡 | 看不到猫娘在说什么 | Canvas overlay 文本渲染 |
| 8 | 无 Motion 淡入淡出 | 动作切换生硬 | 已实现交叉淡入但未启用全部特性 |

### 待验证

- [ ] Emotion → Live2D 表情联动端到端
- [ ] TTS 降级链（Edge TTS → Android TTS → 静默）
- [ ] MCP 工具在 Android 端调用
- [ ] 多模型（shizuku）切换测试

---

## 九、模型 & 配置

### 当前模型

- **主模型**: `mao_pro` (Cubism 5.0), 260 drawables, 133 visible
- **备用模型**: `shizuku` (Cubism 4.0?), 已嵌入 assets

### 添加新模型步骤

1. 将模型文件夹放入 `assets/live2d/`（含 `.model3.json` + `.moc3` + 贴图）
2. 如需切模型: 修改 `Live2DRenderer.loadModel()` 中的 `modelPath`
3. 修改 `shared/persona.yaml` 和 `assets/persona.yaml`

### API Key 设置

**Android**: `ChatService.kt` 常量:

```kotlin
private const val DEEPSEEK_API_KEY = "sk-..."
private const val GLM_API_KEY = "..."
```

**Windows**: `conf.yaml`:

```yaml
llm_api_key: 'sk-...'
zhipu_api_key: '...'
```

---

## 十、开发备忘

### 修改人设

```bash
# 编辑共享
vim shared/persona.yaml

# 同步到 Android
cp shared/persona.yaml android-native/app/src/main/assets/
```

### 运行测试

```bash
# Android 单元测试（5 个测试类）
cd android-native
./gradlew test

# 公共测试
cd F:/Live2Dai
python -m pytest tests/
```

### 代码统计

```bash
# Kotlin 源码
find android-native -name "*.kt" -not -path "*/test/*" -exec wc -l {} + | tail -1

# 所有文件总计
find . -name "*.kt" -o -name "*.cpp" -o -name "*.h" | xargs wc -l | tail -1
```

**当前总计: ~5,782 行 Kotlin/C++ 源码** (不含上游)

---

## 十一、相关链接

- **Purism Core** (MIT): <https://github.com/SakuraMotion/PurismCore>
- **CubismJavaFramework** (公开): <https://github.com/Live2D/CubismJavaFramework>
- **Open-LLM-VTuber**: <https://github.com/Open-LLM-VTuber/Open-LLM-VTuber>
- **DeepSeek API**: <https://platform.deepseek.com/>
- **Zhipu API**: <https://open.bigmodel.cn/>
- **Edge TTS**: <https://github.com/rany2/edge-tts>

---

## 附录: MVP 验收数据 (2026-07-22)

| 检查项 | 结果 |
| --- | --- |
| 渲染稳定性 (4帧差异) | 71K~99K px/帧 → ✅ 动画流畅 |
| 4 手 bug | PartArmLB=0.0, PartArmRB=0.0 → ✅ 已修复 |
| GLM-4.6V 视觉确认 (2次) | 2 只手, 2 只手 → ✅ |
| 启动耗时 | 1.0s → ✅ |
| 运行时异常 | 0 crashes, 0 ANR → ✅ |
| 旧版 vs 新版像素 | 22,633 减少 → ✅ 后臂隐藏 |
| APK 大小 | 278KB vs 320KB (旧版) → ✅ 减少41KB |

---

*本文档面向新接手开发者。初次阅读请重点看 "四、Bug 修复记录" 和 "五、动画系统架构"。*

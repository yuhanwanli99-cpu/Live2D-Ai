# 架构设计文档：模型参数配置化 (task-model-config)

> **版本**: 1.0  
> **日期**: 2026-07-29  
> **目标**: 消除 Live2DRenderer / AnimationSystem 中所有硬编码的参数 ID 和 part 前缀，使换模型不需要修改 Kotlin 源码。

---

## 1. 需求概述

### 1.1 问题

当前代码中有多处硬编码了 Live2D 参数名和 part 名前缀，导致换模型时必须修改 Kotlin 源码：

| 位置 | 硬编码内容 | 问题 |
| ------ | ----------- | ------ |
| `Live2DRenderer.initParameters()` | `"PARAM_EYE_L_OPEN"`, `"PARAM_EYE_R_OPEN"` | niziiro_mao 用 `ParamEyeLOpen` / `ParamEyeROpen`，此代码无声失效 |
| `AnimationPipeline` 构造 | `"PARAM_EYE_L_OPEN"`, `"PARAM_EYE_R_OPEN"` (primary) + `"ParamEyeLOpen"`, `"ParamEyeROpen"` (alt) | 硬编码了两种命名约定，第三种模型可能有第三种命名 |
| `AnimationPipeline` 构造 | `"ParamA"`, `"ParamMouthOpenY"` | 口型同步参数名因模型而异 |
| `BreathManager` | `"ParamAngleX"`, `"ParamAngleY"`, `"ParamAngleZ"`, `"ParamBodyAngleX"`, `"ParamBreath"` | 呼吸参数名/数量因模型而异 |
| `PoseManager.updateParameters()` | 始终选 `group.partIds[0]` 可见 | 没有用参数值驱动 part 选择 |

### 1.2 目标

1. **从 cdi3.json 自动发现参数语义** — 利用 `GroupId` 元数据识别眼睛/嘴/呼吸参数
2. **从 pose3.json 自动解析 part 组** — 利用已有 Groups 数据，结合参数驱动 part 选择
3. **零代码切换模型** — 放入新模型文件夹即可运行，不需要改 Kotlin

### 1.3 输入数据

| 文件 | 关键字段 | 用途 |
| ------ | --------- | ------ |
| `{model}.cdi3.json` | `Parameters[].Id`, `Parameters[].GroupId` | 参数名 → 语义角色映射 |
| `{model}.cdi3.json` | `ParameterGroups[].Id`, `ParameterGroups[].Name` | GroupId → 人类可读名称 |
| `{model}.pose3.json` | `Groups[][]` | Part 分组（互斥可见性） |
| `{model}.model3.json` | `FileReferences.Pose` | pose3.json 路径 |

---

## 2. 系统架构

### 2.1 新增组件：ModelMetaConfig

```
┌──────────────────────────────────────────────────┐
│                  ModelMetaConfig                  │
│  (新建, ~200 行)                                  │
│                                                   │
│  ┌─────────────────┐  ┌─────────────────────┐    │
│  │ ParameterMeta[]  │  │ PoseMeta            │    │
│  │  - id: String    │  │  - groups: List<    │    │
│  │  - groupId: Str  │  │      List<PartRef>  │    │
│  │  - index: Int    │  │    >                │    │
│  │  - name: String  │  │  - driverParamIdx   │    │
│  └─────────────────┘  └─────────────────────┘    │
│                                                   │
│  ┌──────────────────────────────────────────┐     │
│  │ SemanticIndex (computed)                  │     │
│  │  - eyeLeftOpenIdx: Int                    │     │
│  │  - eyeRightOpenIdx: Int                   │     │
│  │  - mouthOpenIndices: List<Int>            │     │
│  │  - breathParamIndices: List<BreathDef>    │     │
│  │  - angleX/Y/Z indices: Int...             │     │
│  └──────────────────────────────────────────┘     │
└──────────────────────────────────────────────────┘
         ▲                              ▲
         │ 构造时解析                    │ 传递给动画组件
         │                              │
  cdi3.json + pose3.json     Live2DRenderer / AnimationPipeline
```

### 2.2 数据流

```
loadModel():
  1. 加载 MOC3、纹理（不变）
  2. buildParameterIndex()                ← 不变
  3. ★ new: parseCdi3Json()               ← 解析 cdi3.json → ModelMetaConfig
  4. initParameters(meta)                 ← 使用 meta.eyeLeftOpenIdx 等
  5. model.update()
  6. buildPartIndex()                     ← 不变
  7. setupAnimationSystem(meta)           ← 传入 meta，替代硬编码查找
     ├── AnimationPipeline(model, meta)   ← 使用 meta 的 SemanticIndex
     ├── PoseManager(meta.poseMeta)       ← 使用 meta 的 PoseMeta  
     └── BreathManager(meta)              ← 使用 meta 的呼吸参数定义
```

---

## 3. 模块划分

### 3.1 新增文件

| 文件 | 大小估算 | 职责 |
|------|---------|------|
| `ModelMetaConfig.kt` | ~200 行 | 解析 cdi3.json + pose3.json，构建语义索引 |

### 3.2 修改文件

| 文件 | 修改范围 | 修改内容 |
| ------ | --------- | --------- |
| `Live2DRenderer.kt` | `loadModel()` + `initParameters()` + `setupAnimationSystem()` | 解析 cdi3.json，传入 meta 配置 |
| `AnimationSystem.kt` | `AnimationPipeline` 构造 + `applyEyeBlink()` + `BreathManager` | 接受 meta 参数替代硬编码查找 |
| `AnimationSystem.kt` | `PoseManager` 构造 | 可选：接受 driver param 索引 |

### 3.3 不变文件

| 文件 | 原因 |
| ------ | ------ |
| `PurismModel.kt` | 纯数据模型，不涉及参数语义 |
| `Live2DNative.kt` | JNI 接口，不涉及参数语义 |
| `native-lib.cpp` | C++ 层，不涉及参数语义 |
| `MainActivity.kt` | UI 层，不涉及参数语义 |
| `Live2DView.kt` | 视图封装，不涉及参数语义 |

---

## 4. 数据模型

### 4.1 ModelMetaConfig

```kotlin
/**
 * 从模型元数据文件（cdi3.json + pose3.json）解析的配置。
 * 替代所有硬编码的参数 ID 查找。
 */
class ModelMetaConfig(
    /** 所有参数的元数据列表（按 MOC3 索引顺序） */
    val parameters: List<ParameterMeta>,

    /** 语义索引：按功能角色预计算的参数索引 */
    val semantic: SemanticIndex,

    /** Pose 元数据（可选，仅当模型有 pose3.json） */
    val poseMeta: PoseMeta?
) {
    data class ParameterMeta(
        val id: String,         // 参数 ID（MOC3 中的名字）
        val groupId: String,    // 所属参数组 ID（如 "ParamGroupEyes"）
        val name: String,       // 人类可读名称（如 "Eye L_Open"）
        val index: Int          // 在 MOC3 参数数组中的索引
    )

    data class SemanticIndex(
        /** 左眼睁开参数索引，-1 表示不存在 */
        val eyeLeftOpenIdx: Int,
        /** 右眼睁开参数索引，-1 表示不存在 */
        val eyeRightOpenIdx: Int,
        /** 口型同步参数索引列表（可能有多个，如 ParamA + ParamMouthOpenY） */
        val mouthOpenIndices: List<Int>,
        /** 呼吸参数定义列表 */
        val breathParams: List<BreathParamDef>,
        /** 头部旋转 X 参数索引 */
        val angleXIdx: Int,
        /** 头部旋转 Y 参数索引 */
        val angleYIdx: Int,
        /** 头部旋转 Z 参数索引 */
        val angleZIdx: Int,
        /** 身体旋转 X 参数索引（呼吸用） */
        val bodyAngleXIdx: Int
    )

    data class BreathParamDef(
        val index: Int,         // 参数索引
        val id: String,         // 参数 ID（调试用）
        val offset: Float,      // 呼吸基线值
        val peak: Float,        // 呼吸幅度
        val cycle: Float,       // 呼吸周期（秒）
        val weight: Float       // 呼吸权重
    )

    data class PoseMeta(
        /** Pose 分组列表，每组是一组互斥的 part */
        val groups: List<PoseGroup>
    )

    data class PoseGroup(
        /** 组内 part ID 列表（如 ["PartArmLA", "PartArmLB"]） */
        val partIds: List<String>,
        /** 驱动此组选择的参数索引（-1 表示无驱动参数，始终选第一个） */
        val driverParamIdx: Int = -1
    )
}
```

### 4.2 语义发现规则

```
从 cdi3.json Parameters[] 构建 GroupId → List<paramIndex> 的映射，然后：

角色 "eyeOpen":
  输入: ParamGroupEyes 组中的所有参数
  匹配: ID 包含 "Open"（不区分大小写）
  分类: 包含 "L" 或 "Left" → eyeLeftOpenIdx; 包含 "R" 或 "Right" → eyeRightOpenIdx
  兜底: 如果只有一个匹配且无法区分左右，则双眼共用

角色 "mouthOpen":
  输入: ParamGroupMouth 组中的所有参数
  匹配: ID 为 "ParamA" 或 "ParamMouthOpenY"或包含 "Mouth"+"Open" 的参数
  输出: List<Int>（可能有多个口型参数）

角色 "breath":
  输入: ParamGroupBody 组中的所有参数
  匹配: ID 匹配 ParamAngleX / ParamAngleY / ParamAngleZ / ParamBodyAngleX / ParamBreath
  默认值: offset/peak/cycle/weight 使用现有硬编码值作为默认（可通过未来配置覆盖）

角色 "faceAngle":
  输入: ParamGroupFace 组中的所有参数  
  匹配: ID 匹配 ParamAngleX / ParamAngleY / ParamAngleZ
  输出: angleXIdx, angleYIdx, angleZIdx
```

### 4.3 Pose 驱动参数关联

```
Pose 组的驱动参数发现规则:
  1. 从 PoseGroup 的第一个 partId 提取前缀（如 "PartArmLA" → "ArmLA"）
  2. 在 cdi3.json ParameterGroups 中查找名称包含此外缀的组
     （如 "PartArmLA" → 查找 Name="Arm L A" → 找到 ParamGroupArmLA）
  3. 将该组中 ID 包含 "01" 或 "Angle" 的参数作为驱动参数
     （如 "ParamArmLA01" — 肩部旋转角度）
  4. 用参数值决定显示哪个 part: 值 < 阈值 → partIds[0], 否则 → partIds[1]
```

---

## 5. 接口定义

### 5.1 ModelMetaConfig 工厂方法

```kotlin
companion object {
    /**
     * 从 cdi3.json 和 pose3.json 构建 ModelMetaConfig。
     *
     * @param cdi3Json cdi3.json 的 JSONObject（已解析）
     * @param pose3Json pose3.json 的 JSONObject（已解析，可选）
     * @param paramNameToIndex 参数名→索引映射（从 MOC3 获取）
     * @param partNameToIndex part 名→索引映射（从 MOC3 获取）
     */
    fun parse(
        cdi3Json: JSONObject,
        pose3Json: JSONObject?,
        paramNameToIndex: Map<String, Int>,
        partNameToIndex: Map<String, Int>
    ): ModelMetaConfig
}
```

### 5.2 修改后的 AnimationPipeline 构造

```kotlin
// 旧签名
class AnimationPipeline(
    private val model: PurismModel,
    private val paramNameToIndex: Map<String, Int>,
    private val partNameToIndex: Map<String, Int> = emptyMap()
)

// 新签名
class AnimationPipeline(
    private val model: PurismModel,
    private val paramNameToIndex: Map<String, Int>,
    private val meta: ModelMetaConfig,
    private val partNameToIndex: Map<String, Int> = emptyMap()
)
```

### 5.3 修改后的 BreathManager

```kotlin
// 旧：硬编码 breathParams
class BreathManager { ... }

// 新：接受 ModelMetaConfig
class BreathManager {
    fun initialize(meta: ModelMetaConfig) { ... }
}
```

### 5.4 修改后的 PoseManager.initialize()

```kotlin
// 旧签名
fun initialize(jsonBytes: ByteArray, partIds: Array<String>?)

// 新签名
fun initialize(jsonBytes: ByteArray, partIds: Array<String>?, meta: ModelMetaConfig?)
// meta 用于获取驱动参数索引
```

---

## 6. 文件清单（新增/修改）

### 6.1 新增

| 路径 | 说明 |
| ------ | ------ |
| `Live2D-Ai-Android/app/src/main/java/com/live2d/ai/android/ModelMetaConfig.kt` | 核心配置类 + 解析逻辑 |
| `docs/plans/plan-task-model-config.md` | 本文档 |
| `docs/plans/plan-task-model-config.schema.json` | ModelMetaConfig 序列化 schema（用于调试/缓存） |

### 6.2 修改

| 路径 | 修改内容 |
| ------ | --------- |
| `Live2DRenderer.kt` | `loadModel()`: 解析 cdi3.json 构建 ModelMetaConfig<br>`initParameters()`: 使用 meta.semantic.eyeLeftOpenIdx 等<br>`setupAnimationSystem()`: 传入 meta |
| `AnimationSystem.kt` | `AnimationPipeline`: 接受 meta，用 meta.semantic 替代硬编码查找<br>`BreathManager`: 接受 meta 初始化呼吸参数<br>`PoseManager`: 可选，接受 meta 获取驱动参数索引 |

---

## 7. 实施顺序

### Phase 1: ModelMetaConfig 解析器（独立可测试）

1. 创建 `ModelMetaConfig.kt` → 实现 `parse()` 方法
2. 在 `loadModel()` 第 5 步（buildParameterIndex 后）调用 `parse()`
3. 单元测试：用 niziiro_mao 的 cdi3.json 验证解析结果

### Phase 2: 替换 initParameters 硬编码

1. 修改 `initParameters(signature)` 接受 `ModelMetaConfig`
2. 用 `meta.semantic.eyeLeftOpenIdx` / `eyeRightOpenIdx` 替代字符串比较
3. 保留兜底：如果 semantic index = -1，尝试旧版硬编码名称 + 日志警告

### Phase 3: 替换 AnimationPipeline 硬编码

1. 修改 `AnimationPipeline` 构造接受 `ModelMetaConfig`
2. `applyEyeBlink()` 使用 `meta.semantic.eyeLeftOpenIdx` / `eyeRightOpenIdx`
3. `setLipSyncValue()` 使用 `meta.semantic.mouthOpenIndices`
4. 删除旧的 `eyeLOpenIndex` / `eyeLOpenIndexAlt` 等字段

### Phase 4: 替换 BreathManager 硬编码

1. `BreathManager` 新增 `initialize(meta)` 方法
2. 从 `meta.semantic.breathParams` 读取呼吸参数定义
3. 如果 meta 中没有呼吸参数（旧模型无 cdi3.json），保持现有硬编码作为 fallback

### Phase 5: Pose 驱动参数关联

1. `PoseManager.initialize()` 接受 `ModelMetaConfig`
2. 实现驱动参数发现规则（§4.3）
3. `updateParameters()` 根据参数值选择可见 part（而非总是 index 0）

---

## 8. 风险与注意事项

### 8.1 向后兼容

- **旧模型兼容**: 如果模型没有 cdi3.json（Cubism 2.x/3.x），`parse()` 返回 null，所有消费者回退到现有硬编码行为
- **未知 GroupId**: 对于 cdi3.json 中存在但未映射到已知语义角色的 GroupId，仅记录日志，不影响功能
- **参数不存在**: 每个 semantic index 初始化为 -1，消费者调用前检查；-1 时静默跳过

### 8.2 模型差异

- **命名约定差异**: Cubism 2.x 用 `PARAM_EYE_L_OPEN`（全大写+下划线），Cubism 4.x/5.x 用 `ParamEyeLOpen`（驼峰）。cdi3.json 统一使用后者。对于没有 cdi3.json 的旧模型，回退到名称启发式匹配。
- **参数数量差异**: 不同模型的 eye/mouth 参数数量不同。`mouthOpenIndices` 使用 List 而非单个 Int。
- **呼吸参数差异**: 有的模型有 `ParamBreath`，有的没有。`BreathManager` 遍历 breathParams 列表，仅应用 model 中实际存在的参数。

### 8.3 性能

- cdi3.json 解析仅在模型加载时执行一次（~5ms for 100+ params），可忽略
- `SemanticIndex` 是预计算的常量值，运行时无额外开销
- 每个 `applyEyeBlink()` 调用省去了字符串比较循环，实际性能略有提升

### 8.4 调试

- 所有语义发现的结果通过 `Log.i("ModelMetaConfig", ...)` 输出
- 标签示例: `"ModelMetaConfig: eyeLeftOpen=ParamEyeLOpen(idx=5), eyeRightOpen=ParamEyeROpen(idx=8)"`
- 未匹配到参数时输出 `Log.w` 警告，方便排查新模型适配问题

### 8.5 已知限制

- **参数语义发现依赖 GroupId**: 如果模型的 cdi3.json 中 GroupId 命名不规范（如所有参数都在一个组），语义发现会退化到名称启发式匹配
- **Pose 驱动参数关联依赖命名约定**: 依赖 PartArmLA ↔ ParamGroupArmLA 的命名一致性。如果模型的 Part/Paremeter 命名不遵循此约定，驱动参数会退化为 -1（始终显示第一个 part）
- **Breath 参数值（offset/peak/cycle）仍为硬编码默认值**: cdi3.json 不含呼吸曲线参数，这些值来自 Cubism SDK 参考实现。未来可通过额外的 breath-config.json 覆盖

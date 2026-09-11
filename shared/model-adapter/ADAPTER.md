# ADAPTER.md（模型适配扩展指南）

> 契约 v3-adapter §4 锁定的扩展指南。适配点 = 单文件配置 + 加载点，**不做多模型注册系统/API/UI**（K3 最小框架）。
> 本文件随 `bai.adapter.json` 一起修订；变更记录见 §4。

## 1. 适配点是什么

双端（Android / PC）共享**唯一**运行时适配真相源：`shared/model-adapter/{modelId}.adapter.json`。

- 一个模型 = 一个 adapter JSON 文件（schema 见 contract-v3-adapter §1.2，字段名锁死，禁止加 decision/自由扩展字段）
- 加载点：Android `ModelAdapterConfig.load()`（assets，Gradle `copyModelAdapter` 同步）；PC `_load_model_adapter()`（模块加载时一次）
- 加载失败 → 回退现有硬编码常量（行为零变化，降级链第 1 级）
- 不做：多模型切换 API / 运行时热加载 / UI（K3 范围外）

## 2. 加新模型要改哪里（清单式）

1. **复制 adapter 文件**：`bai.adapter.json` → `{新模型}.adapter.json`，改 `modelId` / `displayName`（与 registry 一致）
2. **motionAliases**：按新模型 `{模型}.model3.json` FileReferences.Motions **实读**重填（7 键 tag → {group, index, file}；组名可为空串 `""`，不是笔误——空组键格式为 `_N`）。tag 语义（wave=special_01 等）需按新模型动作内容重新验证（[UNSURE-1] 精神：motion3.json 无动作语义标签）
3. **vowelParamMap / vowelRawMap**：按新模型 `{模型}.cdi3.json` 参数集 + pymouth 表 2 公式导出：
   - `vowelRawMap` = pymouth VOWEL_MAP 原值 `[mo, mf]`
   - `vowelParamMap` = buildVisemeFrame 公式（实读 VisemeAnalyzer.kt）：`A→ParamA=mo, ParamO=mf×0.2`；`I→ParamI=mf`；`U→ParamU=mf×0.7`；`E→ParamA=mo×0.3, ParamE=mo×0.7`；`O→ParamU=mf×0.3, ParamO=mo`；SIL 全 0
   - 模型缺五元音参数 → 对应 param 键从映射中删除（运行时走 RMS 降级），或整段留空表
4. **idlePool**：按新模型 idle 可用池实读（Android 键 = `${group}_$index`，空组 `_N`）
5. **emotionIndex**：按新模型表情表实读（8 键 0-7；新模型表情数 ≠ 8 时须契约修订——schema 锁 8 key）
6. **registry 加条目**：`shared/model_registry.json` + schema；双端资源落位（Android assets + PC live2d-models/）
7. **加载点**：Android 传新 asset 路径（Gradle `copyModelAdapter` 挂新文件）、PC 传新 `adapter_path`
8. **校验**：跑 T1.4 一致性校验（C24-C28）+ 双端单测回归门（Android `:app:testDebugUnitTest` / PC `pytest tests/`）

## 3. 降级链说明（锁定，双端一致）

```
配置缺失 → 硬编码常量 → 无参数 → RMS/随机 → 无口型
```

| 级别 | 触发条件 | 行为 | 双端位置 |
|---|---|---|---|
| 0 配置缺失 | adapter 文件缺失/解析失败 | 回退现有硬编码常量（零行为变化） | Android `ModelAdapterConfig.load()` → null；PC `_load_model_adapter()` → None |
| 1 常量兜底 | 配置不可用 | MotionTags / VisemeAnalyzer / EMOTION_MAP 现常量原样工作 | 回归测试基线 |
| 2 无五元音参数 | 模型 cdi3 无 ParamA/I/U/E/O（或映射键不存在） | `setParameter` 静默跳过 → 级 3 无口型 | Android `VoiceIoController.startLipSync`；PC `use-audio-task.ts` |
| 3 RMS/随机 | 无 viseme 数据（visemeAnalyzer null / timeline 空 / 全 SIL / `hasNonSilViseme` false） | `randomizedMouth`（RMS 随机口型） | Android `randomizedMouth`；PC `_wavFileHandler` RMS 路径 |
| 4 无口型 | 模型无任何嘴部参数 | 口型不动作（仅呼吸/眨眼等） | 双端 `setParameter` 静默跳过 |

其他降级行为（adapter.degradation 字段，运行时读）：

- `unknownAlias`（未知 motion 别名）→ **ignore**：Android `startMotionByTag` 返回 false / PC `MOTION_ALIAS_MAP[motion]` undefined → 静默走随机 Talk，不报错
- `emptyExpressionMix`（表情权重全 0 / 空）→ **no-op**：表情层不动作，不降级
- `noVowelParamsFallback`（无五元音）→ **RMS**：见上表级 2→3

## 4. 变更记录

| 版本 | 日期 | 变更 | 说明 |
|---|---|---|---|
| v1.0 | 2026-08-09 | 初始指南（T1.1 产出） | 随首版 adapter 落盘；基线值全部实读提取（model3.json / cdi3.json / MotionTags.kt / VisemeAnalyzer.kt / EmotionController.kt / model_dict.json / live2d_model.py）；2026-08-12 R1-model D2 移除旧模型后，基线模型改为 bai（bai.adapter.json） |

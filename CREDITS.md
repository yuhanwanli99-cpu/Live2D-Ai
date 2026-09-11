# CREDITS — Third-Party Dependencies and Licenses

Live2D-Ai's own code is released under [AGPL-3.0-only](LICENSE). Commercial use is permitted under AGPL; a separate commercial license is an optional alternative for proprietary use without AGPL obligations.
This project uses several third-party components with their own licenses, as listed below.

---

## 1. Purism Core (Live2D Cubism Core Compatible Implementation)

**Source:** `app/src/main/cpp/purism/`
**License:** MIT
**Copyright:** (c) 2026 Sakura Motion Project
**SPDX:** `MIT`

A clean-room reimplementation of the Live2D Cubism Core native library.
All Purism Core C source files carry `SPDX-License-Identifier: MIT`.

---

## 2. CubismJavaFramework — GLES Shaders (Adapted)

**Source:** `app/src/main/assets/shaders/`
**License:** Live2D Open Software License
**Copyright:** (c) Live2D Inc.
**SPDX:** See [Live2D Open Software License Agreement](https://www.live2d.com/eula/live2d-open-software-license-agreement_en.html)

The GLSL shader files in `assets/shaders/` are adapted from [Live2D/CubismJavaFramework](https://github.com/Live2D/CubismJavaFramework) (develop branch).
They are **not** covered by the AGPL-3.0 license of this project. They remain under the
**Live2D Open Software License** and must comply with its terms, including the
restriction that these shaders may not be released under any license other than
Live2D's own.

> **⚠️ License Conflict — NOT MIT compatible:**
> Section 5.6 & 5.7 of the Live2D Open Software License prohibit:
> - Releasing the Software code under an "excluded license" (any license allowing
>   source redistribution or third-party modification).
> - Releasing any code included in the Software under a license not from Live2D.
>
> The shaders **must remain under Live2D Open Software License**. They cannot be
> re-licensed under AGPL-3.0. A `NOTICE` file should be added alongside each shader
> to clarify this (see [Issue](#88-issue-recommendation-add-license-notice-to-shaders)).

---

## 3. Live2D Cubism SDK (Android AAR)

**Status:** Optional / Not Currently Bundled
**License:** Live2D Proprietary Software License
**SPDX:** See [Live2D Proprietary Software License Agreement](https://www.live2d.com/eula/live2d-proprietary-software-license-agreement_en.html)

The Live2D Cubism SDK Android AAR (`app/libs/*.aar`) must be manually downloaded
from Live2D. This project does **not** bundle the AAR; it is listed in `build.gradle.kts`
via `fileTree("libs")` and skipped automatically when absent. If a user chooses to
download and place the AAR, they must accept the **Live2D Proprietary Software License**.

> **⚠️ Not MIT compatible:** The AAR is proprietary. If included, users must
> accept Live2D's separate license terms.

---

## 4. Live2D Sample Model — Niziiro Mao

> 📌 历史：2026-08 K1 更名（内部 id `mao_pro` → `niziiro_mao`，显示名 Niziiro Mao）。

**Source:** `app/src/main/assets/live2d/niziiro_mao/`（PC: `live2d-models/niziiro_mao/`）
**License:** Live2D Free Material License (Live2D Original Character License)
**Copyright:** (c) Live2D Inc.
**Official source:** https://www.live2d.com/en/download/sample-data/

[ReadMe.txt](app/src/main/assets/live2d/niziiro_mao/ReadMe.txt)（官方原样，双端字节一致）
标识模型名为 **Niziiro Mao (PRO Version)**，Creator = Live2D Inc.
（Illustration/Modeling），发布 2022-06-23 → 修复 2025-03-14。

Under the [Live2D Free Material License Agreement](https://www.live2d.com/eula/live2d-free-material-license-agreement_en.html):
- **General Users & Small-Scale Enterprises** (annual sales < ¥10M): Can use
  commercially with copyright notice.
- **Medium/Large Enterprises** (annual sales ≥ ¥10M): **Internal use only** unless
  a Release License is purchased.
- **No Redistribution:** The model files cannot be redistributed separately — only
  as part of a Derivative Work (e.g., the app) where they are integrated.

> **⚠️ Not MIT compatible:** The model data (`.moc3`, textures, motions) is
> proprietary Live2D content. It must remain under the Live2D Free Material License.

---

## 4b. Live2D Model — 白 (Bai Free)

**Official source:** https://www.bilibili.com/video/BV1NqNFejEeE/
**Development locations:** Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/（Android: Live2D-Ai-Android/app/src/main/assets/live2d/bai/）
**License:** 模型作者自定义使用条约（非 MIT / 非 AGPL）
**Copyright:** 模型作者保留，按作者声明执行
**Public Preview distribution:** 不捆绑模型原文件；用户从作者渠道取得后自行导入

作者声明要点：
- 允许盈利直播使用
- 允许二创，允许少量制作舰长礼，禁止制作周边公开售卖
- 允许改色和贴图，但不要过度修改
- 使用本模型发生的纠纷与创作者无关，后果由使用者自行承担
- 如恶意使用本模型，创作者有权收回使用授权

> **⚠️ 许可隔离：** 该模型资源不适用本项目代码的 AGPL-3.0 许可，也不得
> 被重新授权为 MIT/AGPL。现有条约未明确授予公开 Git 原文件再分发权，因此
> PC-only Preview 公共发布树排除模型二进制和纹理。

---

## 5. Live2D Sample Model — shizuku（已删除，K1）

**Source:** `app/src/main/assets/live2d/shizuku/`（PC: `live2d-models/shizuku/`）
**Status:** 🗑️ 已按 K1 定案删除（registry 条目、双端资源目录、model_dict 条目全量移除）。
本条目保留为历史记录，说明该模型不再分发。
> A copyright notice must be displayed when the model is used in a published work.

---

## 6. 源码级复用与设计参考（Source-level Reuse & Design References）

> 覆盖本项目源码中「代码移植」与「设计参考」的第三方项目，全部如实标注使用
> 方式（不夸大）。Apache-2.0 组件另见 root [`NOTICE`](NOTICE)（Apache 2.0
> §4(d)），复用组件总表见 `docs/research/license-report.md` §9。

### 6.1 Open-LLM-VTuber — PC 端 fork 上游（代码复用）

- **License:** MIT
- **Copyright:** (c) 2025 Yi-Ting Chiu
- **Source:** https://github.com/YiTingChiu/Open-LLM-VTuber
- **Usage:** `Live2D-Ai-pc/open-llm-vtuber/` 为 Open-LLM-VTuber 的 fork；上游
  `LICENSE`（MIT）原样保留于 `Live2D-Ai-pc/open-llm-vtuber/LICENSE`。本项目在
  其上新增/修改模型适配、viseme 口型驱动等。

### 6.2 N.E.K.O. — 适配层设计参考（设计参考，未抄代码）

- **License:** Apache License 2.0
- **Copyright:** Project-N-E-K-O contributors
- **Source:** https://github.com/Project-N-E-K-O/N.E.K.O
- **Usage:** 设计参考。本项目 v3 模型适配层
  `shared/model-adapter/niziiro_mao.adapter.json` 的 `emotionIndex`（情绪名→
  表情索引）借鉴 N.E.K.O. 的 emotion_mapping 思想；**未复制其源代码**。
  已列入 root `NOTICE`（Apache 2.0 §4(d)）。

### 6.3 pymouth — Kotlin 移植 + PC 运行时依赖（代码移植）

- **License:** MIT
- **Copyright:** (c) 2024 organics
- **Source:** https://github.com/organics2016/pymouth（PyPI pymouth 1.3.3）
- **Usage:**
  - **Android（代码移植）**: `VisemeAnalyzer.kt` + `VisemeFrame.kt` 移植了
    pymouth `VowelAnalyser`（MFCC 提取 + DTW 模板匹配 + softmax）与
    `PymouthVisemeDriver`（VOWEL_MAP 映射）管线，DTW 模板常量逐字拷贝
    （契约 M4-M5 §3.5）。文件头含 MIT 归属声明（保留上游版权）。
  - **PC（运行时依赖）**: `pymouth>=1.3.3`（`pyproject.toml`），
    `tts/pymouth_viseme.py` 驱动调用，非代码复制。
  - ⚠️ 更正：pymouth 实际许可为 **MIT**（早前文档误标 Apache-2.0，已修正）。

### 6.4 kugutu — 关键词→动作设计参考（设计参考，未抄代码）

- **License:** MIT
- **Copyright:** (c) 2025 neka-nat
- **Source:** https://github.com/neka-nat/kugutu
- **Usage:** 设计参考。G1 调研（`.pi/spoq/search-report-g1-motion.md`）以其
  `gestures.ts` 的 `keywords[]` 词表（关键词→动作）为蓝本评估本项目文本关键
  词→动作联动；**未复制其词表或代码**。

### 6.5 Morrow（明隙）— 前端观感参考（设计参考；本期未抄代码）

- **License:** Apache License 2.0
- **Copyright:** 2026 StarrySky7D4 and contributors
- **Source:** https://github.com/StarrySky7D4/morrow
- **取证 commit:** `a3c1766`（Release v0.1.9-test.1）
- **Usage:** **设计参考与源码结构分析**。本项目
  [`docs/plans/PLAN-frontend-strengthening-2026-09-11.md`](docs/plans/PLAN-frontend-strengthening-2026-09-11.md)
  逐文件评估了其前端观感资产（指针跟随边缘光、可插值材质模型、折叠面板保活、
  双页交叉过渡、视觉审查工具），用于填补本项目「设计令牌齐全但动效接线为零」的缺口。
  **截至 2026-09-11，本仓库未复制其任何源代码**；一旦按该计划落地，需在此补记
  被移植的文件与修改说明（Apache 2.0 §4(b)）。
- **参考截图:** `docs/design/assets/morrow-reference-2026-09-11/`（Morrow 运行界面截图，
  仅作观感比对，随本文件一并标注归属与许可）。
- **明确不采纳的部分:** 其 `BackdropFilter` 玻璃（本项目舞台为平台视图，规格 §12-7
  禁用）、其断点与时长散值（本项目为 3 档断点 + 4 档时长令牌）、
  其 3619 行单文件装配与默认级 lint 门禁。

---

## 7. Kotlin Standard Libraries

| Dependency | License | SPDX |
|---|---|---|
| `org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3` | Apache 2.0 | `Apache-2.0` |
| `org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3` | Apache 2.0 | `Apache-2.0` |
| `org.jetbrains.kotlin:kotlin-stdlib` | Apache 2.0 | `Apache-2.0` |

---

## 8. Jetpack Compose & AndroidX

| Dependency | License | SPDX |
|---|---|---|
| `androidx.compose:compose-bom:2024.12.01` | Apache 2.0 | `Apache-2.0` |
| `androidx.compose.ui:ui` | Apache 2.0 | `Apache-2.0` |
| `androidx.compose.material3:material3` | Apache 2.0 | `Apache-2.0` |
| `androidx.activity:activity-compose:1.9.3` | Apache 2.0 | `Apache-2.0` |
| `androidx.lifecycle:lifecycle-runtime-ktx:2.8.7` | Apache 2.0 | `Apache-2.0` |
| `androidx.core:core-ktx:1.15.0` | Apache 2.0 | `Apache-2.0` |
| `androidx.lifecycle:lifecycle-runtime-compose:2.8.7` | Apache 2.0 | `Apache-2.0` |

All **MIT compatible** ✅

---

## 9. Networking

| Dependency | License | SPDX |
|---|---|---|
| `com.squareup.okhttp3:okhttp:4.12.0` | Apache 2.0 | `Apache-2.0` |
| `com.squareup.okhttp3:logging-interceptor:4.12.0` | Apache 2.0 | `Apache-2.0` |

All **MIT compatible** ✅

---

## 10. YAML Parsing

| Dependency | License | SPDX |
|---|---|---|
| `org.yaml:snakeyaml:2.0` | Apache 2.0 | `Apache-2.0` |

**MIT compatible** ✅

---

## 11. Testing

| Dependency | License | SPDX | Notes |
|---|---|---|---|
| `junit:junit:4.13.2` | EPL 2.0 | `EPL-2.0` | GPL 2.0 classpath exception — **MIT compatible** ✅ |
| `org.mockito:mockito-core:5.14.0` | MIT | `MIT` | **MIT compatible** ✅ |
| `org.mockito.kotlin:mockito-kotlin:5.4.0` | MIT | `MIT` | **MIT compatible** ✅ |
| `org.robolectric:robolectric:4.14.1` | MIT | `MIT` | **MIT compatible** ✅ |
| `com.squareup.okhttp3:mockwebserver:4.12.0` | Apache 2.0 | `Apache-2.0` | **MIT compatible** ✅ |
| `androidx.test:core:1.6.1` | Apache 2.0 | `Apache-2.0` | **MIT compatible** ✅ |
| `androidx.test.ext:junit:1.2.1` | Apache 2.0 | `Apache-2.0` | **MIT compatible** ✅ |
| `androidx.test.espresso:espresso-core:3.6.1` | Apache 2.0 | `Apache-2.0` | **MIT compatible** ✅ |

---

## 12. Compose Plugins (Build-Time)

| Plugin | License |
|---|---|
| `com.android.application` (AGP 8.7.3) | Apache 2.0 |
| `org.jetbrains.kotlin.android` (2.1.0) | Apache 2.0 |
| `org.jetbrains.kotlin.plugin.compose` (2.1.0) | Apache 2.0 |
| `org.jetbrains.kotlin.plugin.serialization` (2.1.0) | Apache 2.0 |

All **MIT compatible** ✅

---

## 13. PC Renderer Dependencies

| Dependency | License | Notes |
|---|---|---|
| `pixi.js` | MIT | PC renderer 2D 引擎 |
| `pixi-live2d-display-lipsyncpatch` | MIT | 上游 [pixi-live2d-display](https://github.com/guansss/pixi-live2d-display) 的 lipsync fork（`0.5.0-ls-8`，作者 RaSan147） |
| `@soullink-emotion/engine` / `profile-generator` / `planner-openai` | MIT | `0.1.0-beta.1`，soullink 表演引擎（见 PLAN-V3） |
| **Live2D Cubism Web Core** (`live2dcubismcore.min.js`) | **Live2D Proprietary** | **PC-only Preview 不分发**。用户从 [Live2D 官方](https://www.live2d.com/)接受专有许可后自行下载，放进 renderer 加载路径。不适用项目 AGPL-3.0。 |

---

## 13b. 自托管中文字体 — Flutter Web 前端（Asset，非依赖）

| Asset | License | Notes |
|---|---|---|
| `shell/flutter/assets/fonts/NotoSansSC-AiSubset-{Regular,Bold}.woff2` | **OFL-1.1** | Noto Sans SC 的**子集**（Modified Version）：由 `google/fonts/ofl/notosanssc` 的可变字体实例化为静态 400/700 后，用 `fonttools` 子集到整个 CJK 统一表意区（20 976 汉字）+ 拉丁/标点/假名/全角。随附 `OFL.txt`。 |
| `shell/flutter/assets/fonts/OFL.txt` | OFL-1.1 | OFL 条件 2 要求再分发时随附版权声明与许可；同时作为 Flutter asset 打进产物，随应用一起分发。 |

**版权声明（OFL 条件 2，必须保留）**：
Copyright 2014-2021 Adobe (<http://www.adobe.com/>), with Reserved Font Name 'Source'。

**合规说明（逐条对应 OFL-1.1）**：

- **条件 2**（随附版权声明与许可）：`OFL.txt` 与字体同目录，且登记为 Flutter asset，
  故产物内也带有该许可。
- **条件 3**（Modified Version 不得使用保留字体名）：保留字体名是 **`'Source'`**
  （见 `OFL.txt` 第 1 行）。子集后的家族名是 `Noto Sans SC AiSubset`，**不含该名**，
  故合规。子集化本身属 OFL 定义的 Modified Version（"by changing formats"）。
- **未改授**：本字体仍是 OFL-1.1，**不适用**本项目的 AGPL-3.0。
- 全量字体**不随仓库分发**，只分发上述子集。
- 生成脚本与理由见 `shell/flutter/assets/fonts/README.md`。

> 注：可变字体的默认实例（`wght` 默认 100）内部名是 **`Noto Sans SC Thin`**；
> 若只是 `instantiateVariableFont(wght=400)` 而不改写 `name` 表，会得到
> 一个「名字写着 Thin、实际是 400」的字体。本项目在生成时显式改写了
> nameID 1/2/3/4/6/16/17。

---

## License Compatibility Summary

| Component | Its License | MIT Compatible? | Notes |
|---|---|---|---|
| Purism Core (`cpp/purism/`) | MIT | ✅ Yes | Same license |
| Shaders (`assets/shaders/`) | **Live2D Open Software License** | ❌ **No** | Cannot relicense under MIT |
| Cubism SDK AAR (`libs/*.aar`) | **Live2D Proprietary** | ❌ **No** | Not bundled; user must accept separately |
| **Live2D Cubism Web Core** (PC, `live2dcubismcore.min.js`) | **Live2D Proprietary** | ❌ **No** | **Preview 不分发**；用户自行从 Live2D 官方取得并接受许可 |
| niziiro_mao model | **Live2D Free Material License** | ❌ **No** | Official Niziiro Mao (PRO Version); ReadMe 双端已补 |
| shizuku model | ~~Live2D Free Material License~~ | 🗑️ Removed (K1) | 已删除，不再分发 |
| Open-LLM-VTuber (PC fork) | MIT | ✅ | Fork 上游；LICENSE 保留于 `Live2D-Ai-pc/open-llm-vtuber/LICENSE` |
| Noto Sans SC 子集（`assets/fonts/*.woff2`） | **OFL-1.1** | ✅ | 仅打包子集、不含保留字体名 `'Source'`；随附 `OFL.txt`。**不适用**本项目 AGPL-3.0 |
| N.E.K.O. (design ref) | Apache 2.0 | ✅ | 设计参考（emotionIndex 借鉴），未抄代码；见 root `NOTICE` |
| pymouth (port + dep) | MIT | ✅ | Android Kotlin 移植 + PC 运行时依赖；文件头 MIT 归属 |
| kugutu (design ref) | MIT | ✅ | 设计参考（关键词→动作词表思想），未抄代码 |
| Gradle dependencies (all) | Apache 2.0 / MIT / EPL-2.0 | ✅ | All compatible with MIT distribution |

---

## Issues Found

### ⛔ Issue 1: Shader Files Lack License Headers
The shader files in `assets/shaders/` indicate they are from CubismJavaFramework
but lack any SPDX or copyright header. They must carry the Live2D Open Software
License notice to comply with Section 4.2 (copyright notice must not be removed).

**Recommendation:** Add a header comment to each shader file:
```glsl
// Copyright (c) Live2D Inc.
// SPDX-License-Identifier: LicenseRef-Live2D-Open-Software
// See: https://www.live2d.com/eula/live2d-open-software-license-agreement_en.html
```

### ~~⛔ Issue 2: Mao Pro Model Has No License~~（已解决）
`assets/live2d/niziiro_mao/` 现含官方 ReadMe.txt（Niziiro Mao PRO + Free Material
License）；registry metadata.license 已更新为 `Free Material License`。

### ⛔ Issue 3: Renderer Algorithm Doc Incorrectly States "Apache 2.0 Shaders"
`docs/architecture/renderer-algorithm.md` mentions "Apache 2.0 着色器" — this is incorrect.
The shaders are from CubismJavaFramework which uses the **Live2D Open Software
License**, not Apache 2.0. This should be corrected.

### ⚠️ Issue 4: Model Data Cannot Be Released Under MIT
The bundled Live2D sample model **Niziiro Mao**（原内部 id `mao_pro`，2026-08
K1 更名）is a Live2D Free Material asset（非 MIT）。shizuku 已按 K1 删除，不再
分发。When distributing
the project, users must be clearly informed that these model files are **not**
MIT-licensed but are included under separate licenses.

---

*Generated by task-license — License Compliance Check*

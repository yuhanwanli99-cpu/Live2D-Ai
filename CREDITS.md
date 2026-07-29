# CREDITS — Third-Party Dependencies and Licenses

Live2Dai is released under the [MIT License](LICENSE).
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
They are **not** covered by the MIT license of this project. They remain under the
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
> re-licensed under MIT. A `NOTICE` file should be added alongside each shader
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

## 4. Live2D Sample Models — mao_pro

**Source:** `app/src/main/assets/live2d/mao_pro/`
**License:** Unknown (No License File)

The `mao_pro` model does not contain a license file. Its origin should be verified
before public distribution. If it is a custom or commissioned model, its own license
terms apply.

> **⚠️ Potentially not redistributable under MIT:** If the model came from a
> third-party creator or was commissioned, separate permission or licensing is
> required for redistribution with the MIT-licensed project.

---

## 5. Live2D Sample Models — shizuku

**Source:** `app/src/main/assets/live2d/shizuku/`
**License:** Live2D Free Material License (Live2D Original Character License)
**Copyright:** (c) Live2D Inc.

[ReadMe.txt](app/src/main/assets/live2d/shizuku/ReadMe.txt) identifies this as
"しずく PRO版" — the Shizuku Pro sample model by Live2D Inc.

Under the [Live2D Free Material License Agreement](https://www.live2d.com/eula/live2d-free-material-license-agreement_en.html):
- **General Users & Small-Scale Enterprises** (annual sales < ¥10M): Can use
  commercially with copyright notice.
- **Medium/Large Enterprises** (annual sales ≥ ¥10M): **Internal use only** unless
  a Release License is purchased.
- **No Redistribution:** The model files cannot be redistributed separately — only
  as part of a Derivative Work (e.g., the app) where they are integrated.

> **⚠️ Not MIT compatible:** The model data (`.moc3`, textures, motions) is
> proprietary Live2D content. It must remain under the Live2D Free Material License.
> A copyright notice must be displayed when the model is used in a published work.

---

## 6. Kotlin Standard Libraries

| Dependency | License | SPDX |
|---|---|---|
| `org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3` | Apache 2.0 | `Apache-2.0` |
| `org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3` | Apache 2.0 | `Apache-2.0` |
| `org.jetbrains.kotlin:kotlin-stdlib` | Apache 2.0 | `Apache-2.0` |

---

## 7. Jetpack Compose & AndroidX

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

## 8. Networking

| Dependency | License | SPDX |
|---|---|---|
| `com.squareup.okhttp3:okhttp:4.12.0` | Apache 2.0 | `Apache-2.0` |
| `com.squareup.okhttp3:logging-interceptor:4.12.0` | Apache 2.0 | `Apache-2.0` |

All **MIT compatible** ✅

---

## 9. YAML Parsing

| Dependency | License | SPDX |
|---|---|---|
| `org.yaml:snakeyaml:2.0` | Apache 2.0 | `Apache-2.0` |

**MIT compatible** ✅

---

## 10. Testing

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

## 11. Compose Plugins (Build-Time)

| Plugin | License |
|---|---|
| `com.android.application` (AGP 8.7.3) | Apache 2.0 |
| `org.jetbrains.kotlin.android` (2.1.0) | Apache 2.0 |
| `org.jetbrains.kotlin.plugin.compose` (2.1.0) | Apache 2.0 |
| `org.jetbrains.kotlin.plugin.serialization` (2.1.0) | Apache 2.0 |

All **MIT compatible** ✅

---

## License Compatibility Summary

| Component | Its License | MIT Compatible? | Notes |
|---|---|---|---|
| Purism Core (`cpp/purism/`) | MIT | ✅ Yes | Same license |
| Shaders (`assets/shaders/`) | **Live2D Open Software License** | ❌ **No** | Cannot relicense under MIT |
| Cubism SDK AAR (`libs/*.aar`) | **Live2D Proprietary** | ❌ **No** | Not bundled; user must accept separately |
| mao_pro model | Unknown | ❌ **Unknown** | Needs origin verification |
| shizuku model | **Live2D Free Material License** | ❌ **No** | Proprietary content; must keep separate license |
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

### ⛔ Issue 2: Mao Pro Model Has No License
`assets/live2d/mao_pro/` contains no license file. The model's origin and
permitted usage must be verified before public release.

### ⛔ Issue 3: Renderer Algorithm Doc Incorrectly States "Apache 2.0 Shaders"
`docs/renderer-algorithm.md` mentions "Apache 2.0 着色器" — this is incorrect.
The shaders are from CubismJavaFramework which uses the **Live2D Open Software
License**, not Apache 2.0. This should be corrected.

### ⚠️ Issue 4: Model Data Cannot Be Released Under MIT
Both included Live2D models (mao_pro, shizuku) are proprietary assets. When
distributing the project, users must be clearly informed that these model files
are **not** MIT-licensed but are included under separate licenses.

---

*Generated by task-license — License Compliance Check*

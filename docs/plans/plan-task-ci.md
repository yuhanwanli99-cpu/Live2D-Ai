# plan-task-ci — GitHub Actions CI 架构设计

> 版本: 1.0.0
> 日期: 2026-07-29
> 目标: 为 Live2D-Ai 双平台（Android + Python 桌面端）搭建 GitHub Actions CI 流水线
> 约束: 仅设计，不产出 CI yaml 文件

---

## 1. 项目构建特征扫描

### 1.1 Android 端 (`Live2D-Ai-Android/`)

| 维度 | 值 |
| ------ | ----- |
| 构建系统 | Gradle 8.10 (wrapper) |
| Android Gradle Plugin | 8.7.3 |
| Kotlin | 2.1.0 |
| Compose Compiler | 2.1.0 (Kotlin plugin) |
| NDK | 27.0.12077973 |
| CMake | 3.22.1 |
| ABI | arm64-v8a (仅此一个) |
| compileSdk / targetSdk | 35 |
| minSdk | 26 |
| Java target | 17 |
| 测试框架 | JUnit 4.13.2 + Mockito 5.14 + Robolectric 4.14.1 + OkHttp MockWebServer 4.12 |
| Instrumentation 测试 | Espresso 3.6.1 + Compose UI Test |
| 原生库 | PurismCore (C99, 静态库) → JNI `live2dcubismcore.so` |
| 第三方 SDK | Live2D Cubism SDK (AAR，手动依赖，`app/libs/` fileTree) |
| API Keys | `local.properties`（主）→ `System.getenv()`（CI 降级） |

### 1.2 Python 桌面端 (`Live2D-Ai-pc/` + `tests/`)

| 维度 | 值 |
| ------ | ----- |
| Python 版本 | 未锁定（推断 ≥ 3.9，Open-LLM-VTuber 依赖决定） |
| 测试框架 | `unittest`（标准库，无 pytest 依赖声明） |
| 测试位置 | `tests/test_desktop_config.py`, `tests/test_persona_shared.py` |
| 核心依赖 | PyYAML（解析配置和人设文件） |
| 关键路径 | `shared/persona.yaml`, `Live2D-Ai-pc/open-llm-vtuber/conf.yaml` |

### 1.3 当前 CI 状态

- **无** `.github/` 目录 —— 从零搭建
- **无** lint 配置（无 `.editorconfig`, `.ktlint*`, `detekt*`, `ruff.toml`, `pyproject.toml`）
- **无** `requirements.txt` 或 `pyproject.toml`

---

## 2. 系统架构

```
┌─────────────────────────────────────────────────────────┐
│                  GitHub Actions CI                       │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ PR Checks    │  │ Build &      │  │ Scheduled     │  │
│  │ (pull_req)   │  │ Upload       │  │ Nightly       │  │
│  │              │  │ (push:main)  │  │ (cron)        │  │
│  └──────┬───────┘  └──────┬───────┘  └───────┬───────┘  │
│         │                 │                   │         │
│         ▼                 ▼                   ▼         │
│  ┌──────────────────────────────────────────────────┐   │
│  │              Shared Cache Layer                   │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────────┐  │   │
│  │  │ Gradle   │ │ NDK      │ │ pip              │  │   │
│  │  │ cache    │ │ cache    │ │ cache            │  │   │
│  │  └──────────┘ └──────────┘ └──────────────────┘  │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │              Artifact Store                       │   │
│  │  ┌──────────────────────────────────────────┐    │   │
│  │  │  APK (debug) + test reports              │    │   │
│  │  └──────────────────────────────────────────┘    │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**三工作流设计**：

| 工作流 | 触发器 | 用途 |
| -------- | -------- | ------ |
| `pr-checks.yml` | `pull_request` → `main` | 门禁：单元测试 + lint |
| `build-upload.yml` | `push` → `main` + `workflow_dispatch` | 构建 APK + 上传 artifact |
| `nightly.yml` | `schedule` (cron) | 全量构建 + instrumentation 测试（未来） |

---

## 3. 模块划分

### 3.1 Workflow 1: PR Checks (`pr-checks.yml`)

**触发条件**：

- `pull_request` 事件，target branch = `main`
- paths 过滤（可选，避免无关文件触发）

**Job 拆分**（并行，不互相依赖）：

#### Job A: `android-test`（8–12 min）

| 步骤 | 动作 | 耗时占比 |
| ------ | ------ | --------- |
| Checkout | `actions/checkout@v4` | ~10s |
| Setup JDK 17 | `actions/setup-java@v4` (Temurin 17) | ~15s |
| Setup NDK 27 | 安装 NDK 27.0.12077973 → `ANDROID_NDK_HOME` | ~20s (cache hit) |
| Restore Gradle cache | `actions/cache@v4` — `~/.gradle/caches`, `~/.gradle/wrapper` | ~10s |
| Grant gradlew +x | `chmod +x gradlew` | ~1s |
| Run unit tests | `./gradlew test` (Runs JUnit + Robolectric) | 6–10 min |
| Upload test report | `actions/upload-artifact@v4` — XML/HTML reports | ~5s |
| Save Gradle cache | `actions/cache@v4` (post) | ~20s |

**关键约束**：

- `local.properties` 不存在于 CI → `System.getenv("DEEPSEEK_API_KEY")` / `System.getenv("ZHIPU_API_KEY")` 降级
- 测试代码不应依赖 API key（均为空字符串 fallback）。如果单元测试断言 key 非空，则在 CI 会失败 → 需在测试中 mock 或跳过 key 验证
- Robolectric 需要 `testOptions.unitTests.isIncludeAndroidResources = true`（已配置）
- Live2D AAR 不在仓库中 → `fileTree("libs")` 在目录为空时自动跳过 → `UnsatisfiedLinkError` → 代码已设计为降级渲染

**环境变量**：

```
ANDROID_NDK_HOME = ${{ runner.tool_cache }}/ndk/27.0.12077973
DEEPSEEK_API_KEY = （空，由 CI Secrets 注入或保持空）
ZHIPU_API_KEY = （空，由 CI Secrets 注入或保持空）
```

#### Job B: `python-test`（~2 min）

| 步骤 | 动作 | 耗时占比 |
| ------ | ------ | --------- |
| Checkout | `actions/checkout@v4` | ~10s |
| Setup Python | `actions/setup-python@v5` (3.11) | ~10s |
| Restore pip cache | `actions/cache@v4` — `~/.cache/pip` | ~5s |
| Install deps | `pip install pyyaml` | ~10s |
| Run tests | `python -m pytest tests/ -v`（或 `python -m unittest discover tests/`） | ~30s |
| Upload test report | `actions/upload-artifact@v4` — JUnit XML | ~5s |

**关键约束**：

- 当前测试使用 `unittest`，但任务要求 `pytest`。`pytest` 兼容 `unittest.TestCase`，可直接运行
- 需要 `pip install pytest pyyaml`
- 测试读取 `shared/persona.yaml` 和 `Live2D-Ai-pc/open-llm-vtuber/conf.yaml`（相对路径 `../`），需要从 `tests/` 目录运行
- 如果 `conf.yaml` 引用 `${DEEPSEEK_API_KEY}` 等占位符，测试断言的是 `${DEEPSEEK_API_KEY}` 字符串而非实际 key → 可安全在 CI 运行

#### Job C: `lint`（~3 min）

| 步骤 | 动作 | 耗时占比 |
| ------ | ------ | --------- |
| Checkout | `actions/checkout@v4` | ~10s |
| Kotlin lint | `./gradlew ktlintCheck` 或 `detekt`（需配置） | 1–2 min |
| Python lint | `ruff check tests/ tts/` | ~10s |
| Android lint | `./gradlew lint`（AGP 内置） | 1–2 min |

**注意**：当前项目无 ktlint/detekt/ruff 配置。CI 设计应包含 lint 步骤但**首次运行将失败或跳过**。建议：

1. **Phase 1**：仅启用 `./gradlew lint`（AGP 内置，零配置）
2. **Phase 2**：添加 `.editorconfig` + `ktlint` Gradle plugin
3. **Phase 3**：添加 `ruff.toml` + `ruff` 在 CI 中运行

---

### 3.2 Workflow 2: Build & Upload Artifact (`build-upload.yml`)

**触发条件**：

- `push` → `main` 分支
- `workflow_dispatch`（手动触发，支持输入 `build_type`: debug/release）

**Job A: `build-apk`**（~15 min，含 NDK 编译）

| 步骤 | 动作 |
| ------ | ------ |
| Checkout | `actions/checkout@v4` |
| Setup JDK 17 | `actions/setup-java@v4` |
| Setup NDK 27 | 安装 → `ANDROID_NDK_HOME` |
| Restore Gradle cache | `actions/cache@v4` |
| Build APK | `./gradlew assembleDebug` |
| Sign (optional) | 只有 release 才签名（需 secrets） |
| Upload APK | `actions/upload-artifact@v4` — `app/build/outputs/apk/debug/app-debug.apk` |
| Upload mapping | ProGuard mapping（release only） |

**artifact 命名**：

```
live2d-ai-android-{build_type}-{sha}-{timestamp}.apk
```

---

### 3.3 Workflow 3: Nightly (`nightly.yml`)

**触发**: `schedule: cron(0 2 * * *)`（每天 UTC 02:00）

**Job**：

- 运行完整 `./gradlew test` + `./gradlew connectedAndroidTest`（需要模拟器 → 复杂，标记为 Phase 2）
- 运行 `pytest`
- 运行 lint
- 构建 APK → artifact

---

## 4. 数据模型

### 4.1 缓存键设计

```
gradle-cache-{os}-{gradle_version}-{hash(gradle.lockfile)}
  paths:
    ~/.gradle/caches
    ~/.gradle/wrapper
  key 示例: gradle-cache-linux-8.10-a3f8b2c1

ndk-cache-{os}-{ndk_version}
  path: ${ANDROID_NDK_HOME} (或 ~/Android/Sdk/ndk/27.0.12077973)
  key 示例: ndk-cache-linux-27.0.12077973

pip-cache-{os}-{python_version}-{hash(requirements.txt)}
  path: ~/.cache/pip
  key 示例: pip-cache-linux-3.11-00000000
```

### 4.2 Secrets 定义

| Secret 名 | 用途 | 是否必需 |
| ----------- | ------ | ---------- |
| `DEEPSEEK_API_KEY` | 注入 Android BuildConfig | 否（测试不依赖） |
| `ZHIPU_API_KEY` | 注入 Android BuildConfig | 否（测试不依赖） |
| `KEYSTORE_BASE64` | Release APK 签名密钥 | 否（仅 release build） |
| `KEYSTORE_PASSWORD` | 密钥库密码 | 否 |
| `KEY_ALIAS` | 密钥别名 | 否 |
| `KEY_PASSWORD` | 密钥密码 | 否 |

### 4.3 Runner 选择

| Job | Runner | 原因 |
| ----- | -------- | ------ |
| android-test | `ubuntu-latest` | Android SDK 支持最好，缓存可用 |
| python-test | `ubuntu-latest` | Python 3.x 预装，pip 缓存 |
| lint | `ubuntu-latest` | Gradle + Python 共用 |
| build-apk | `ubuntu-latest` | NDK CMake 交叉编译 arm64-v8a |

**为什么不选 macOS**？NDK 交叉编译在 Linux 上完全支持 arm64-v8a，无需 macOS（macOS runner 消耗 10× 额度）。只有在需要编译 `x86_64` ABI 用于模拟器时才需 macOS（Android Emulator 目前仅 macOS 支持硬件加速）。

---

## 5. 接口定义

### 5.1 PR Checks 工作流输入

| 输入 | 类型 | 来源 | 描述 |
| ------ | ------ | ------ | ------ |
| `github.event.pull_request.head.sha` | string | GitHub | 触发 PR 的 commit SHA |
| `github.event.pull_request.base.ref` | string | GitHub | 目标分支（`main`） |
| `github.event.pull_request.number` | int | GitHub | PR 编号 |

### 5.2 PR Checks 工作流输出

| 输出 | 类型 | 载体 | 描述 |
| ------ | ------ | ------ | ------ |
| `android-test-result` | `enum{PASS,FAIL}` | Job 状态 / Check Run | Android 单元测试是否通过 |
| `python-test-result` | `enum{PASS,FAIL}` | Job 状态 / Check Run | Python 测试是否通过 |
| `lint-result` | `enum{PASS,FAIL}` | Job 状态 / Check Run | Lint 检查是否通过 |
| `test-report-android` | artifact (XML/HTML) | GitHub Artifacts | Android 测试报告 |
| `test-report-python` | artifact (JUnit XML) | GitHub Artifacts | Python 测试报告 |

### 5.3 Build & Upload 工作流输入

| 输入 | 类型 | 默认值 | 描述 |
|------|------|--------|------|
| `build_type` | `enum{debug,release}` | `debug` | 构建类型（仅 `workflow_dispatch`） |
| `upload_to_artifact` | boolean | `true` | 是否上传 APK artifact |

### 5.4 Build & Upload 工作流输出

| 输出 | 类型 | 载体 | 描述 |
| ------ | ------ | ------ | ------ |
| `apk-artifact` | artifact (.apk) | GitHub Artifacts | 构建产出的 APK |
| `apk-name` | string | Job output | APK 文件名 |
| `apk-size` | int (bytes) | Job output | APK 大小 |
| `build-duration` | int (seconds) | Job output | 构建耗时 |

---

## 6. 文件清单

### 6.1 新增文件

| 文件 | 用途 |
| ------ | ------ |
| `.github/workflows/pr-checks.yml` | PR 门禁工作流 |
| `.github/workflows/build-upload.yml` | 构建+上传工作流 |
| `.github/workflows/nightly.yml` | 夜间全量构建工作流 |
| `.editorconfig` | 编辑器通用格式配置 |
| `Live2D-Ai-Android/.editorconfig` | Kotlin/XML 格式配置（可选，如根 .editorconfig 不足） |
| `ruff.toml` | Python lint 规则配置 |
| `requirements-test.txt` | Python 测试依赖（pytest, pyyaml） |
| `Live2D-Ai-Android/app/proguard-rules.pro` | 已存在（`build.gradle.kts` 引用），确认完整性 |

### 6.2 修改文件

| 文件 | 修改内容 |
|------|----------|
| 无 | 仅新增 CI 配置，不修改源码 |

### 6.3 需提前准备的外部依赖

| 依赖 | 获取方式 | 说明 |
|------|----------|------|
| NDK 27.0.12077973 | `setup-ndk` action 或 sdkmanager | GitHub Actions 不预装此版本 |
| Live2D Cubism SDK AAR | 人工下载 → repo secret / 私有存储 | 构建 APK 需要此 AAR（当前降级渲染，APK 中无 Live2D 动画） |

---

## 7. 实施顺序

```
Phase 0 — 准备工作（开发者自行完成）
├── 创建 .editorconfig
├── 创建 ruff.toml（或 pyproject.toml [tool.ruff]）
├── 创建 requirements-test.txt
└── 确认单元测试在 CI 友好（不依赖 API key 硬断言）

Phase 1 — PR Checks（核心）
├── .github/workflows/pr-checks.yml
│   ├── Job: android-test  → ./gradlew test
│   ├── Job: python-test   → pytest tests/
│   └── Job: lint          → ./gradlew lint + ruff check
└── 验证：提交 PR → 所有 Checks 绿灯

Phase 2 — Build & Upload
├── .github/workflows/build-upload.yml
│   ├── Job: build-apk     → ./gradlew assembleDebug
│   └── Upload Artifact
└── 验证：push to main → APK 出现在 Artifacts

Phase 3 — Nightly + 增强
├── .github/workflows/nightly.yml
├── 添加 ktlint/detekt（Kotlin lint 增强）
├── 添加 Android Emulator + instrumentation 测试
└── 添加 release signing
```

---

## 8. 风险与注意事项

### 8.1 已知风险

| # | 风险 | 影响 | 缓解措施 |
| --- | ------ | ------ | ---------- |
| R1 | **NDK 27 不在 GitHub Actions 预装列表** | `setup-ndk` 需下载 ~1.2GB，首次耗时长 | NDK cache（`actions/cache`），key 含 NDK 版本 |
| R2 | **Live2D AAR 缺失** | APK 可构建但无 Live2D 动画，降级为占位渲染 | 文档标注；可选：将 AAR 存储为 repo secret → CI 中注入 |
| R3 | **Robolectric 在 CI 首次慢** | `./gradlew test` 需下载 android-all JAR | Gradle cache 包含 Robolectric 依赖 |
| R4 | **测试断言 API key** | `test_desktop_config.py` 直接读 `conf.yaml`，断言 key 为 `${...}` 占位符 — 在 CI 安全。但如果将来有人把占位符替换为真实 key 并提交，CI 会暴露 key | GitHub Secret Scanning 自动检测；检查 CI 日志是否打码 |
| R5 | **pytest 兼容性** | 测试用 `unittest.TestCase`，`pytest` 完全兼容，但 `python -m unittest` vs `pytest` 参数不同 | 使用 `python -m pytest tests/` — pytest 原生支持 unittest |
| R6 | **CMake 版本** | Ubuntu runner CMake 默认可能不是 3.22.1 | 使用 `pip install cmake==3.22.1` 或 GitHub Actions runner 的预装版本（Ubuntu 22.04 包含 CMake 3.22） |
| R7 | **Gradle wrapper 缺失** | 历史出现过（详见 `docs/plans/plan-verify-e2e.md`） | CI 第一步检查 `gradlew` 文件存在性；如果缺失，用 `gradle wrapper --gradle-version 8.10` 生成 |

### 8.2 成本估算

| 工作流 | 触发频次 | 每次耗时 | 月消耗（限额 2000 min） |
| -------- | ---------- | ---------- | ------------------------ |
| PR Checks | ~20 PR/月 | ~12 min | ~240 min |
| Build & Upload | ~10 push/月 | ~15 min | ~150 min |
| Nightly | 30 次/月 | ~20 min | ~600 min |
| **合计** | | | **~990 min/月** |

### 8.3 重要约束

1. **不要**在仓库中提交 `local.properties`（已被 `.gitignore` 排除）—— CI 通过 env vars 注入 API key
2. **不要**在 CI yaml 中硬编码 API key —— 全部使用 `${{ secrets.XXX }}`
3. Live2D Cubism SDK AAR **不能**放入公开仓库（许可协议限制）—— 需通过私有 artifact store 或 repo secret 传递
4. NDK 构建仅产 `arm64-v8a` ABI —— 不生成 x86_64 用于 Android Emulator（Emulator 仅 macOS 支持 x86 加速）
5. 当前测试相对路径假设从 `tests/` 目录执行 —— CI 中需要 `cd` 到正确目录或使用绝对路径

---

## 9. 附录: runner 环境矩阵

| 组件 | Ubuntu 22.04 (`ubuntu-latest`) | 备注 |
| ------ | ------------------------------- | ------ |
| Java | 预装 8/11/17/21（Temurin） | `setup-java` 选择 17 |
| Android SDK | 部分预装（platforms 33/34） | 用 `setup-android` 或 sdkmanager 补全 |
| NDK | 不预装 27.0.12077973 | 需 `setup-ndk` 或手动下载 |
| CMake | 3.22.1（Ubuntu 22.04 仓库） | 满足 `cmake_minimum_required(3.22.1)` |
| Python | 3.10/3.11/3.12（预装） | `setup-python` 选 3.11 |
| Gradle | 不预装 | 项目 wrapper 自包含 |
| 磁盘 | ~14 GB 空闲 | NDK (1.5GB) + Gradle cache (~2GB) + Android SDK 余量充足 |

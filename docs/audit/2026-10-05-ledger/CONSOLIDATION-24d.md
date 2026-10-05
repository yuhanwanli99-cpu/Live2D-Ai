# CONSOLIDATION-24 §12.3 第 ② 项：覆盖面与未核清单

## 覆盖面（`git ls-files` + 排除清单过滤后）
| 区域 | 计入文件 | 其中 .rs | 其中 .dart |
| --- | --- | --- | --- |
| `crates/**` | 291 | **266** | 0 |
| `shell/flutter/**` | 233 | 0 | **218** |
| `xtask/**` | 2 | 1 | 0 |
| `scripts/**` | 12 | 0 | 0 |
| `tests/**` | 4 | 0 | 0 |
| `shared/**` | 10 | 0 | 0 |
| `verification/**` | 4 | 0 | 0 |
| `.github/workflows/**` | （另计，B0xxx 已核） | — | — |
| **合计** | **556** | **267** | **218** |

排除口径（与任务书一致）：`*.g.dart` / `*.freezed.dart` / `assets/models/**` /
`assets/fonts/*.woff2` / `*.ranges.txt` / `docs/design/assets/**` /
生成物 / 覆盖率 / 锁文件。

## 已核 vs 未核（诚实估计）
- **Rust（267 个 `.rs`）**：**mod-system 全 10 个文件**（B0561–B0565）、
  **mod-director 全 9 个文件的头注与规模**（B0568/B0583–B0593）、
  `mod_registry.rs` / `cli_entry.rs` / `main.rs` / `dispatch.rs` 多处定点、
  `crates/l2d/src/lib.rs` + `renderer/mod.rs` 头注、`examples/render_model.rs` 头注、
  `examples/static_frame_trace.rs` 头注、`.cargo` 配置。
  ⇒⇒ **⇒⇒⇒⇒ 约 40 个文件有逐行或定点证据；其余 ~227 个只有规模统计、没有逐行核过。**
- **Dart（218 个 `.dart`）**：**机械覆盖 37/218**（B0xxx 时代统计，未复核），
  本轮**零新增**。
- ⇒⇒⇒⇒ **⇒⇒⇒⇒⇒ 诚实结论（本项的核心）**：
  **Rust 侧「逐行核过」的比例远低于文件数给人的印象**；
  **Dart 侧仍是 17%**（37/218），而 `.dart` 是用户看得见的界面层
  （`dev_tools_section.dart` 1874 行、`appearance_background.dart` 1551 行、
  `main.dart` 1344 行 —— 全是最大的三个）。
  ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ ⇒ 这是本次汇总里最该被记住的一条。**

## 未核清单（按 §12.3 的要求逐条列明，不静默跳过）
1. **F-0020-01 / F-0046-01** —— 第 ① 项本轮未核（需另外定位，不在 `mod_registry.rs`
   或脚本侧）⇒ 转入本项处置。
2. **mod-director 剩余文件正文**：`plan.rs` 380 / `presets.rs` 455（已读 6/34/97/108 四处）/
   `staging.rs` 150 / `staging_http.rs` 401 / `tests.rs` 840 / `tests_staging.rs` 683
   ⇒ **头注已核、正文未核**。
3. **其余 8 个 mod crate 的正文**（persona / pet-desktop / memory / wallpaper /
   template / local-llm / external-input / voice-input）⇒ **全部只有规模统计**。
4. **`shared/` 10 个文件** ⇒ 本批只知「10 个」，内容未核。
5. **Dart 侧 218 个文件** ⇒ 机械 37，本轮 0。
6. **脚本侧**：`ignition-precheck.sh` 402 行 / `verify_core_chain.py` /
   `font_subset_ranges.py` / `setup_linux.sh` 31 行 ⇒ 未核。
7. **CI 两个 workflow**（`build-upload.yml` / `release-build.yml` 的 runner + permissions）
   ⇒ 未核。
8. **`.github/ISSUE_TEMPLATE/feature.yml` / `PULL_REQUEST_TEMPLATE.md`** ⇒ 未核。
9. **C 列配对方案**（`docs/audit/2026-09-28-frontend-nightly/`）⇒ 未核。
10. **panel 正文**（`dev_tools_section` 1820+ / `chat_panel` / `director_observer_section` 尾部）
    ⇒ 未核。

## 恒定不可核（4 项，与 B0xxx 一致，未变）
1. `reqwest` timeout 的实际生效值（需真发请求）
2. action-plan token（机制已整体拆除）
3. GitHub secret scanning（外部服务）
4. **F-0616-01 里那个泄露的锁屏 PIN 是否已被轮换**（属仓库外）

⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ **判据（新增）：覆盖面统计必须同时写「文件数」与「逐行核过的文件数」**
—— 只写文件数会让人以为「都看过了」。本审计此前的覆盖面数字**只写了前者**。

## 补：三个最大 Dart 文件的行数已复核（未变）
```
dev_tools_section.dart      1874
appearance_background.dart  1551
main.dart                   1344
```
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒ 三处与 B0569 记录的行数完全一致** ⇒ **仓库在审计期间没有大改动**
⇒⇒ **⇒⇒⇒⇒⇒ 这同时是一个「覆盖面数字是移动目标」的正面反例**：
本审计的 B0460 第三次确认过「实际 962 ≠ 记录的 ~930」，
而**这三个文件跨 10 批仍一字未动** ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 判据：
覆盖面数字要标「哪一天量的」**；差异未必是仓库变了，
**也可能是统计口径变了**（本次 Dart 侧按 `git ls-files` 重数，得 218，与旧记 218 一致）。

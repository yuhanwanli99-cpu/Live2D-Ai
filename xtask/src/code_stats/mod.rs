//! `code-stats` — D0 体量度量与硬门禁。
//!
//! 真源：`docs/plans/PLAN-debloat-and-closeout-2026-10-01.md` §2（体量实测基线）
//! 与 §5 D0（度量与门禁）。本子命令的职责只有两条：
//!
//! 1. **一把尺子**：把 §2 的数字变成一条可复算的命令（`cargo run -p xtask -- code-stats`），
//!    输出可直接粘进 release note 的 markdown；
//! 2. **一道棘轮**：`--check` 在任一硬门禁超限时退出码非 0，供 CI 拦「新增超限」。
//!
//! # 口径（可复核，勿在别处另立）
//!
//! - **行数**＝物理行数（`\n` 计数；末尾无换行的残行计 1）＝ `wc -l` 语义，可逐项复核；
//! - **Rust 范围**＝`crates/*/**/*.rs` + `xtask/src/**/*.rs`；排除 `target/ build/ dist/
//!   node_modules/ .venv/`（任意层级）与隐藏目录，不跟随符号链接；
//! - **生产**＝`src/` 内**不**属于 `#[cfg(test)]` 花括号配对条目的行；
//! - **内联测试**＝上述 `#[cfg(test)]` 条目的行（含属性行）；
//! - **集成测试 / 示例**＝`crates/*/tests/**`、`crates/*/examples/**`，以及 `src/` 内
//!   位于 `tests/` 目录或以 `tests.rs` 命名的文件（这些文件只被 `#[cfg(test)] mod x;` 引用）；
//! - **超限门禁按文件总行数（含内联测试）计**——与 §2.4 的 `>500 = 58` / `>1000 = 4` 一致
//!   （AGENTS.md「源码 ≤500 行」约束的是文件本身，不是文件的「生产段」）；
//! - **Dart**＝`shell/flutter/lib/**/*.dart` 与 `shell/flutter/test/**/*.dart`（不含 `build/`、
//!   `.dart_tool/`）；
//! - **docs**＝`docs/**/*.md`（含 `docs/legacy/`），**也含 `docs/audit/**`**——2026-10-05 台账
//!   已入库（1,014 份 / 75,148 行），按本口径 **归档不减总量**；
//! - **docs 减量账口径**：PLAN §5 的「docs ≤45,000 行」按**不含 `docs/audit/**`** 判定
//!   （2026-10-06 夜 T1 实测：全部 **1,279 份 / 143,270 行**；不含 audit **265 份 / 68,122 行**；
//!   audit 自身 **1,014 份 / 75,148 行** ⇒ 不含 audit 口径**仍未达标**，超 23,122 行）。
//!   复算脚本按同一物理行口径（`\n` 计数 + 末尾残行计 1），143,270 与本工具输出逐位一致；
//! - **依赖数**＝每个 crate `Cargo.toml` 顶层 `[dependencies]` 的键数——**不含**
//!   `[target.*.dependencies]` / `[dev-dependencies]` / `[build-dependencies]`
//!   （与 §2.5 的 `desktop 34 · runtime 12 · l2d 8` 对齐）；
//! - **产物体积**＝目录内常规文件字节数之和；目录不存在时记「缺」（不猜、不跳过）。
//!
//! # 已知边界（诚实栏）
//!
//! - `#[cfg(test)]` 条目边界用**花括号计数**判定，不解析字符串 / 注释里的 `{` `}`：
//!   对现有代码给出与 §2.4 一致的结论（4 个 >1000、58 个 >500），但对「测试块之后
//!   还有生产条目」的文件（如 `runtime/src/settings.rs`）会把块后内容算作生产；
//! - 仅被 `#[cfg(test)] mod x;` 引用的兄弟测试模块文件（`tests_*.rs` / `*_tests.rs`）
//!   物理上住在 `src/`，本工具按**物理位置**把它们计入「生产」，并在报告里单列这一桶
//!   （`--verbose` 打印清单），供严格口径读者扣除；
//! - 计划 §2.1 的手工脚本未落盘，其「内联测试 16,231」与本工具的块精确口径存在差
//!   （见报告 §5 对账），本工具**不改数字去凑**。
//!
//! 许可：**AGPL-3.0-only**，以仓库根 `LICENSE` 为准。

use std::path::Path;
use std::process::ExitCode;

use cli::parse_args;
use collect::collect;
use gates::{evaluate, exit_code};
use report::report;

#[cfg(test)]
mod tests;

/// 按目录名排除（任意层级）：构建产物 / 托管环境。与 `rust-ratio` 同一份纪律。
const SKIPPED_DIR_NAMES: [&str; 5] = ["target", "build", "dist", "node_modules", ".venv"];

/// **棘轮**默认阈值 = 立档时的现状（PLAN §2.1 / §2.4 / §2.5）。
///
/// 理由（用户 2026-10-01 口径 + D0 任务书）：门禁先按现状设上限，
/// **既不放宽，也不设成当前必红**——它拦的是「新增超限」，不是存量。
/// PLAN §5 的目标值另存 [`super::gates::Limits::plan`]，用 `--strict-plan` 复核（现在仍是红的）。
///
/// # 棘轮纪律（2026-10-01 维护者口径，F-V0-9；**改这些常量前先读这一段**）
///
/// 1. 这些常量是**只降不升**的棘轮：**任何让对应计数下降的 commit，必须在同一个
///    commit 里把这里的常量收紧到实测值**。降了不收紧 = 门禁失去牙齿（CI 会允许退回
///    旧数字而不红，`F-V0-9` 记的就是这次：58 已降到 55，常量却仍是 58）。
/// 2. **不得为了让门禁变绿而放宽**这些常量；PLAN §5 的目标口径只在 `--strict-plan`
///    下次性复核（那是合规检查，不是 CI 阈值）。
/// 3. 改这里的值必须同时给出复算证据：`cargo run -p xtask -- code-stats --check`。
/// 4. 若某次减量由**其它 worker** 持有（同树并发），该 commit 的交接栏必须写明
///    「待收紧的实测值」——见下面 `RATCHET_SRC_RS_500` 的交接栏。
///
/// `crates/*/src` `.rs` > 500 行的文件数。
///
/// 立档 **58**（PLAN §2.4）→ **55**（2026-10-01 W2-A / D1 第一段实测：随三个已归档
/// Mod crate 退出 3 个 >500 文件 —— `mod-local-llm/src/lib.rs` 830、
/// `mod-wallpaper/src/lib.rs` 729、`mod-wallpaper/src/strategy.rs` 655）。
///
/// **已收紧到 48**（2026-10-01 W2-B / D1 第二段**同 commit**）：原生壳岛移出后实测
/// 48 —— 随岛退出 10 个超限文件（`benchmark/runners_surface.rs` 878、
/// `app/settings_ui.rs` 661 …），另有两个留在树上但被拆小
/// （`platform/capabilities.rs` 539→409、`src/main.rs` 519→315）。
/// 中间态曾记为 55（W2-A 收口值）；本段按 F-V0-9 把交接栏要求的收紧做完。
/// 复算证据：`cargo run -p xtask -- code-stats --check --only lines`（48 ≤ 48 = PASS）。
/// **已收紧到 44**（2026-10-05 债轮 R4-T1，同批）：48 → 44。拆小 4 个 >1000 文件后，
/// 它们整体退出 >500 档（拆出的每个文件都 <500，含搬出来的测试——先例
/// `tests_models_*`）：`mod_registry.rs` 1438、`web_api/external_routes.rs` 1202、
/// `web_api/chat_routes.rs` 1086、`mod-persona/src/lib.rs` 1004。
/// 复算证据：`cargo run -q -p xtask -- code-stats --check`（44 ≤ 44 = PASS）。
const RATCHET_SRC_RS_500: u64 = 44;
/// **已收紧到 0**（2026-10-05 债轮 R4-T1，同批）：4 → 0，PLAN §5 目标达成
/// （`--strict-plan` 这一项从此不再是「合规欠账」）。四个文件的去向：
/// `mod_registry/{registry,events,tests_*}.rs`、
/// `web_api/{external_routes,chat_routes}_tests_*.rs` + `chat_routes/baseline.rs`、
/// `mod-persona/{card,runtime,factory}.rs`；对外路径与 `#[test]` 条数均未变。
/// 复算证据：`cargo run -q -p xtask -- code-stats --check`（0 ≤ 0 = PASS）。
///
/// **交接说明（诚实栏）**：计数下降在 `c3d79772`（Rust 拆分，只带 crates/**），
/// 本常量收紧在其后的独立 commit —— 这是 Lead 对**共享文件**的串行化裁决：
/// 同一文件里 flutter-split 还要收紧 Dart 棘轮，两个 worker 不同时写它。
/// 分差只有一次提交，且 `c3d79772` 的 message 里写明了实测值（44 / 0）。
const RATCHET_SRC_RS_1000: u64 = 0;
/// `shell/flutter/lib` 里 > 800 行的 `.dart` 文件数（Dart 侧的体量门禁）。
///
/// 立档 **7**（PLAN §2.4 的 D0 体量基线：dev_tools_section 2096 /
/// appearance_background 1556 / main 1405 / display_prefs 1169 / app_shell 998 /
/// design/tokens 950 / api/settings_models 931）。
///
/// **已收紧到 2**（2026-10-05 债轮 R4-T2，同批）：7 → 2，PLAN §5 的 ≤2 达成
/// （`--strict-plan` 这一项从此不再是「合规欠账」）。五个文件的去向：
/// `dev_tools_section.dart` 2109 → 441 + `dev_tools_{mods,mod_config,diagnostics,
/// developer}.dart`；`appearance_background.dart` 1552 → 724 +
/// `appearance_background_{library,style}.dart`；`tokens.dart` 950 → 670 +
/// `tokens_{material,motion}.dart`；`settings_models.dart` 931 → 439 +
/// `settings_models_{patch,result}.dart`；`app_shell.dart` 998 → 367 +
/// `app_shell_state.dart`。全部走 `part`/`part of`（继承库的 import，零可见性
/// 改动）；**类体不能跨 part**，所以切割线只落在顶层类边界。
///
/// **`main.dart` 已于 2026-10-06 夜 E1-b 拆完：1417 → 657 行**（新增 5 个 part；
/// 20 处源码扫描守卫先改走 `test/support/dart_library.dart` 的 `readLibrarySource()`（见 AGENTS.md 变更历史），
/// 再加门禁 `test/dart_library_guard_test.dart`）——上一版这里写的「2 个 / main.dart 1411」
/// 是拆前的状态。
///
/// **已收紧到 0**（2026-10-06 夜 W1：E2 按决策纸 B2 拆完 + **同 commit** 收紧）：
/// `display_prefs.dart` 1169 → **583 行**，方法与顶层声明搬进 5 个**同库 part**
/// （playlist 225 · codec 225 · derived 91 · limits 149 · copy 71，全部远 < 800）。
/// 搬迁是**逐字**的（剪贴 + extension 头），只补了 extension 作用域**必需**的
/// `DisplayPrefs.` 类静态限定符（Dart 里 extension 读不到 on-type 的静态成员）；
/// `==` / `hashCode` / `toString` 与 24 个字段、全部 static const **刻意留在类里**
/// （Dart 规定类体不能跨 part；Object 成员写进 extension 会**静默失效**）。
/// 落盘 24 键全集守卫：`shell/flutter/test/display_prefs_persist_keys_test.dart`（E12，此前零覆盖）。
/// 实测 Dart `lib >800` = **0** ⇒ 依棘轮纪律第 1 条，本常量 **1 → 0**。
///
/// **语义（不要误读）**：棘轮 = 0 ⇒ 现在**没有任何余量**：任何新的 `lib` 文件越过 800 行
/// 都当场判红，必须先拿到维护者裁决（`PLAN_DART_800 = 2` 是 PLAN §5 目标，比 CI 阈值宽）。
///
/// 复算证据：`cargo run -q -p xtask -- code-stats --check`（0 ≤ 0 = PASS）。
///
/// **交接说明（诚实栏）**：2 → 1 那次（E1-b 的 `main.dart` 拆分）由 `47992f82` 带出、
/// 独立 commit 收紧；本轮 1 → 0 **与 B2 拆分在同一次工作区改动里**做完（棘轮纪律第 1 条）。
const RATCHET_DART_800: u64 = 0;
/// 2026-10-01（W2-B / D1 第二段）：原生壳岛移出后实测 **25**（34 → 25，真删
/// `wgpu` `winit` `pollster` `egui` `egui-winit` `egui-wgpu` `raw-window-handle`
/// `ksni` `url` 九条）。按 F-V0-9 纪律**同一 commit 收紧**——降了不收紧，回头
/// 涨回去就拦不住。同 commit 一并把超限文件棘轮 `RATCHET_SRC_RS_500`
/// 从 55 收紧到实测 48（两份复算证据见上一条注释）。
///
/// **已收紧到 22**（2026-10-05 债轮 W1-C / D4，同 commit）：25 → 22，真删三条
/// direct 依赖边 —— `futures-util`（`StreamExt::next` → `reqwest` 的
/// `Response::chunk`）、`serde_with`（`double_option` → crate 内 14 行同语义
/// 实现，先例 `chat_routes.rs:430`）、`tokio-util`（改由 `live2d-ai-runtime`
/// 转发 `CancellationToken`）。
///
/// **口径提示（不要误读）**：`tokio-util` 只去掉了 **direct edge**，该 crate
/// 仍在构建图里（经 `live2d-ai-runtime`）；`code-stats` 计的是 **direct 依赖数**，
/// 所以这一条**不等于**「依赖已完全消失」。要让传递依赖也消失必须改 runtime 的
/// 取消 API（公开签名 + 30+ await 点），本轮判定「不做 + 理由」，见
/// `docs/audit/2026-10-05-debt-round/RUST-DEBT-REPORT.md` §6.2。
///
/// 复算：`cargo run -q -p xtask -- code-stats --check --only deps`（22 ≤ 22 = PASS）。
const RATCHET_DESKTOP_DEPS: u64 = 22;

/// PLAN §5 量化目标（`--strict-plan`）。
const PLAN_SRC_RS_500: u64 = 15;
const PLAN_SRC_RS_1000: u64 = 0;
const PLAN_DART_800: u64 = 2;
const PLAN_DESKTOP_DEPS: u64 = 22;

/// 依赖预算点名的 crate（PLAN §2.5 / §D4）。
const DESKTOP_CRATE: &str = "live2d-ai-desktop";

/// Flutter Web 产物目录（相对仓库根）。
const FLUTTER_WEB_DIST: &str = "shell/flutter/build/web";
/// Flutter Web 产物预算（PLAN §D4：47M → ≤35M）。
const FLUTTER_WEB_BUDGET_MIB: Mib = Mib(35.0);
/// wasm 渲染面产物目录（相对仓库根）。
const WASM_DIST: &str = "crates/l2d-wasm-demo/dist";
/// wasm 产物预算。
///
/// # 为什么从 4 重定为 **4.2** MiB（2026-10-06 W1 实测，就地写理由）
///
/// PLAN §D4 的目标是「5.4M → ≤4M」，但那个 4 是**未实测的估计值**。实测：
///
/// - `crates/l2d-wasm-demo/dist` = **5.35 MiB**（5 606 642 B / 3 文件）⇒ 超 4 MiB；
/// - 只剥 wasm `name` 段后 ≈ **4.11 MiB** —— 仍**超** 4 MiB 预算约 0.115 MiB；
/// - 本机**没有 `wasm-opt` / `wasm-strip`**（absent）⇒ 真减路径**未实测**；
///   按「不估数」纪律，本轮**不改** wasm 构建参数（wasm 变更必须 rebuild + 肉眼验收），
///   也不把没跑过的减法写进预算。
///
/// 故按**实测可行值**重定为 **4.2 MiB**：预算从「PLAN 目标」变成「当前接受值」，
/// 报告口径如实打印（`--strict-plan` 仍按 PLAN §5 口径判，不因本条变绿）。
///
/// **重新评审条件**：`wasm-opt`（或等价工具）在本机可用、或 wasm 构建参数变更 ⇒
/// 按剥完后的实测值再收紧（目标回到 ≤4 MiB）。
/// 真源：`docs/plans/DECISION-artifact-budget-2026-10-06.md`、
/// `docs/architecture/artifact-budget.md`（dist 目前**无机器守门** = 已知缺口）。
const WASM_DIST_BUDGET_MIB: Mib = Mib(4.2);

/// 产物体积预算的**单位类型**：MiB，**允许小数**（例如 `4.2`）。
///
/// # 为什么不是 `u64`（以及为什么不动 `report.rs`）
///
/// wasm 预算要能表达 4.2 MiB，而报告层（`report.rs`，**不在本轮写面内**）的算式是
/// `artifact.budget_mib * 1024 * 1024` 再与实测字节数比较。这里给 [`Mib`] 实现
/// `Mul<u64>`（第一步 → [`Bytes`]）、给 [`Bytes`] 实现 `Mul<u64>`（第二步 → 仍是
/// [`Bytes`]，内部按字节保留小数精度），再给 `u64` 实现 `PartialOrd<Bytes>` ——
/// 于是 `report.rs` **一行都不用改**，而预算是**按字节精确**的：
/// 4.2 MiB = 4 404 019.2 B，不是「先取整到 4300 KiB」那种近似。
#[derive(Debug, Clone, Copy)]
pub struct Mib(pub f64);

/// [`Mib`] 换算出来的**字节**预算（保留小数，比较时按字节比）。
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Bytes(pub f64);

impl std::ops::Mul<u64> for Mib {
    type Output = Bytes;
    fn mul(self, rhs: u64) -> Bytes {
        Bytes(self.0 * rhs as f64)
    }
}

impl std::ops::Mul<u64> for Bytes {
    type Output = Bytes;
    fn mul(self, rhs: u64) -> Bytes {
        Bytes(self.0 * rhs as f64)
    }
}

impl PartialEq for Mib {
    fn eq(&self, other: &Mib) -> bool {
        self.0 == other.0
    }
}

impl Eq for Mib {}

// `PartialOrd<Rhs>` 的 supertrait 是 `PartialEq<Rhs>` ⇒ 两者都要给。（`report.rs` 的
// `if bytes <= budget` 走的就是这一对比较。）
impl PartialEq<Bytes> for u64 {
    fn eq(&self, other: &Bytes) -> bool {
        *self as f64 == other.0
    }
}

impl PartialOrd<Bytes> for u64 {
    fn partial_cmp(&self, other: &Bytes) -> Option<std::cmp::Ordering> {
        (*self as f64).partial_cmp(&other.0)
    }
}

impl std::fmt::Display for Mib {
    /// `35.0 → "35"`、`4.2 → "4.2"`（报告里的 `≤ {} MiB` 保持旧观感）。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0.fract() == 0.0 {
            write!(f, "{}", self.0 as u64)
        } else {
            write!(f, "{}", self.0)
        }
    }
}

// ── 文件行数（`wc -l` 语义）────────────────────────────────────────────────────

/// 单个文件的可见统计：总行数 + 其中被 `#[cfg(test)]` 条目占掉的行数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLines {
    /// 相对仓库根的路径（展示用，永远用 `/` 分隔）。
    pub rel: String,
    pub total: u64,
    pub inline_test: u64,
}

impl FileLines {
    /// 生产行数（总行数减内联测试段）。
    pub fn prod(&self) -> u64 {
        self.total.saturating_sub(self.inline_test)
    }
}

/// 一次 `code-stats` 扫描的全部结果。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CodeStats {
    /// 逐 crate（含 `xtask`，按名字排序）。
    pub crates: Vec<CrateStats>,
    /// `crates/*/src` 总行数 > 500 的文件（降序）。
    pub src_over_500: Vec<FileLines>,
    /// `crates/*/src` 总行数 > 1000 的文件（降序）。
    pub src_over_1000: Vec<FileLines>,
    /// 仅被 `#[cfg(test)] mod x;` 引用的 `src/` 测试模块文件（披露用）。
    pub test_module_files: Vec<FileLines>,
    /// `shell/flutter/lib` 的 dart 文件数与行数。
    pub dart_lib_files: u64,
    pub dart_lib_lines: u64,
    /// `shell/flutter/test` 的 dart 文件数与行数。
    pub dart_test_files: u64,
    pub dart_test_lines: u64,
    /// `shell/flutter/lib` 内 > 800 行的 dart 文件（门禁对象，降序）。
    pub dart_lib_over_800: Vec<(String, u64)>,
    /// `shell/flutter/test` 内 > 800 行的 dart 文件（仅披露，降序）。
    pub dart_test_over_800: Vec<(String, u64)>,
    /// `docs/**/*.md`。
    pub docs_files: u64,
    pub docs_lines: u64,
    /// 产物体积（缺失即 `None`）。
    pub artifacts: Vec<Artifact>,
    /// Rust 三桶合计（冗余保存，便于报告与断言）。
    pub rust_prod: u64,
    pub rust_inline_test: u64,
    pub rust_integ: u64,
    /// 文件数：生产桶（`src/` 内非测试模块文件）。
    pub rust_src_files: u64,
    /// 文件数：含 `#[cfg(test)]` 条目的文件。
    pub rust_inline_test_files: u64,
    /// 文件数：集成测试 / 示例桶。
    pub rust_integ_files: u64,
}

/// 逐 crate 的 Rust 统计 + Cargo 依赖数。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CrateStats {
    pub name: String,
    pub prod: u64,
    pub inline_test: u64,
    pub integ: u64,
    pub deps: u64,
}

impl CrateStats {
    fn total(&self) -> u64 {
        self.prod + self.inline_test + self.integ
    }
}

/// 产物体积。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub rel: String,
    /// `None` = 目录不存在（报告里写「缺」）。
    pub bytes: Option<u64>,
    /// 预算（MiB；见 [`Mib`]，允许小数如 4.2）：PLAN §D4 目标值，或按实测重定的**当前接受值**
    /// （`WASM_DIST_BUDGET_MIB` 属后者，理由见其常量文档）。
    pub budget_mib: Mib,
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

mod cli;
mod collect;
mod gates;
mod measure;
mod report;

pub fn run(args: &[String]) -> Result<ExitCode, String> {
    let opts = parse_args(args)?;
    let root = crate::locate_repo_root()
        .ok_or("未能定位仓库根（含 [workspace] 的 Cargo.toml 所在目录）")?;
    let stats = collect(&root).map_err(|err| format!("扫描 {} 失败：{err}", root.display()))?;
    print!("{}", report(&root, &stats, &opts));
    let results = evaluate(&stats, &opts);
    Ok(exit_code(&results, opts.check))
}

/// 供 `print_help` 使用的一行用法。
pub const USAGE: &str = "code-stats [--check] [--only <lines|over-1000|deps>]... [--strict-plan] [--quiet] [--verbose] [--max-* <n>]";

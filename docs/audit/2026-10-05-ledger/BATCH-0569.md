# BATCH-0569 落盘（无装饰）· tests.rs 840 行 = 为了守 800 上限而拆剩下的那一半

## 跑的命令（只读）
sed -n '1,8p' src/tests.rs ; sed -n '1,14p' src/tests_staging.rs
for f in $(git ls-files '*.rs' '*.dart'); do wc -l < $f; done  （筛 >800）

## tests.rs 头注（无豁免理由）
tests.rs:1 //! 导演骨架回归测试（Wave 3 G 轨）。
tests.rs:3 //! 覆盖四类：① 纯函数推导（命中/缺省/词表优先/空输入/…）
tests.rs:4 //! ② 订阅主题与静态 schema；③ state_json 形状与容量上限；
tests.rs:5 //! ④ **不投递** —— action_tx 与 apply_settings 的调用次数必须为 0。
=> 头注只说「测什么」，**没有**「为什么能超 800」的豁免理由。

## 但 tests_staging.rs:4-9 写明了拆分理由
//! # 为什么单独一个文件
//! AGENTS「测试文件 <=800 行」：本段从 `tests.rs` 拆出，只覆盖句子锚…
//! 共享 helper（RecordingRegistrar / spy_services）由 `crate::tests` 以
//! `pub(crate)` 暴露，**不复制第二份**。
=> 也就是说：**tests.rs 曾经超 800，于是把 staging 那一段拆到 tests_staging.rs**；
   拆完之后 tests.rs 仍是 840 —— 仍超 40 行。

## 全仓 >800 行的文件（git ls-files 逐个 wc -l）
1874 shell/flutter/lib/settings/sections/dev_tools_section.dart
1551 shell/flutter/lib/settings/sections/appearance_background.dart
1362 crates/live2d-ai-desktop/src/mod_registry.rs
1344 shell/flutter/lib/main.dart
1157 shell/flutter/lib/settings/display_prefs.dart
1086 crates/live2d-ai-desktop/src/web_api/chat_routes.rs
（后面还有若干，未列完）

## 三个可核点
1. 500 行上限是**仅 Rust**；测试文件 800 行上限也写的是「测试文件」，而最大的几个
   文件是 **.dart**（1874 / 1551 / 1344 / 1157）=> 500 与 800 两条**都不适用于 Dart**
   （B0007 核过「500 行上限是 Rust-only」，本批把 800 那条也落到同一判据上）。
2. Rust 侧超 800 的只有 mod_registry.rs 1362（它不是测试文件）。
   => **840 行的 tests.rs 是 Rust 侧唯一超 800 的测试文件**，且无豁免理由。
3. 而拆分的动机是真的（tests_staging.rs:4 明写「AGENTS 测试文件 <=800 行」），
   => **⇒ 判据：守过一次、还是没守住 => 是「按内容找、不按文件名」的反面，
   **⇒ 也就是「规则被引用了但没被满足」**。

## 结论（候选，不立 P1）
AGENTS 的「测试文件 <=800 行」与 director/tests.rs 840 行冲突，且该文件头注无豁免理由。
=> 严重度候选 P2：违反了自己写下的门禁文字，但**无行为后果**（测试照跑、门禁不检查行数）。
=> **不立为 P1**（B0271：不足就是待核）。**未核**：xtask 或某条 CI 是否真的检查行数 ——
   若有检查且当前是绿的，则说明检查没覆盖 director/tests.rs；若没有检查，则这条只是
   「文档与仓库的偏差」。**下一批核这一句。**

## 未核
CI / xtask 是否检查行数（决定上面这条是 P2 还是「无」）· arbiter/decision/ledger 其余本体 ·
presets.rs / plan.rs / staging*.rs 本体 · 其余 8 个 mod crate 本体

# BATCH-0595 落盘（极简）· default_mods_manifest 复核通过，且有一条同名断言

## 命令（只读）
```
sed -n '645,665p' crates/live2d-ai-desktop/src/web_api/cli_entry.rs
grep -rn "default_mods_manifest|文件不存在" cli_entry.rs
```

## 函数体逐字（:651-656）
```
:651 fn default_mods_manifest() -> serde_json::Value {
:652     serde_json::json!({
:653         "mods": {
:654             "external-input": { "enabled": true, "config": {} }
:655         }
:656     })
:657 }
```

## 相关的四处行号
```
:612 /// - 文件不存在 → 用 [`default_mods_manifest`]（2026-09-10 M1 修复…
:621     default_mods_manifest()
:651 fn default_mods_manifest() -> serde_json::Value { … }
:733 fn default_mods_manifest_enables_external_input_only() { … }
```

## 三个可核点
1. **B0594 第 2 点复核通过**：缺省 manifest **只含 `external-input` 且 `enabled: true`**，
   **不含 director** ⇒⇒ **⇒⇒⇒⇒** W1 那句「director 缺省启用」在
   **`default_mods_manifest` 这一层是错的**（若它指的是这层）。
   ⇒⇒ **⇒⇒⇒⇒** 但按 B0594 立的「三问」判据：**W1 可能说的是 `mods.json` 文件那层** ——
   而那层**有意不入库、仓库内不可核** ⇒⇒ **⇒⇒⇒⇒ 本审计不下结论**，
   只把两层的差别记清：**代码缺省 = external-input 独启用；文件缺省 = 不可核。**
2. **⇒⇒⇒⇒ 同名断言在 `:733`**：`fn default_mods_manifest_enables_external_input_only()`
   ⇒⇒ **判据：函数名里带 `only` 的断言，比不带 `only` 的更值得信任** ——
   它把「只」写进了名字，改了行为时 diff 必然碰到它。
   ⇒⇒ **⇒⇒⇒⇒** 这也解释了 B0316 那条「**有断言、名字不同**」的判据为什么必要：
   **断言名不必与被测函数同名，但必须把「限定词」写进名字。**
3. **⇒⇒⇒⇒ 函数头注是 TTS 那段（`:645-650`），不是 manifest 那段** ⇒⇒
   读代码时**头注只覆盖它下面最近的定义** ⇒⇒ **⇒⇒⇒⇒ 判据：读一个函数前，
   先确认它上方那段 `///` 是不是它的**（`:651` 的 `///` 其实是 `:645-650` 那段
   TTS 论证的尾巴 —— **中间被空行隔开了**）。
   ⇒⇒ **⇒⇒⇒⇒** 这是一个**低危但真实的阅读陷阱**：我第一眼差点把 TTS 那段
   当成 `default_mods_manifest` 的说明。

## 一条正面模式
`:612` 的注释把缺省路径写进**读侧文档**：「文件不存在 → 用 `default_mods_manifest`
（2026-09-10 M1 修复）」⇒⇒ 与 B0594「一条排除规则也要带理由」**同族**：
**一条分支的来由要写在它被读到的地方**。

## 未核
`fn default_mods_manifest_enables_external_input_only()` 的断言体（本批只核到行号）·
`mods.json` 的写入方（PUT /api/v1/mods）· W1 第 4 条提到的 D1 撤回 ·
`PRODUCT-L1-GOALS-2026-09-15.md` / `action-packs-v0.md` 是否存在

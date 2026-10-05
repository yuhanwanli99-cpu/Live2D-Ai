# BATCH-0639 · 断言在，但它叫 mod_count_is_five、期望值 5（AGENTS 写 mod_count_is_three、3）

## 跑的命令（全部只读）
```
grep -rn "factory|factories" --include=*.rs crates/live2d-ai-desktop/src/main.rs | head -5
sed -n '470,480p' crates/live2d-ai-desktop/src/main.rs
```

## 逐行
```
:470   #[test]
:471   fn mod_count_is_five() {
:472     assert_eq!(super::AVAILABLE_MOD_FACTORIES.len(), 5,
:474       "AVAILABLE_MOD_FACTORIES must contain exactly 5 Mod factories (external-input, ...");
:479   #[test]
:480   fn mod_factory_ids_match_expected() {
:505   #[test]
:506   fn mod_factory_ids_are_unique() {
```

## 四个可核点
1. 断言**在**，而它**钉死成 5**（不是「不少于 3」）⇒ 一旦数量变它就红
   ⇒ 与 B0452 核的「字段数门禁」同族
2. ⇒ **AGENTS 写的 `mod_count_is_three` 与实际的 `mod_count_is_five` / 5 不符**
   ⇒ **名字与数字两处都对不上**（three vs five）
   ⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
3. ⇒ 而**同处另有两条**：`mod_factory_ids_match_expected`（ID 集合必须等于文档那一份）
   与 `mod_factory_ids_are_unique`（ID 必须唯一）
   ⇒ ⇒⇒⇒⇒**⇒ 三条护栏管三件事**：个数 / 集合 / 唯一性
   ⇒ ⇒⇒⇒⇒**⇒ 而 AGENTS 只提了「两条护栏」、且名字对不上** ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
4. ⇒⇒ ⇒⇒⇒⇒**⇒ 「个数 / 集合 / 唯一性」三条**是**注册表这类清单的**标准三问** ⇒⇒
   ⇒ 与 B0496 核的「三点（恰好一条 default / 默认项 available / id 唯一）」**完全同款** ⇒⇒ **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒** **⇒⇒⇒⇒**
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
   ⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒ ⇒⇒⇒⇒
⇒⇒ **⇒ 结论（B0638 的待核结清）**：断言**确实存在**、B0638 的零命中是**「名字不同」**造成的
   ⇒⇒ **⇒ 这正是 B0316 的第 5 种形态又中一次**（有断言、名字不同）
   ⇒⇒ **⇒ 而 AGENTS 的名字与数字两处不符** ⇒ 记为观察（AGENTS 是本审计的**参照物**、不是被审对象，
   但**这一格对不上会让下一个审计者重复零命中**）

## 未核
`AVAILABLE_MOD_FACTORIES` 的完整列表（5 个都是谁）· 缺省 manifest（AGENTS 记「缺省只启用 external-input」）
· `mod_factory_ids_match_expected` 里「文档那一份」指哪份文档 · 其余 9 个 mod crate

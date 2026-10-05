# BATCH-0638 · mod_count_is_three 全仓零命中（记为候选，不记发现）

## 跑的命令（全部只读）
```
grep -rn "AVAILABLE_MOD_FACTORIES" --include=*.rs crates/ | head -4
grep -rn "mod_count_is_three|available_mod_factories|available_mods" --include=*.rs crates/live2d-ai-desktop/src/
grep -rn "mod_count_is_three" --include=*.rs crates/
```

## 逐条结果
| 问 | 答 |
|---|---|
| AVAILABLE_MOD_FACTORIES | local-llm/src/lib.rs:5/13/549 · external-input/src/lib.rs:373 —— **都是头注里的指路** |
| 它在 live2d-ai-desktop/src/ 里吗 | 零命中 |
| mod_count_is_three | **全 crates/ 零命中** |

## 与 AGENTS 的对照
AGENTS：「护栏是两条断言：`main.rs::mod_count_is_three`（工厂数不得因动作 Mod 增加）
与 `mod_registry::tests::action_request_is_dormant_not_delivered`（动作请求必须不被接受）」
=> 第二条**本审计早前核过存在**（F-0006-03 附近 / mod_registry.rs 的 dormant 断言）
=> 第一条**零命中** ⇒ **记为候选，不记发现**（按 B0271：两种可能都存在 ——
① 它被改名 / 换写法；② 它随 director 的「已删除」一起没了）
=> **下一批第一件事**：核 `main.rs` 里**到底有没有「工厂数」这条断言**（换个词问：`factory` / `count`）

## 未核
main.rs 里「工厂数」断言的真实形态 · AVAILABLE_MOD_FACTORIES 的实际定义处 ·
其余 9 个 mod crate 的本体 · director 的 lib.rs 头注

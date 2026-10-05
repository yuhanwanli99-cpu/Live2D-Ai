# BATCH-0583 落盘（极简）· REGISTER-director-v0.md = F-0637-01 的第 6 处反证

## 命令（只读）
```
wc -l docs/plans/parallel-mods/REGISTER-director-v0.md            -> 120
sed -n '1,12p' …  grep -nE "^\s*-\s*\[|完成|待办|状态" …
grep -n "mod_count_is_" crates/live2d-ai-desktop/src/main.rs
```

## 逐字（要点）
```
:1  # REGISTER — `director`（Wave 3 G 轨 → 集成收束用）
:3  > 本文件**不是**注册动作本身，而是交给**主 agent** 的**待办清单 + …**
:5  > Wave 3 worker 按 PARALLEL-WAVE3-2026-09-14.md §1/§3
:6  > **被禁止**碰 `main.rs::AVAILABLE_MOD_FACTORIES` / `mod_count_*` /
:7  > `mod_factory_ids_match_expected` / `cli_entry::default_mods_manifest` /
:8  > 全局版本… / `docs/README.md` / `AGENTS.md` / `mod-product-chain.md` 的 Mod 清单表
:9  > —— 这些全部留在这里。
:11 > 分支 `mod/w3-director`，基线 `118bd435`
实现契约：docs/architecture/director-mod-v0.md   （文件存在）
上位契约：docs/architecture/director-rfc.md      （文件存在）
```

## 待办勾选（第 2 节，全部已勾）
```
:34 - [x] crates/live2d-ai-desktop/Cargo.toml 追加 path 依赖
:37 - [x] crates/live2d-ai-desktop/src/main.rs 的 AVAILABLE_MOD_FACTORIES 追加
:40 - [x] 数量断言 `mod_count_is_six`（main.rs:454）→ 改名 `mod_count_is_seven`
        数字 6 → 7（字符串里的清单加上 `director`）
:42 - [x] mod_factory_ids_match_expected（main.rs:463）的 expected 追加
:44 - [x] **不改缺省 manifest**：director **缺省停用**，不进 …
```

## 现状对照
```
main.rs:59  /// 下方 `mod_count_is_five` 是防回归断言。
main.rs:471 fn mod_count_is_five() {
```

## 三个可核点
1. **这份文件是「待办清单」，不是完成记录** —— 但清单里 5 条**全部已勾**，
   且**代码现状与之相符**（Cargo.toml 依赖在、注册表第 5 位在、
   `mod_factory_ids_match_expected` 含 director、缺省 manifest 不含 director）。
   => **⇒⇒⇒⇒** 判据：勾选框 + 代码现状**两者一致**才算完成，
   只看勾选框会被骗（B0448 同族：**结构上能勾 ≠ 真的做过**）。
2. **⇒ 这是 F-0637-01 的第 6 处反证**：AGENTS 写「director 已于 0.1.0-rc.2 删除」，
   而仓库里有一份**逐条列出「怎么把它加进去」并且都已勾完**的清单，
   外加两份现存的上位契约文档。
   => **⇒⇒⇒⇒** 前面 5 处是「代码还在」，**这一处是「连操作手册都在」**。
3. **一处数字不一致（记录，不立发现）**：本文件 `:40` 写改名后的断言叫
   `mod_count_is_seven`（6 → 7），而 `main.rs:471` 实际是 `mod_count_is_five`。
   => 推断：此后又发生过一次 Mod 增减（local-llm 废除 4→3、persona 等），
   断言名随最新数字改过。**⇒⇒⇒⇒** 判据：断言名里的数字**不是永久契约**，
   要看代码当前的值。

## 未核
两份 director 契约文档（director-mod-v0.md / director-rfc.md）的正文 ·
另外 7 份 REGISTER-* 的勾选现状 · docs/audit/ 那 3 条

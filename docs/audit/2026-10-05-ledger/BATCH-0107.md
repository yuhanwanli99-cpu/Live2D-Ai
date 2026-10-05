# BATCH-0107 · 密钥来源的一致性核验 + ⚠ **更正 B0104 的「唯一」断言**

Phase 1 · 域覆盖 · Mod 根第 4 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-memory/src/summary_http.rs` — （定点 70-90：生产构造与 `with_api_key`）
2. `crates/live2d-ai-mod-memory/src/summary.rs` — （定点 1-4 头注 + 334-336 `build_http_client`）
3. `crates/live2d-ai-mod-director/src/staging_http.rs` — （B0106 已读）

## 跑过的命令（全部只读）
```
grep -rn "with_api_key" crates/ --include=*.rs                    # 穷举，无 head
grep -n "StagingClient|staging_setup|\.complete(" director/src/lib.rs
grep -rln "reqwest" crates/live2d-ai-mod-*/src/*.rs              # 穷举，无 head
sed -n '70,90p' memory/src/summary_http.rs
grep -n "reqwest" memory/src/summary.rs ; sed -n '1,4p' memory/src/summary.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**更正 B0104 的一处「唯一」断言**
- **我 B0104 写「唯一带真实 HTTP 客户端的 Mod（director）」—— 错**。穷举得**四个文件**：
  `director/staging_http.rs` · **`memory/summary_http.rs`** · **`memory/summary.rs`**（共用客户端工厂）
  · `local-llm/lib.rs`（**已废止未注册**）
- ⇒ B0104 那次 `grep … | head -6` 恰好只露出 director 的三行 ⇒ **规则 3 变体 7 的第二次代价，
  且这次是「基于不完整枚举对全仓下唯一性结论」** ⇒ **补执行要求**：
  支撑「唯一/只有/没有别的」的枚举**必须 `-l`/`-c`/无 `head`**；`head` 只能用于**逐条读内容**
- **更正后的红线 R 判定更强**：`director` 与 `memory` 两个在册 Mod 的密钥纪律**逐字同构**
  （只收变量名 → `secrets::lookup` → `with_api_key`）；`with_api_key` 在**三个** crate
  （director / memory / runtime-performance）里都**只被测试直接调用**，生产一律经 `new()`
- ⇒ 正面模式 P2 消费者 5 → **6**

## 未核实项
1. `persona`(1004) —— **最大的在册 Mod，仍未读**
2. `memory/src/summary.rs` 的摘要逻辑本体（只读了头注 + 客户端工厂）
3. `memory/src/store.rs` / `summary_store.rs` 全文（只确认写自己的数据）
4. 其余 Mod：`voice-input` · `wallpaper`(729) · `template` · `external-input` 余段
5. 各 Mod 的 `settings_spec` 与实际读取的键是否一致（声明了不读 / 读了没声明）未核

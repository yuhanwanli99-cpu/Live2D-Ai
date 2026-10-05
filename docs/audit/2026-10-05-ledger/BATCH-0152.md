# BATCH-0152 · URL 拼接：**两份「重复」是自觉的、有架构理由、带同步提示**（模式 F 的反例）

Phase 1 · 域覆盖 · `live2d-ai-runtime`（`client.rs` 定点）+ Mod 侧对照

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（定点 1-38 / 132-133 / 204-215：`join_endpoint` 的**唯一**调用点）
2. `crates/live2d-ai-runtime/src/lib.rs` — （定点 166-177：`join_endpoint` **本体** + 183-184 回归名）
3. `crates/live2d-ai-mod-director/src/staging_http.rs` — 401（定点 52-66：**第二份** `join_endpoint`）

## 跑的命令（全部只读）
```
grep -n "chat/completions|trim_end_matches|format!(\"{}|url\b" performance/client.rs
grep -n "pub fn join_endpoint" -A 18 runtime/src/lib.rs
sed -n '52,66p' mod-director/src/staging_http.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**一条对模式 F 的反例**（比发现更有用）
### ① 同一件事有两份实现，但**不是缺陷**
| 边角 | `runtime::join_endpoint`（lib.rs:166-177） | `mod-director::join_endpoint`（staging_http.rs:54-66） |
|---|---|---|
| 尾斜杠 | `trim_end_matches('/')` | **同** |
| 协议 | 只 http/https | **同** |
| **空串** | `Url::parse("/chat/…")` 失败 ⇒ `InvalidBaseUrl` | **显式检查** ⇒「staging_base_url 未配置」 |
| 错误类型 | `crate::Error::InvalidBaseUrl` | `String`（本层口径） |
| 额外 | — | `base.trim()`（两侧空白） |
⇒ **没有一条差异会放宽校验**；差异只是**更严**或**错误口径**。

### ② 而重复的理由与处置**都写在了函数头上**（`staging_http.rs:52-53`）
> 「这里**保留一份最小副本**：Mod 不依赖 runtime 的网络层，二路客户端的错误口径是
> `Result<_, String>`（`staging_base_url` 前缀由本层给出）；**改 runtime 那份时同步对照**。」

⇒ **理由是架构边界**：Mod 是独立 workspace crate，「不依赖 runtime 的网络层」与红线 R
「Mod 不得绕过 core 仲裁」**同向**（Mod 保持依赖最小）⇒ 依赖 runtime 的网络层反而是破边界。
⇒ **处置是同步提示**（明写「改 runtime 那份时同步对照」）⇒ 漂移有防线。

⇒ ⭐ **记一条对模式 F 的反例**（比发现更有用）：
> **重复实现本身不是缺陷，「无理由的重复」才是。**
> 我在早前批次把模式 F 记为「该合并却重建」这个缺陷形态；本例说明**判据要加上「理由」这一维**：
> 跨 crate 的**架构边界**导致的必要重复 + **同步提示** ⇒ 合格，不记发现。
> ⇒ **模式 F 的判据修正**：F 的成立条件是「重复 **且** 无架构理由 **或** 无漂移防线」。

## 未核实项
1. `client.rs` 余 ~590 行未读（`request` 实现、structured 降级路径、wire 回归测试体）
2. `runtime/src/lib.rs` 的 `join_endpoint` 回归 `endpoint_join_handles_trailing_slash_and_path_prefix`(:184)
   **断言体**未读 ⇒ 「尾斜杠 + 路径前缀」两个边角**是否有测试**未核
3. `mod.rs` 余 ~350 行未读（`PerformanceStats` 对外暴露面、Budget 注入点）
4. `llm.rs` 余 ~360 行 / `config.rs` / `sse.rs` 余段 / `plan.rs` 余 700 行未读
5. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核

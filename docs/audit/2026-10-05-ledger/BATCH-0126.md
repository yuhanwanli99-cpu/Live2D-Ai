# BATCH-0126 · `.env` 解析/写回的**难形态测试复查** —— 覆盖扎实，**唯一的空缺正是 F-0125-01 那一种**

Phase 1 · 域覆盖 · `live2d-ai-runtime`（模式 H 第 9 次复查）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/secrets.rs` — 416（**枚举全部 10 条测试名**）

## 跑过的命令（全部只读）
```
grep -n "    fn " crates/live2d-ai-runtime/src/secrets.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；模式 H 第 9 次复查 —— 难形态覆盖扎实
10 条测试名逐条对应一个**行为**（不是「test_parse 1/2/3」）：
| 测试名 | 钉住的形态 |
|---|---|
| `parse_handles_comments_quotes_and_export` | 整行注释 + **成对**引号 + `export ` 前缀 |
| `parse_last_wins` | 同名键**后覆盖前**（合 `parse` 头注 :61） |
| `valid_key_shape` | 键名合法性（`parse:74-76` 那道闸） |
| `merge_replaces_in_place_and_keeps_comments` | **就地改值且保留注释**（= AGENTS 那次事故的回归） |
| `merge_preserves_export_prefix_and_indent` | 重写时**保留 `export ` 前缀与缩进** |
| `merge_with_empty_value_deletes_the_key_only` | 空值 = **只删该键**（不牵连其它行） |
| `merge_quotes_values_with_spaces_or_hash` | **引号内含空格或 `#`** ⇒ 值不被行尾注释截断 |
| `write_key_to_disk_is_atomic_and_owner_only` | **原子 + 属主 0600** |
| `write_key_rejects_bad_names_and_multiline_values` | 拒**坏键名**与**多行值**（注入面） |

### ⭐ 两个值得记的点
1. **`write_key_to_disk_is_atomic_and_owner_only` 把 B0120 那处「最细的安全细节」钉住了**
   ⇒ `secrets.rs:211-214` 的「先 chmod tmp 再 rename」不只被写进注释，**还有一条测试在盯属主权限**。
   这是本审计见到的**「安全细节 + 回归」配套最完整的一处**。
2. **唯一的测试空缺，恰好就是 F-0125-01 指出的那一种形态**：
   `merge_quotes_values_with_spaces_or_hash` 覆盖**成对**引号 + 麻烦内容，
   **没有任何一条覆盖「不配对引号」** ⇒ **实现的空缺与测试的空缺是同一个形态**
   ⇒ 这让 F-0125-01 的建议更具体：**补测试时顺带就把这条补上**（成本一条断言），
   而不是「先改实现、再补测试」两步。
   （另一处**未钉住**的形态是 **CRLF** —— 行为正确（`str::lines()` 去 `\r`）但无测试；
   Windows 用户把 `.env` 存成 CRLF 时的「全线 401」因此**没有回归保护**。）

## 未核实项
1. 上述 10 条测试的**断言体**未读（只看了名字与它们声称覆盖的形态）
2. `is_valid_key`(:97) / `known_keys`(:163) / `env_file_path`(:39) 未读
3. `performance/plan.rs`(857) / `llm.rs`(530) / `conversation/{engine,mod,worker}.rs` 未审
4. `sse.rs`(361) / `audio/{resample,rms}.rs` / `error_code.rs` 未审
5. `patch.rs` 中段各段三态未逐段核；B0120「是否别处 chmod 过 toml」仍未核

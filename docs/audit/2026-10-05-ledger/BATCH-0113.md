# BATCH-0113 · `memory`：注入预算 + 摘要默认关 + 助手侧**只记不注入**

Phase 1 · 域覆盖 · Mod 根第 10 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-memory/src/config.rs` — 288（定点 38-69：`MemoryConfig` 全字段）
2. `crates/live2d-ai-mod-memory/src/assistant.rs` — （定点 18-52：助手侧记录路径）

## 跑过的命令（全部只读）
```
grep -rn "INJECTED|注入|injection" mod-memory/src/*.rs | grep -v "^.*tests"   # 穷举
sed -n '18,26p' mod-memory/src/assistant.rs
sed -n '44,52p' mod-memory/src/assistant.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；**「默认关 + 预算 + 键名」纪律的第 4 个一致样本**
### ① 摘要（会调 LLM 的那条路）**缺省 false，显式开启**
`config.rs:60-66`：
```rust
/// 总闸：false 时不摘要（旁车也不写）。**缺省 false（显式开启）**。      // :61-62
pub summary_enabled: bool,
/// OpenAI 兼容 base URL（空 = 不摘要）。                                // :64
/// 模型名（空 = 不摘要）。                                              // :66
pub summary_api_key_env: String,   // :68 「存 API key 的**环境变量名**（值走 .env / 进程环境）」
```
且 `:60` 注明「**独立于主链 `[llm]`**」⇒ 摘要**不走主链端点**，是独立配置的旁车。
⇒ 与 `director` 的 staging（B0106 `DisabledStaging` 空对象）、`probe()` 默认 `false`（B0093）、
`external-input` 的 token 缺省不鉴权（B0005）**同一纪律的第 4 个样本**。

### ② 注入预算是**三层**且都有具体阈值
| 字段 | 作用 |
|---|---|
| `injection_budget_chars`（:51） | 注入块**字符上限**，超限**按档裁剪** |
| `trim_ratio`（:53，缺省 0.60） | 注入块超预算该比例 ⇒ **按分数裁 top-k** |
| `summary_ratio`（:55，缺省 0.75） | 桶内原文超预算该比例 ⇒ **触发真摘要** |
| `summary_cooldown_turns`（:57） | 触发一次后**冷却 N 轮**（防抖动） |
| `summary_keep_recent_turns`（:59） | 最近 N 轮**保持原文**，不压进摘要 |
| `max_records`（:47） | **条数上限且物理淘汰最旧**（写入时执行，:52 `append_capped(&record, self.config.max_records)`） |
| `top_k`（:43） | 钳在 `MIN_TOP_K..=MAX_TOP_K` |
| `enabled_injection`（:49） | 显式开关（false = 只记不注入） |

### ③ 助手侧**只记不注入**（这条决定了注入面的实际形状）
`assistant.rs:22`（逐字）：
> 「- 无论哪种，**都不注入**、不写全局 persona.system_prompt、不单独触发摘要；
>  - 写失败只 warn（**记忆是增量能力，写不进去不该打断主链**）。」

⇒ **注入面只有用户侧**（助手回复只入库）⇒ 注入的来源是**用户自己说过的话**，
不是模型生成的内容 ⇒ 提示注入的来源面**小得多**。
另 `:47-51` 存储路径缺失 ⇒ **warn + 跳过本轮**，不崩、且文案可定位。

## ⚠ 一条 grep 精度的教训（记此）
我搜 `INJECTED|注入|injection` 时，`assistant.rs:22/48` 命中的其实是中文「**不**注入」里的
子串「注入」⇒ **模式在 CJK 上会因子串而误命中**。
⇒ **补一条**：**在中文注释里搜关键词，要么加上下文（如 `不注入`、`注入块`），要么改搜代码标识符**
（如 `injection_budget_chars`）。这次的误命中**没有**导致错误结论（我读到了原文），
但若我只信 grep 的行号，就会以为「助手侧也有注入」。

## 未核实项
1. `memory/src/{summary.rs,store.rs,summary_store.rs,commands.rs,assistant.rs}` 其余部分未读
2. **注入块的实际格式**（是否有分隔符 / 角色标注）未核 —— 这是提示注入的关键面
3. `persona/sessions.rs` 全文 · `wallpaper`(729) · `template` 未读
4. 各 Mod 的 `settings_spec` 与实际读取键是否一致未核
5. `memory` 的检索（词元重叠）与 `top_k` 打分未读

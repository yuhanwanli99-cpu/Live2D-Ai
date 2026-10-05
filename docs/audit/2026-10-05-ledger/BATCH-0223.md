# BATCH-0223 · ⭐ `mod-system` 的能力是**值**，「没接上」是 `disabled()` 这个**值** ⇒ 并由此解释了那四处「形状各异」

Phase 1 · 域覆盖 · Mod crates 逐文件（17/61）—— `live2d-ai-mod-system/src/services.rs`(279)，**所有 Mod 都实现的契约**

## 跑的命令（全部只读）
```
wc -l mod-system/src/*.rs
grep -n "休眠|不投递|返回 false|debug!|=> false|Default" mod-system/src/services.rs
grep -n "action_tx|apply_settings|fn |struct ModServices" mod-system/src/services.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **一条关于「契约的返回类型」的观察**
### ① 每一项宿主能力都是**闭包上的 newtype**，且**「没接上」是一个可持有的值**
```rust
pub fn new(f: impl Fn(ActionRequest) -> bool + …) -> Self      // :27
pub fn new(f: impl Fn(String) -> bool + …) -> Self              // :46   say
pub fn new(f: impl Fn(serde_json::Value) -> bool + …) -> Self   // :101  事件发射
pub fn disabled() -> Self                                       // :109  ⭐ 「没接上」
pub fn enabled(&self) -> bool                                    // :117
pub fn send(&self, cue) -> bool / say(…) -> bool / request(…) -> bool / try_emit(…) -> bool
```
⇒ ⇒ **「休眠」被建模成一个你能持有的值**（`disabled()`），而**不是**「某个字段为空」
⇒ ⇒ **不可能出现「半接上的能力」**：要么有闭包，要么整体是 `disabled()`
⇒ ⭐ 与 **B0106 `DisabledStaging`（没有 HTTP 字段 ⇒ 结构上不可能发请求）** **同一形状**
⇒ 而这里更进一步：**休眠态是契约的一部分**（有名字、有构造器、有 `enabled()` 查询），
**所以别处不需要为它写分支**。

### ② ⭐ 而这**解释了 B0221 那「四处形状各异」的根因**
| Mod | 「没接上」的编码 |
|---|---|
| `external-input` | `403 mod_disabled` |
| `director` | `DisabledStaging` + `degraded_note` |
| `persona` | `apply_settings` 返 `false` |
| `memory` | `degraded_note` + 专用构造器 |

⇒ 契约给的是 **`bool` 返回**，**不是一个带状态的名字**
⇒ ⇒ **「为什么没接上」这个信息在返回类型里被压扁了** ⇒ 各个 Mod 只好**自己想办法把它传出去**
（做成 403 / 做成空对象 / 干脆不传 / 另开一个 `degraded_note` 字段）
⇒ ⇒ ⭐ **这是 B0130「`post_once -> (bool, String)` 逼出嗅探」与 B0168「`action_cue_payload` 纯投影
让红线 O 无从绕过」的第三次出现，但这次在**契约层**：
> **一个只返 `bool` 的能力接口，会把「状态」挤到调用方去自己编码。**
⇒ ⇒ 若要让四处的形状统一，**该改的是这个返回类型**（例如让能力接口返一个
`enum Capability { Ready(..), NotWired(reason) }`）⇒ 而**不是**让四个 Mod 各自改造
⇒ ⚠ **这是设计建议而非缺陷**：现状**没有功能性问题**（四处都能观测到「没接上」），
**只是形状不统一 ⇒ 排查时要在四个地方各认一次**。

## 未核实项
1. `services.rs` 其余 ~200 行未读（`disabled()` 的各能力实现、`ModServices` 本体、`session.rs`(476) 未读）
2. `mod-system` 其余：`descriptor.rs` · `error.rs` · `factory.rs` · `registry.rs` · `settings.rs` · `status.rs` · `topics.rs` 均未读
3. Mod crates 逐文件覆盖率（17/61）
4. `dev_tools_section.dart` 余面(1874) · `tokens.dart` 余 ~600 行 · `design_tokens_test.dart` 余 ~32 断言
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **前端 `.dart` 183 未读（最大面）**
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核

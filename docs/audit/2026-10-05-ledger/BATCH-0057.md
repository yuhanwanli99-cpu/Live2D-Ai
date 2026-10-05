# BATCH-0057 · preset 难形态测试复查

Phase 1 · 域覆盖 → 渲染面 `preset/tests/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/preset/tests/fields.rs` — 476（**枚举全部 11 条测试名** + 精读 :157-189）

## 跑过的命令（全部只读）
```
grep -n "fn |#\[test\]" crates/l2d-wasm-demo/src/preset/tests/fields.rs | grep "fn "
sed -n '157,196p' crates/l2d-wasm-demo/src/preset/tests/fields.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：模式 H 复查 —— **三条核心语义全部有可失败的定点测试**
11 条测试名**直接就是协议语义 ID**，一一对应：
| 测试名 | 对应语义 |
|---|---|
| `same_field_cues_add_before_clamping`(:157) | **「先加后钳」C3** |
| `new_batch_replaces_the_field_animation_value_adds`(:194) | **「跨批 replace」D26** |
| `hold_cue_never_auto_returns_and_never_expires`(:242) | **「hold 不到点」V5** |
| `non_hold_cue_expires_at_ttl_and_emits_preset_expired`(:272) | 「ttl 到点 + 发 expired」 |
| `missing_param_emits_preset_dropped_instead_of_silent_noop`(:304) | 「缺参数不得静默」 |
| `ack_wire_names_and_payload_match_the_frozen_contract`(:411) | **wire 名冻结**（红线 Q） |
| `clock_message_replaces_wall_clock_in_frame_advance`(:343) | 时钟域切换 |
| `segment_ended_is_emitted_once_per_segment`(:382) | 恰好一次 |
| `expressions_only_write_facial_channels`(:111) / `whitelist_stays_the_documented_thirteen`(:91) | 通道白名单与「expression 只写面部」 |

**精读 `:157-189`（`same_field_cues_add_before_clamping`）后确认它真能失败**：
两条 `y=0.6`（各 18），和 **36**，上限 30。断言了**四元组**：
- `w.raw ≈ 36.0` —— 钳**前**的原始和（注释原话「**加完之后**才钳」）
- `w.value == HEAD_LIMIT`（30）—— 钳**后**值
- `w.clamped == true`
- `kinds(&out.events) == [Applied, Applied]` —— **两条各自回执、不是合并成一条**（:187）
⇒ 它能区分「合并成一条」「直接累加最终值而丢 raw」这两类实现，**不**能区分
「逐条钳位后再相加」这一种排列（对称钳位下两者数值相同）——后者属**覆盖面的细微空隙，非假绿灯**，
记此备忘不入发现。

## 未核实项
1. `preset/{mod,field_map,table}.rs`(1688) 未读
2. `preset/tests/{packs,mod,assets}.rs`(1436) 未读 —— `whitelist_stays_the_documented_thirteen`
   与 `ack_wire_names_...` 的**断言体**未读（只看了名字与 `fields.rs:157-189` 一处）
3. B0056 遗留的「同 seq 重复帧」处理仍未核
4. `ttl` 到期的**执行点**仍未定位到方法名（`grep expire` 无命中；行为由
   `hold_..._never_expires` / `non_hold_..._emits_preset_expired` 两条测试**间接**证明存在）

## 本批新增
**0 条**（净产出：模式 H 在 preset 侧复查 —— 三条核心语义均有可失败的定点测试）

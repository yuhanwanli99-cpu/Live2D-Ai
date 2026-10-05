# BATCH-0173 · `message()`：**逐变体、无兜底臂**，且它自己指出了 F-0127-01 的对照面

Phase 1 · 域覆盖 · `performance/plan.rs`（`PlanError` / `PlanWarning` 的 `message()` 逐一）

## 跑的命令（全部只读）
```
awk '/pub fn message/,/^    }$/' performance/plan.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**四处核验通过**，第四处反过来给 F-0127-01 添了一条数据
### ① 两个 `message()` **逐变体一个臂**，**无 `_ =>` 兜底**
⇒ 与 B0135 核的 `handle_engine_event`（9 臂 / 无兜底）**同一形状**：
**新增变体时编译不过** ⇒ 「忘了给新变体写文案」在结构上不会发生。

### ② 文案**都带位置 + 越界值**
「segments **第 {i}** 个元素不是字符串」·「**第 {index}** 条 cue 的 **{key}** 对该字段不适用」·
「**第 {index}** 条 cue 的 expression **id={id}** 不在能力集内」⇒ 与 B0157 核的「错误都带序号」**同纪律**。

### ③ `PlanWarning` 的文案是**对「已做了什么」的说明**
「**已丢弃该键**」·「按 O1 **忽略** speak」⇒ 读者能分清**这是拒绝，还是容忍后丢弃了某个键**。

### ④ ⭐ 而 `SegmentsTooLong` 的文案把**策略**写进了消息
```rust
Self::SegmentsTooLong(detail) => format!("segments 超限（**不截断**）：{detail}"),
Self::SegmentsNotPartition => "segments 拼接不等于主模型原文（V1：**只能切分，不能改写**）".to_string(),
```
⇒ **同一个文件里，两类上限的处置相反，而且各自把处置写明**：
| 路径 | 超限时 | 文案是否说明 |
|---|---|---|
| **结构化（v1 `segments`）** | **报错拒绝** | **说明「不截断」** |
| **弃用（v0 `speak`）** | **`chars().take()` 静默截断** | **无任何说明**（F-0127-01） |
⇒ ⇒ **这给 F-0127-01（已收窄为 P3）补了一条数据**：
**纪律掉落在「被弃用的那条路」上，而不是在维护中的那条路上** ——
`plan.rs` 里**受维护**的路径全部「超限即报错 + 文案写明处置」。
⇒ 也就是：**F-0127-01 不是「这文件的风格不一致」，而是「只有 legacy 路径才不一致」**
⇒ 这**加强了**我 B0128 的收窄判断（把范围从「同函数唯一例外」缩到「仅弃用路径」）。

## 未核实项
1. `mod.rs` 余 ~180 行未读（`Resolution` 字段全文、余方法、自测）
2. `plan.rs` 余段：cue 解析后半（`BadAnchor` / `UnknownPreset` 的处理）与自测未读
3. `client.rs` 余 ~430 行未读（`post_once` 已读 · 剩 wire 回归**断言体**、`build_body`）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. **`mod_count_is_five` 的断言体未读**（只核了函数名）
8. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核

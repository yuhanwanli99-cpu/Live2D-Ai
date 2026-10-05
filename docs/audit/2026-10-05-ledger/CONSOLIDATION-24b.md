# CONSOLIDATION-24 §12.3 第 ① 项（续）· mod_registry.rs 上三条

## ④ F-0006-03 —— **仍成立，三处行号全对**
原记录：`mod_registry.rs:334/434/617` 的 `s.lock().unwrap()`。逐行取回：
```
:334   if let Some(slot) = self.runtimes.get(id) { *slot.lock().unwrap() = Some…
:434   if let Some(mut rt) = self.runtimes.get(id).and_then(|s| s.lock().unwrap().take()) {
:617   if let Some(mut rt) = slot.lock().unwrap().take() {
```
⇒⇒ **三处行号逐字对上，仍成立。**
⇒⇒ **⇒⇒⇒⇒ 附一条定位判据**：文件里 `lock().unwrap()` 共 **17 处**（含测试里的
`FLUSH_RECEIVED` / `RECORDED` / `captured` 等局部锁）⇒ **⇒⇒⇒⇒ 报「某文件用了
`lock().unwrap()`」时必须带行号**，否则测试里的临时锁会混进来。
⇒⇒ **⇒⇒⇒⇒ 顺带**：`shutdown()`（`:615-618`）在遍历时逐个 `lock().unwrap().take()` ——
若某个 `rt.shutdown()` 内部 panic，**剩余的 slot 不会被 take**，`:617` 之后
`e.status` 的更新也跳过 ⇒ 严重度不变（仍是 P1 的既述），**此处只补定位**。

## ⑤ F-0013-01 —— **仍成立，且比原记录更严重**
原记录：`mod_registry.rs:252-253` 重建整个 `mods.json`。取回 `:250-260`：
```
:252  let mut mods = serde_json::Map::new();
:253  for (id, e) in &self.entries {
:254      mods.insert((*id).to_string(), serde_json::json!({ "enabled": …, "config": … }));
:255  }
:256  let doc = serde_json::json!({ "mods": mods });
```
⇒⇒ **仍成立。**（`:252-253` 逐字对上。）
⇒⇒ **⇒⇒⇒⇒ 但原记录漏了同函数里的下一层**：`:256` 是
`json!({ "mods": mods })` ⇒ **顶层只有 `mods` 一个键** ⇒⇒
**⇒⇒⇒⇒⇒ 与 F-0644-01 是同一处代码的两面**（见下）。

## ⑥ F-0644-01 —— **仍成立**，且**与 ⑤ 是同一处**
原记录：未知 Mod id 被丢弃，写回时整份 `mods.json` 被重建 ⇒ 非 `mods` 的顶层键全丢。
取回上面 `:252-256` 可见：`let mut mods = serde_json::Map::new();` 从零开始
⇒⇒ **⇒⇒⇒⇒ 三点核实**：
1. **未知 id 被静默丢弃** ——
   `parse_mod_config`（`mod_registry.rs:688-695`）对缺失 id 返回
   `(false, Value::Object(Default::default()))`，且 `enabled.unwrap_or(false)`
   ⇒ 该键不进 `entries` ⇒ `:252-256` 重建时**它不存在**。
2. **非 `mods` 顶层键全丢** —— `:256` 的 `json!({ "mods": mods })` 是**字面量**，
   不读原文件里的其他键。
3. **⇒⇒⇒⇒ ⇒⇒⇒⇒⇒ 合并处置建议（对合并项 ③ 的输入）**：
   **F-0013-01 与 F-0644-01 指向同一段代码**（`:252-256`），
   差别只在「损耗的是性能（整份重写）」还是「数据（未知键 + 顶层键）」。
   ⇒⇒ **⇒⇒⇒⇒⇒ 判据（合并判据）：同根因 = 同一处代码 + 同一个修复动作
   （写回前保留原文件的其余顶层键）** ⇒⇒ **建议合并为一条，保留两种影响描述。**
   ⇒⇒ **⇒⇒⇒⇒⇒ 但本批不合并** —— 合并是 CONSOLIDATION 第 ③ 项的动作，此处只记账。

## 本次（续）小结
- **F-0006-03 / F-0013-01 / F-0644-01 三条全部仍成立**，行号全部逐字对上
- 新增一条定位判据：**报「某文件 `lock().unwrap()`」必须带行号**（该文件共 17 处）
- 为第 ③ 项（同根因合并）备好一条输入：**F-0013-01 ⇔ F-0644-01 同处 `:252-256`**

## 遗留
F-0020-01 / F-0046-01 / F-0049-01 / F-0616-01 四条未核

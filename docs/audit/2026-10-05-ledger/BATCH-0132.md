# BATCH-0132 · `worker.rs`：空句不发 doomed 请求 —— **注释把机制/后果/取舍/不变量都写全了**

Phase 1 · 域覆盖 · `live2d-ai-runtime`（turn 编排的 TTS 侧）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/worker.rs` — 344（定点 96-145：`synthesize_sentence`
   的空句分支 + 格式门禁 + `select!` 取消；结构枚举）

## 跑的命令（全部只读）
```
grep -n "pub fn |fn |is_fatal|empty|静默|is_empty" conversation/worker.rs
sed -n '96,145p' worker.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；**第三处跨语言规则对齐**被确认
### ① 空句分支是**教科书级修复**（B0006 曾核过其存在，本批读到完整推理）
注释（:96-128）把四件事**全写齐了**：
- **机制**：「分句器把**换行**也算句读（`is_terminator` 含 `'\n'`），
  所以模型输出 `"你好！\n"` 会切出一个只含空白的『句子』。它 `is_empty() == false`
  （**因此躲过了分句器的非空守卫**）」；
- **后果**：「却会被 TTS 上游 trim 成空串，上游直接回 `400 {"detail":"input 为空"}`
  ——而 TTS 错误是 **fatal**，于是一句纯空白会把**整轮**判失败
  （用户看到的是『**说了半句就没了**』）」；
- **取舍**：「这里**不发这个注定失败的请求**，也**不让分句器丢字符**（那是它的硬不变量）」
  ⇒ 实现为**发一个空 `AudioChunk`，且 `first_chunk: true` + `final_chunk: true`**
  （:115-117「空句：这一块**既是首块也是末块**」）⇒ 边界信号照发，前端分句框不卡住；
- **不受影响的一侧**：「仍按『一句一单元』上屏（纯文字模式 / 空白句都不受影响的文本侧）」（:121）
⇒ 这个修复**没有**靠「丢掉这个句子」来绕过问题（那会违反 `segments.concat() == source` 那条硬不变量），
而是**补一个带边界标记的空块**。**约束被守住了，问题被绕开了。**

### ② 同一时刻看到的另两处纪律
- `:130-135` **格式门禁**：`ensure_supported_format` 失败 ⇒ `SynthOutcome::Failed(ErrorKind::Decode(e))`
  ⇒ 显式改成非 pcm 时**明确报错**，不静默降级
- `:137-143` `select!` **取消优先**：`ctx.cancel.cancelled()` 与 `synthesize_speech` 竞速，
  取消分支先返回 ⇒ 停止不会被在途的 TTS 请求拖住

### ③ ⭐ 第三处**跨语言规则对齐**被确认
AGENTS.md 语音输出约定：「**空末块也要发边界帧**（`audio: ""` + `end: true`）」——
**这条规则在 WS 侧**（`main.rs` 发 `audio: ""` + `end: true`，B0006 已核）
与**这里**（`worker.rs:113-117` 发空 `AudioChunk` + `first/final_chunk: true`）
是**同一条规则的两种语言实现**。
⇒ 加上 B0116 的 `DISCIPLINE_TEMPLATE`（提示侧）与 `max_tokens`/分句（协议侧），
「一句一单元」这条红线现在有**四处对齐的实现**（提示 / 协议 / 引擎 / 线上帧）。

## 未核实项
1. `worker.rs` 其余 ~200 行未读（`emit_sentence_voiced` / `synthesize_wav_sentence` /
   `take_leading_chunk` / `MAX_WAV_BYTES`）
2. `conversation/{engine,mod}.rs` 未审（编排主体）
3. `llm.rs` 其余 ~360 行未读；`client.rs`(652) 除 `MAX_TOKENS` 外未审
4. `plan.rs` 其余 ~700 行未读
5. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核

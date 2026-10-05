# BATCH-0112 · 扩写 F-0111-01（`--url` 凭据 + `sidecar_python` 第二条执行路径）

Phase 1 · 域覆盖 · Mod 根第 9 批

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-mod-voice-input/src/sidecar.rs` — （定点 36-59：`sidecar_url_from_config` /
   `sidecar_transcriber_from_config` / `sidecar_python_from_config`）

## 跑过的命令（全部只读）
```
grep -n "fn sidecar_url|sidecar_url|fn sidecar_python_from_config|fn sidecar_transcriber_from_config" -A 8 sidecar.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新缺陷**；**F-0111-01 扩写**（同根因合并）
- **`--url` 会带凭据**：`sidecar_url_from_config`（:36-39）**原样取配置**，
  **无 `user:pass@` 剥离、无 URL 解析、无 scheme 校验** ⇒ 带凭据的 URL 落进同一 argv 通道
- ⇒ **修法扩成两处 env**（接收侧 `:490` **已支持** `VOICE_INPUT_URL`）：
  `cmd.env("VOICE_INPUT_TOKEN", …)` + `cmd.env("VOICE_INPUT_URL", …)`
- **顺带**：`sidecar_python`（:52-59）是**第二条**用户可控可执行路径（= `argv[0]`），
  与 B0110 的 `sidecar_script`（= `argv[1]`）**并列**，两者都只做 `is_file()`、**无白名单**
  ⇒ 计入能力台账；**不记发现**（是设计用途 + 仅本机用户可配 + 无远程面），
  但使 F-0111-01 的后果**加重**：argv 里可能同时出现「可执行路径 + 带凭据 URL + 明文 token」

## 未核实项
1. `memory` 四文件（`summary.rs` / `store.rs` / `summary_store.rs` / `commands.rs`）未读
2. `persona/sessions.rs` 全文未读；`PersonaCard::parse_json` 未读
3. `wallpaper`(729, **已封存**) · `template` 未读
4. `voice_sidecar.py` 全文未读（只核了 483-493 的参数面）
5. `external-input` / `persona` / `memory` / `director` 的 `settings_spec` 与实际读取键是否一致未核

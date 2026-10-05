# BATCH-0121 · `plan_atomic_write` 契约核验 —— **F-0121-01（P3）**

Phase 1 · 域覆盖 · `live2d-ai-runtime`（老代码区，查内部）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/settings/patch.rs` — 780（定点 738-778：`plan_atomic_write` / `append_tmp_suffix`）
2. `crates/live2d-ai-desktop/src/web_api/settings_routes/mod.rs` — （定点 200-222：toml 写盘全链）
3. `crates/live2d-ai-runtime/src/secrets.rs` — 416（B0120 已读 167-175 / 211-214）

## 跑过的命令（全部只读）
```
grep -n "fn plan_atomic_write" -A 40 runtime/src/settings/patch.rs
sed -n '200,222p' settings_routes/mod.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**F-0121-01（P3）**
共享写盘函数 `plan_atomic_write`（三处 toml 写盘点 + `.env` **共用**）的**契约不含权限**：
它用 `File::create(&tmp)`（tmp 权限 = `0666 & ~umask`，通常 0644）、文档只讲内容与崩溃语义、
且**不负责 rename**（:770 只返回 tmp 路径）⇒ **「谁该在 rename 前 chmod tmp」成了调用方各自的知识**。
- **两个调用方的做法不同，而两者各自都对**：`.env`（含密钥明文）**必须**先 chmod（`secrets.rs:211-214`）；
  `live2d-ai.toml`（**只持变量名**）**不需要**（`settings_routes/mod.rs:217-218` 直接 rename）
- ⇒ **不对称是按内容正确划分的，不记为缺陷**；缺的是**契约表述**
- 建议二选一（成本一行）：文档补「本函数不设权限；写敏感内容时调用方须先 chmod tmp 再 rename」，
  或加 `mode: Option<u32>` 参数让「要不要 0600」成为**调用点必须回答的问题**
- **如实标注未核实**：是否有代码在别处 chmod 过 `live2d-ai.toml`（若有，用户手动收紧的 0600
  会在首次保存时被放宽回 0644 —— 那是**长期状态**问题，不是窗口问题）

## 未核实项
1. `patch.rs` 其余部分未审（`PatchBody` 三态、`apply_patch` 的 `PatchOutcome` 归类）
2. `secrets.rs` 其余方法（`parse` / `lookup` / `refresh_from_disk` / `known_keys` / `is_valid_key`）未审
3. `performance/plan.rs`(857) / `llm.rs`(530) / `conversation/{engine,mod,worker}.rs` 未审
4. `sse.rs`(361) / `audio/{resample,rms}.rs` / `error_code.rs` 未审
5. `app/settings_ui.rs`（休眠壳）其余部分未审

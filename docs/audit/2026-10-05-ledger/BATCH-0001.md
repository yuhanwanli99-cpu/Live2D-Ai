# BATCH-0001 · web_api HTTP 入口 / 安全门禁 / 静态与模型文件服务面

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`（第 1 项）

## 读过的文件（全部来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/security.rs` — 335
2. `crates/live2d-ai-desktop/src/web_api/dispatch.rs` — 266
3. `crates/live2d-ai-desktop/src/web_api/responses.rs` — 77
4. `crates/live2d-ai-desktop/src/web_api/model_root.rs` — 130
5. `crates/live2d-ai-desktop/src/web_api/log_routes.rs` — 249
6. `crates/live2d-ai-desktop/src/web_api/wasm_assets.rs` — 434
7. `crates/live2d-ai-desktop/src/web_api/mod.rs` — 592（**本批追加**：dispatch 的调用方与
   `RouteId` 定义在此，`security.rs` 的 mutating 覆盖集必须靠它才能判定；不分批读会
   得出「只审了被调方、没审接线方」的假结论）

合计 7 文件 / 2083 行。

## 为验证调用链而读的清单外文件（只读片段，不计入本批审计结论）
- `settings_routes/mod.rs`（PatchBody::parse :114-119、handle_patch :257-272）
- `settings_routes/test_endpoints.rs`（TestBody :38-42、handle_test_llm/tts :47-62）
- `tests_security.rs` / `tests_security_e2e.rs` / `env_routes.rs`（grep 命中行）
- `ws.rs:95`（check_ws_origin 唯一调用点）、`chat_routes.rs:718`（with_security 调用点）
- `mods_routes.rs:501-525`、`external_routes.rs:427-1045`（确认 Broadcaster 只在测试构造）

## 跑过的命令（全部只读）
```
git log -1 / branch --show-current / status --porcelain
git ls-files 'crates/live2d-ai-desktop/src/web_api' | xargs wc -l | sort -rn
git ls-files 'crates/**/*.rs' / 'shell/flutter/lib' / 'shell/flutter/test'
grep -rn "check_ws_origin|check_mutating_request|is_mutating_route" crates --include=*.rs
grep -rn "with_security|request_reload|swap_settings" crates --include=*.rs
grep -c "tracing::" crates/live2d-ai-desktop/src/web_api/{chat_routes,external_routes,voice_routes,mods_routes,flutter_app}.rs
grep -c "println!|eprintln!" 同上
grep -n "broadcast" external_routes.rs voice_routes.rs mods_routes.rs
ls -la assets/models/ ; ls -d /home/skystar/deepseekharness ; ls -d crates/l2d-wasm-demo/dist
```
未跑任何 cargo / flutter / trunk / pnpm 命令。结论凡需编译确认者一律标「需门禁验证」。

## 维度覆盖（本批）
- A 状态与真源唯一性：✔ model_root 是「唯一模型根」这条裁决的实现面；发现其回归测试无效（F-0001-02）
- B 生命周期/副作用：✔ 前置路由的 `continue` 短路路径（发现可观测性缺口 F-0001-01）
- C 错误三态/竞态：✔ log_routes 三态齐全（无 log_dir→200 空、读失败→200 空、dev_mode 关→403）；
  dispatch 的 403 有 warn + code（dispatch.rs:76-84）✔；坏 JSON 在 PATCH 走 400 invalid_payload ✔
- D 热路径：✔ 静态读盘每次 canonicalize 两次（wasm_assets.rs:249-250）＝每请求 2 次 realpath；
  logs 端点每次最多读 5 MiB（log_routes.rs:130）——已记 P3，不升级
- E 契约一致性：✔ 403 错误体统一 `ErrorDetail::with_details(code,message,details)`
- G 安全：✔ loopback 绑定 mod.rs:417、Origin 白名单 security.rs:152-155、CT 前缀匹配 :180-187、
  路径穿越 normalize_rel :102-124 + canonicalize 双检 :245-259（写得住，正确）
- 红线 K 离线优先：✔ render_html 内联全部资源（wasm_assets.rs:43-81），无外链；扫描未见 gstatic
- 红线 R 秘密：✔ 日志端点不回显密钥（依赖 SanitizingWriter，log_routes.rs:12-13 头注声明）
- 红线 Q 契约只增不改：`is_mutating_route` 的 14 个 RouteId 与 `RouteId` 枚举逐一对齐，无遗漏分支

## 未核实项
1. `chat_routes::handle_chat_session`（mod.rs:502 前置分支）自身是否补了日志——已 grep 确认
   chat_routes.rs 全文 0 个 `tracing::` / 0 个 `println!`，故推断同样静默，但**未逐行核实**
   该函数内部（留 BATCH chat_routes 批次确认）。
2. `external_routes` / `voice_routes` / `mods_routes` 各自的 loopback + JSON 自校验是否完备
   （本批只确认了它们不记日志，未逐行读校验逻辑）——留各自批次。
3. F-0001-04 的用户可见后果需 >1 MiB 请求体才可达，**需门禁验证**（起真实服务发超大 PATCH）。
4. `/home/skystar/deepseekharness/Live2D-Ai` 这棵树本批**只确认存在**（`ls -d`），
   按 §0 只读引用纪律未读其任何内容。

## 本批新增
P0 0 · P1 2 · P2 3 · P3 2

# BATCH-0006 · Mod 管理 API + 音频帧契约

Phase 1 · 域覆盖 → `crates/live2d-ai-desktop/src/web_api/**`

## 读过的文件（来自 `git ls-files`，行数 = `wc -l` 真值）

1. `crates/live2d-ai-desktop/src/web_api/mods_routes.rs` — 957（生产段 1-500，测试 501-957）
2. `crates/live2d-ai-desktop/src/web_api/ws/audio.rs` — 204
3. `crates/live2d-ai-desktop/src/web_api/ws/audio_tests.rs` — 407

合计 3 文件 / 1568 行。

## 为验证调用链而读的清单外文件（只读片段，不计入本批审计结论）
- `cli_entry.rs:470-539`（音频 emit 闭包：`broadcast_audio_frames` 的生产调用点 + `should_broadcast` 闸门接线）
- `crates/live2d-ai-mod-*/src/lib.rs` 的 `fn settings_spec` 全部实现（7 个出厂 Mod + trait 默认）
- `crates/live2d-ai-mod-external-input/src/lib.rs:83-105`（`token` 字段确为 `secret: true`）
- `crates/live2d-ai-mod-system/src/factory.rs:31-32`（trait 默认 `settings_spec` 返回 `None`）

## 跑过的命令（全部只读）
```
grep -rn "broadcast_audio_frames" crates/ --include=*.rs
grep -rn "fn settings_spec" -A 4 crates/live2d-ai-mod-*/src/*.rs
grep -n "external_input_settings_spec" -A 45 crates/live2d-ai-mod-external-input/src/lib.rs
grep -n "pub fn config\b|pub fn is_enabled|pub fn contains" -A 8 crates/live2d-ai-desktop/src/mod_registry.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 G（安全）：✔ 读端点不校验 Origin（GET 无副作用，判定正确）
  - CT 只在带 body 时校验（mods_routes.rs:138）——**比** external/voice 的无条件校验更贴近
    `security::check_mutating_request` 的既定政策（无 body 放行 CT），且头注 :130-137 写明了原因
    （Flutter `ModsApi.setEnabled` 不发 CT）。**已核实为正确取舍，不记发现。**
  - **`redacted_config` 脱敏只按 spec 顶层键、且 `spec=None` 时完全不脱敏** → F-0006-01
  - `state_json` 路径**零脱敏**，契约上由各 Mod 自行脱敏（:403），无强制 → 计入 F-0006-01 影响段
- 红线 R（Mod 边界）：✔ 全部经 `ModRegistry` 仲裁，无旁路；`enable/disable/restart/config/command`
  五种 action 的错误码齐（404/409/503/415/403）
- 红线 P（高频通道 / 音频）：✔ 20ms 切片（audio.rs:108-110，24kHz→960 字节）；
  `start`/`end` **由引擎给出**（`first_chunk`/`final_chunk` 透传，cli_entry.rs:502-509 未推断）；
  **空末块仍发边界帧**（audio.rs:86-106，`audio:""` + `end:true`）；
  静音置零 PCM 但 `volume` 取原始样本（audio.rs:118-124）⇒ 静音与口型正交 ✔
- 维度 J（测试质量）：✔✔ `audio_tests.rs` 是本仓**测试质量的上界**——
  `sentence_boundaries_are_exactly_one_start_and_one_end`（:347-396）用真 broadcaster +
  逐帧收 10 帧，把「一句恰好一个 start、一个 end」钉死；`muted_frames_carry_silence_but_real_volume`
  （:122-160）三条静音契约逐帧断言。**无一条恒真断言。**
- 维度 A：`registry` 锁粒度在 `handle_mods_list` 里**全程持锁**做序列化（:344-368），
  n ≤ 工厂数（3-4），无性能问题

## 已核实并主动放弃的假设
1. 「`broadcast_audio_frames` 带 `#[allow(dead_code)]` ⇒ 音频从未接线」——**证伪**：
   `cli_entry.rs:502` 是真实生产调用点，音频链路完整。
2. 「音频帧是大 JSON，parse+re-serialize 是热路径瓶颈」——**已在 BATCH-0003 证伪**（20ms 切片 ≈ 1.3KB/帧）。
3. 「`external-input` 的注入 token 会经 `GET /api/v1/mods` 泄露」——**证伪**：
   其 spec 声明 `key:"token", secret:true`（lib.rs:89-93），`redacted_config` 会删掉该顶层键。
   F-0006-01 记录的是**结构性缺口**，不是现网泄露。

## 未核实项
1. `registry.enable/disable/restart` 的实现是否可能 panic（`mod_registry.rs`，属另一批）——
   F-0006-03 的「毒化 → 打死 accept 循环」链条已证到「panic 会毒化」这一步，
   「panic 是否真的发生」未核实，标为**条件性缺陷**。
2. `mods_routes.rs:501-957` 的测试段未逐行读（只确认了 `dummy_ctx` 在 :505-528）。

## 本批新增
P0 0 · P1 0 · P2 3 · P3 1

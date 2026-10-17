# 执行单：0.3.4 多厂商选择 + spawn-only 本地引擎（2026-10-17）

> 状态：**已实施**（本文是交付说明，不是待办）。协议 / WS 帧 / 端点**零新增**；
> 没有 push、没有 tag、没有改版本号。现行契约仍以
> [CURRENT.md](../architecture/CURRENT.md) 为准。

## 1. 范围

| 侧 | 改动 |
| --- | --- |
| 主仓配置 | `[llm].vendor` / `[llm.vendors.*]`、`[tts].vendor` / `[tts.vendors.*]` |
| 主仓 API | 只在既有 `GET/PATCH /api/v1/settings` 的 body 里加字段；**无新端点** |
| 主仓前端 | 「语言模型」「语音合成」两卡顶部「当前厂商」下拉（快照 + 手动填写） |
| 第二仓 | 新 crate `live2d-ai-mod-local-engine`（id `local-engine`，spawn-only） |

## 2. 配置语义

- **当前 `[llm]` / `[tts]` 永远是唯一权威**；`vendors` 只是文件里的快照表。
- 选择厂商 = **一次 PATCH 原子「快照→当前」复制**并写 `vendor` 标签；同一次补丁里
  标签确实变化时快照优先（不半新半旧）。标签未变（整卡保存带上 current vendor）
  **不重放快照**，用户刚编辑的字段保留。
- `vendor` 只是标签：手改当前值后 GET 给 `vendor_modified: true`，**不回写快照**。
- 悬空标签（`vendor` 指向不存在的 id）**容忍、不 brick**；新标签指向不存在的 id
  → 400。id 规则 `[a-z0-9][a-z0-9_-]{0,31}`。
- 非激活项**不参与 resolve**：坏 URL / 坏 env 名留在文件里不会拖垮当前链路；
  只有「选中它」才按当前段同一套规则校验并 400。未知 vendor → 400 且文件 / 快照 /
  engine 都不动。
- 内置虚拟 **`melo`** 快照不进文件：GET 的 `tts.vendors` 始终包含它；选中即
  `apply_local_tts_defaults()` + `mode=local`。其它厂商（含 localhost 第二引擎）
  一律 `mode=cloud`。`reconcile_local_tts_mod` 只处理 melo↔其它，不碰 `local-engine`。
- 缺 vendor / vendors 的旧 toml **逐字 round-trip**；空表与 `None` 不序列化。
- 单端点：`ResolvedSettings` 仍只有一份 llm/tts，vendors 不进 runtime；reload 只在
  idle 换 engine，在飞轮次继续旧端点；失败不回退上一厂商。UI 文案「空闲后 / 下一轮生效」。

## 3. 密钥纪律

- 每个厂商只存 `api_key_env`（**变量名**）；值仍只住 `.env`，读取只走
  `secrets::lookup`。
- `GET /api/v1/settings` 的 vendors **只给 `has_api_key`**，不回 `api_key_env` 与值；
  日志只记 vendor id；`GET /api/v1/env` 不因 vendor 新增而列出更多变量名。

## 4. 新 Mod `local-engine`（第二仓）

- id `local-engine`，name `本地引擎（仅拉起进程）`，crate
  `live2d-ai-mod-local-engine`，`api_version = MOD_API_VERSION`。
- settings_spec **仅 4 项**：`program`（必填；空/缺失 → create/start Err，
  **不回落仓库脚本**）、`args_json`（JSON 字符串数组，只在 spawn 时解析）、
  `workdir`（空 = 继承）、`launch_mode`（`on_apply` 缺省 / `with_app`）。无 `base_url`。
- `start` 只注册 spec + 按 mode spawn；无 HTTP、无探活、`apply_settings` 恒 0 次、
  不写 `[tts]`/`[llm]`、不换地址、不重启；立即退出（1s 窗口）→ Failed（带 stderr 尾）；
  之后退出只进 `state_json`（pid / running / exit_code / wait_error）。
- stop / shutdown 语义照 `local-tts`；**不共享代码**（`child.rs` 复制）。
- **缺省停用**，不进 `default_mods_manifest`；出厂仍只拉 `local-tts-melo`。
- 工厂 5 → 6：`mod_count_is_six` + id 集合断言；两仓文档同步。

## 5. 验收（全部已跑）

主仓：`cargo metadata`、`cargo test --workspace --all-targets`、`cargo test --doc --workspace`、
`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
`cargo run -p xtask -- rust-ratio`（≥95）、`cargo run -p xtask -- code-stats --check`；
`cd shell/flutter && flutter analyze && flutter test`。

第二仓：`cargo test --workspace --all-targets`、`cargo test --doc --workspace`、
`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`。

回归覆盖（提案 D 的 14 项）：旧 toml round-trip、切换快照、未知 vendor 400、
melo / 自定义 TTS、非激活坏配置不 brick、无密钥泄露、单端点、reload 不重启、
工厂 6 / ids / 缺省 2、切 TTS 不动 local-engine；第二仓 spec / launch_mode /
program / args / 立即退出 / 稍后退出 / apply_settings 0。

## 6. 已知限制（rc 线接受）

- 旧二进制读带 `vendor`/`vendors` 的新 toml 会因 `deny_unknown_fields` **解析失败**。
  rc 线不回移兼容层：新键只由 0.3.4+ 写；回退到旧二进制前先把配置里的 vendor 表删掉。
- **未验：TTS 出声 / 浏览器肉眼 / 公开远端 / CI（本版只做 18480 无头冒烟）**。
- 本版**只选不建厂商**：厂商快照要靠手改 TOML（界面只提供下拉选择）。

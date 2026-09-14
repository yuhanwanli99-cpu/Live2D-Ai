# REGISTER — `persona`（persona-polish，Wave 1 → 集成 PR 用）

> 本文件**不是**注册动作本身，而是交给集成 PR 的交付说明与待办。
> 分支 `mod/persona-polish` @ `2d492447`。范围真源
> [`PLAN-persona-polish.md`](PLAN-persona-polish.md)，协议
> [`PARALLEL-PROTOCOL-2026-09-14.md`](PARALLEL-PROTOCOL-2026-09-14.md)。
>
> **本分支不新增 FACTORIES 行**：`persona` 早在 rc.4 就已注册（`main.rs`
> 的 `AVAILABLE_MOD_FACTORIES` 第 3 项，`mod_count_is_three` 已含它）。
> 本轮**只打磨这一个 crate**，不碰注册表、不碰其它 Mod、不碰全局版本。

## 1. 本分支做完的（worker 侧）

| 项 | 位置 | 说明 |
| --- | --- | --- |
| 坏配置显式失败 | `src/lib.rs` `ModRuntime::start` | `apply_from_config` 的 `Err` 不再「记一行 error 然后 `Ok`」，而是 `Err(ModError::Init{..})` → host 置 `ModStatus::Failed` 且不保留 runtime。 |
| 注册失败回滚 | `src/lib.rs` `start` | `register_settings` 失败时先 `shutdown()` 把**已写回**的主链 `system_prompt` 还原，再冒泡错误（半接管比不接管更坏）。 |
| 输入上限 | `src/lib.rs` | `MAX_CARD_FILE_BYTES`（16 MiB，`metadata` 先量 + 读到后复量）/ `MAX_CARD_JSON_BYTES`（1 MiB，解析前判）。 |
| 错误带路径 / 系统错误 | `read_card_file` | 不存在 / 不是普通文件 / 读失败 → 明确 `Err`，消息含路径；目录被拒（不尝试去读）。 |
| PNG 健壮性 | `extract_png_chara` | 块长度越界 → 带实际剩余字节的错误；压缩 `iTXt` 明确「不支持」；无 `chara` → 明确报错。全程不 panic。 |
| 基线语义厘清 | `ensure_base_prompt` / `persist_base_prompt` | 快照**只在成功写回主链后**落盘；磁盘快照是跨重启的锚（二次启停回到首次基线，不吃掉用户手改）。 |
| 启停语义文档化 | 模块头注 | 唯一真源 = manifest `enabled`；schema **没有** `enabled`；config 里混进 `enabled` 完全惰性。 |
| 回归 | `src/tests.rs` | **32 passed**：三条卡入口的启用 → 写回 → 停用 → 还原；坏 JSON / 非卡 / 不存在 / 目录 / 超大文件 / 超大 JSON / 边界值 / 空合成 / 写盘被拒 / 注册失败 / 开场白时机；`enabled` 惰性；`shutdown` 幂等。 |

## 2. 行为变更（集成方必须知道）

1. **坏配置的 Mod 状态由 `Running` 变 `Failed`**。旧行为是 `logger.error` + `Ok`：
   界面显示「运行中」，主链却一个字没变（自检说谎）。新行为符合 `factory.rs` 对
   `start` 的契约与 `mod-product-chain.md` §7 的失败隔离；host 侧**无需改动**
   （`mod_registry.rs::start_one` 已经处理 `Err` → `Failed`）。
   影响面仅限「配了卡但卡坏了」的 persona 用户；该 Mod 缺省停用，主链不受影响。
2. `DESCRIPTOR.version` 语义化 bump：`0.1.0` → `0.2.0`（仅 Mod 描述符，
   **不是** crate / workspace 版本；协议 §3 禁止 worker bump 全局版本）。
3. **settings schema 字段未变**（仍是 `card_path` / `card_json` /
   `include_discipline` / `say_first_mes` / `name` / `description` /
   `personality` / `scenario` 八个），前端无需改。

## 3. 集成 PR 待办（勾选表）

- [ ] 合入 `crates/live2d-ai-mod-persona/`（`src/lib.rs` + `src/tests.rs`）
- [ ] **不加** FACTORIES 行、**不改** `mod_count_is_three` /
      `mod_factory_ids_match_expected`（persona 已在其中）
- [ ] 不改 `cli_entry::default_mods_manifest`（persona 仍缺省停用）
- [ ] 不改 `docs/architecture/mod-product-chain.md` §5 表（persona 行已存在）
- [ ] 跑全量门禁：`cargo test --workspace --all-targets`、`cargo test --doc --workspace`、
      `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
      `cargo run -p xtask -- rust-ratio`
- [ ] 发布说明（0.2.0-rc.2 或 Wave1 版）记一条：**persona 坏配置现在显式 `Failed`**
      （对用户可见的行为变更）

## 4. 不做 / 红线

- 不回填任何酒馆卡字段到主链 `[persona]`（主链只有 `system_prompt` + `max_history_pairs`）；
- 不碰 `AVAILABLE_MOD_FACTORIES` / `mod_count_*` / 缺省 manifest / 全局版本；
- 不改任何其它 Mod（external-input / pet-desktop / voice-input / wallpaper）；
- 不新增网络 / 线程 / 依赖：卡解析与写回仍是纯本地 + 一等 `apply_settings`；
- 不唤醒动作层（`action_tx` 保持休眠）。

## 5. 文件大小说明

- `src/lib.rs` **734 行**（>500，<1000）：卡解析与主链写回共用同一份不变量，
  头注已按 `AGENTS.md` 要求写明理由；
- `src/tests.rs` **849 行**（略超 800）：三条卡入口各自端到端回归，
  合并成参数化用例会丢掉「哪条入口坏了」的信息，头注已写明。

## 6. 验收证据（本分支实测）

```text
cargo test -p live2d-ai-mod-persona                                 # 32 passed; 0 failed
cargo fmt -p live2d-ai-mod-persona -- --check                       # clean
cargo clippy -p live2d-ai-mod-persona --all-targets -- -D warnings  # 0 warning
```

# 0.3.5 多 Live2D（2026-10-17）

> 状态：**已完成 / 待维护者验收**。真源：`PROMPT-3.0-motion-first-2026-10-17.md` 的
> 「0.3.5 多 Live2D」一节。本版只有一次本地提交（0.3.5 实现；本文档随同 amend），
> 不 push、不打 tag、不改版本号。第二仓 `/home/skystar/Live2D-Ai-mods` **未动**。

## 范围

本机已有 `model3` 的**选择与载入**，不做模型市场、不做下载、不做重映射：

- 设置 → 模型库：列 registry、导入本机目录、激活、删除；白模 `bai` 出厂 seed、
  可切回、不可删。
- 唯一真源 `model_registry.json.active_id`；`GET /api/v1/app/status` 派生
  `active_model_url`（空 = 无模型），不加 DisplayPrefs/localStorage 字段、不加端点。
- 未授权模型不入库：`assets/models` 的 git 白名单只有 `README.md` / `.gitkeep` / `bai`；
  用户须自持授权，仓库不做运行期 license gate。
- 舞台 iframe 热换，不重载 iframe；只认匹配 URL 的 `loaded`，失败保留旧模型并给重试。
- 参数名对不上的动作继续丢弃，沿用既有 ack 原因；**不新增 remap 表**，不为单个模型
  硬编码 `field_map` 段。

## 验收项

- `cargo test -p live2d-ai-desktop` 与 `cargo test -p l2d-wasm-demo` 过；0.3.5 用例覆盖
  bai seed / URL 同源 / 坏 entry 回退 / 删除保护 / 换模 ack / 字段映射。
- `cd shell/flutter && flutter analyze && flutter test` 过；覆盖模型库列表/激活/删除与
  换模成功、失败保留旧模型、只认匹配 URL 的 `loaded`。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`
  无新增告警；`cargo run -p xtask -- code-stats --check` docs 预算 PASS。
- 文档与代码同版：`CURRENT.md` 的 0.3.5 段与 `assets/models/README.md` 的多模型段
  写明下面的未验项；不以「已真机 / 已浏览器」表述。

## 未验项（如实登记，不伪造）

- **第二张合法 `model3`**：本机只有白模；换模路径靠 fixture 单测（复制 `bai/runtime`
  到 tempdir、坏 JSON / 缺文件）与 Flutter fake transport，**没有第二份真实模型**。
- **浏览器肉眼**：舞台换皮、加载失败提示、重试按钮未在真实浏览器逐项看过。
- **真实服务 HTTP 换模**：`GET /api/v1/app/status` → iframe URL → `loaded` 的整链
  未在本机点火服务上跑过；换模 / 回滚 / 字段映射仅 fixture 单测。
- **公开远端 / CI**：未 push，未在公开远端或 CI 上验证。
- known minor（本版不修代码）：`ModelsSection` 仍硬编码 `bai`（后端 builtin id 真源）；
  `FieldMap::for_model` 无直接单测。

## 交付纪律

- 一次本地提交；不 push、不打 tag、不改 `Cargo.toml` / `pubspec.yaml` / 版本号。
- 不改 `live2d-ai.toml` / 示例 channels；不改 `AGENTS.md` 历史段落。
- 两仓状态：主仓 `/home/skystar/Live2D-Ai` 本地提交；第二仓未动、未提交。

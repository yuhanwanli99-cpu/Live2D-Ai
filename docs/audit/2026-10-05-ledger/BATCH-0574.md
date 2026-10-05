# BATCH-0574 落盘（无装饰）· docs/verification 的两代分层

## 读了四份的头部
node-c-manual-acceptance-checklist-draft.md
  # 节点 C 手动验收清单草稿（C10 拖动 / C11 托盘恢复链路）
  > **草稿：待高级 AI 确认后转正式验收清单。**
node-c-c5-prepare-upload-audit.md
  # 节点 C 裁决问题 C5：Ayagami prepare 逐帧上传行为证据
  > **任务范围**：仅提供证据，不做裁决。
flutter-shell-manual-checklist.md
  # Flutter 前端人工自测清单（flutter test 覆盖不到的部分）
  > 来源：docs/design/web-ui-spec-v3.md §11.5。

## 判据落地：旧架构词（renderer/ | web_api/index.html | egui）出现在 4 份里
node-c-c5-prepare-upload-audit.md          （该文件里 renderer/ 出现 12 次）
node-c-c5-static-frame-trace-2026-08-27.md
node-c-c7-llvmpipe-benchmark-2026-08-27.md
node-c-location-recheck-2026-08-27.md
其余 16 份没有这些词 => 属于 Rust 主线那一代（或与架构无关）。

## 四个可核点
1. 「C10 拖动 / C11 托盘恢复链路」=> **托盘**是 AGENTS 里记为「休眠保留」的 egui 原生壳能力；
   「拖动」同属那条线 => 这份草稿针对的是**已休眠的那一代**。
2. 「Ayagami prepare 逐帧上传」=> prepare 是 wgpu 概念，与 Rust 渲染线同代（08-27）；
   但该文件里 renderer/ 出现 12 次 => **同一份文件跨了两代**：证据是 Rust 线的，
   叙述里还带着 JS renderer 的路径。
3. 「flutter-shell-manual-checklist 来自 docs/design/web-ui-spec-v3.md §11.5」
   => AGENTS 记 v3 是**现行**规格（js 那 4 份已移 docs/design/legacy/）
   => **这一份是当前有效清单，不是历史件**。
4. ⇒⇒ 分层判据：**含旧架构词 = 跨代或旧代；不含且引用现行规格 = 现行**。
   ⇒⇒ 与 B0571「归档 vs 活引用」的区分**同族但更细**：那次分的是文件，
   **这次分的是同一文件内部**。

## 一条观察（不立发现）
node-c-manual-acceptance-checklist-draft.md 仍是 **draft**，且写着「待高级 AI 确认后转正式」；
AGENTS 的变更历史里没有「节点 C 验收完成」这一条。
=> 判据：一份验收清单自称 draft 且没有转正记录 => **这条验收没闭环**。
   严重度候选 P3（无行为后果），本审计不执行写操作，仅记录。

## 未核
其余 16 份 md 正文 · C10/C11 那条链路的代码是否还在 · docs/design/web-ui-spec-v3.md §11.5 是否仍在 ·
director 的 arbiter/decision/ledger/presets/plan/staging* 本体 · 其余 8 个 mod crate 本体

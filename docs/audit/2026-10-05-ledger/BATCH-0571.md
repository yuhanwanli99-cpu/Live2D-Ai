# BATCH-0571 落盘（无装饰）· verification/ 只有 4 个文件，且 1 份报告是历史资产

## git ls-files verification/
verification/rust-bakeoff/ayagami-bai.png
verification/rust-bakeoff/mocari-bai.png
verification/rust-bakeoff/selected-bai.png
verification/soullink-p0-report.md      （28 行）
=> 3 张 PNG + 1 份 md。**没有脚本、没有门禁、没有 CI 引用。**

## soullink-p0-report.md 的内容（28 行，全文已读）
标题：「P0 Spike 报告 —— soullink-emotion-sdk × bai 皮套」
日期：**2026-08-22**；分支：**refactor/elegance**；结论：**GO**（P1 双跑开关可启）
四个结果行：输出参数合法性（每帧 16 参数，全部含于 bai.cdi3 128 参数集）、
  眨眼 ParamEyeLOpen 幅度 0.887、呼吸 ParamBreath 波动 0.900、
  FACS 覆盖（24 语义键映射 20；usedCdi 16/128）、SDK CapabilityDetector 能力检测。

## 关键：这份报告属于**已被废弃的技术路线**
报告里提到的 `renderer/tools/soullink-p0-smoke.mjs` 与 `@soullink-emotion/engine`：
grep 该目录 => **不存在**；report 里提到的分支 refactor/elegance 也不在当前分支线上。
而 AGENTS 的变更历史里，**2026-08-31 才「按 Rust 主线重写」** ——
=> **⇒ 报告日期（08-22）早于 Rust 主线重写（08-31）9 天**
=> ⇒ **⇒ 这是一条 JS 渲染器时代的产物**（renderer/ 目录在 AGENTS 里已被记为删除）。

## 但它仍被 CHANGELOG 引用
CHANGELOG.md:2358 「**P0 Spike GO**：profile-generator 确定性生成 bai ModelProfile…」
=> ⇒ 引用点在 CHANGELOG（历史记录），**不在任何现行文档或门禁**。

## 三个可核点
1. verification/ 是一个**只读归档区**，不是活代码：3 张对比图 + 1 份 spike 报告，
   没有任何脚本或测试。AGENTS 也没把它列进门禁。
2. 那份报告的**结论仍写着 GO**，而它描述的技术路线（JS renderer + soullink SDK）
   已在 AGENTS 里被记为「**已于 2026-09-11 删除**」。
   ⇒⇒ **⇒ 一份写着「GO」的报告，指向一个已经不存在的实现**
   ⇒⇒ 与 F-0637-01 **不是同一族**（那条是「说删了但还在」，这条是「说了结论但路线没了」）
   ⇒⇒ 差别值得记：**「已删除」的资产留在仓库里，是两种不同的问题**
   —— 一种是**状态说反了**（危险，会误导人以为它还活着），
   一种是**只留下证据**（无害，是 spike 的正常留存）。
3. **判据：一份结论型报告，要不要标注它描述的路线是否还在**
   ⇒⇒ 建议（一处）：在该报告首行加一句「**本文描述的 renderer/ 路线已于 2026-09-11 删除；
   本报告仅作 spike 留档**」。**P3 级**（纯归档卫生），本审计不执行写操作。

## 未核
CHANGELOG.md:2358 的上下文（那是不是唯一引用点）· rust-bakeoff 三张图的用途与是否被文档引用 ·
arbiter/decision/ledger/presets/plan/staging* 本体 · 其余 8 个 mod crate 本体 ·
mod-system 的 tests/ 四行 · shared/ 其余 8 个 json

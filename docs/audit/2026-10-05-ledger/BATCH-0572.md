# BATCH-0572 落盘（无装饰）· bakeoff 那一族是**活链路**，不是归档

## 关键更正：B0571 我把 verification/ 归为「只读归档区」——**只对了一半**

### 1. 三张图**是活的默认输出路径**
crates/l2d/examples/render_model.rs:26-27
  /// 默认输出：bakeoff 选型验证产物位置。
  const DEFAULT_OUTPUT: &str = "verification/rust-bakeoff/selected-bai.png";
=> **⇒ 我 B0571 说的「没有脚本、没有门禁」是错的**
=> **⇒ 有一个 example 的默认输出路径就指着它**，每次跑 render_model 都会写这张图。
render_model.rs:105 注释：「以便产物可与 verification/rust-bakeoff… 比对」

### 2. 那三张图背后的正文档在 docs/verification/（21 个文件），不在 verification/
docs/verification/rust-bakeoff-ayagami.md
docs/verification/rust-bakeoff-decision.md
docs/verification/rust-bakeoff-mocari.md
=> **⇒ 所以 verification/ 那 3 张 PNG 是 docs/verification/ 那三份 md 的附件**。
   两个同名目录，**图在根、文档在 docs/ 下** —— 这本身是一个可记的观察。

### 3. Cargo.toml 直接引用了那份文档
crates/live2d-ai-desktop/Cargo.toml:11-12
  # 与 crates/l2d / ayagami-render 的 wgpu 29 大版本一致（bakeoff 已验证组合，
  # 见 docs/verification/rust-bakeoff-ayagami.md；Cargo.lock 已含 wgpu 29.0.4）。
=> **⇒ 这是「bakeoff 的结论正在被当依据用」的活引用**，不是历史留存。
   与 soullink-p0-report.md（只被 CHANGELOG 引）**性质不同**。

### 4. CHANGELOG.md:2357 的上下文
# P0-P1 落地：soullink 表演引擎双跑（2026-08-22）
  - **P0 Spike GO**：profile-generator 确定性生成 bai ModelProfile（FACS 24->20 mapped）
  - **P1 双跑开关落地**：renderer 新增 soullink-adapter.ts（8key->naturalVAD 意图…）
  - **核心缺陷修复**：参数写入挂到 internalModel 的 afterMotionUpdate 相位…
=> 引用点确实在 CHANGELOG（历史），**且同一段里还提到 renderer 新增的 adapter** ——
   同样属于已删除的 JS 渲染器时代。

## 三个可核点
1. **更正自己**：B0571 的「没有脚本」是**假阴性**（B0440 第 1 形态：只搜了目录没搜路径引用）。
   render_model.rs:27 的 DEFAULT_OUTPUT 就在指这张图。**这一条记进 STATE 的自纠台账。**
2. **两个同名目录**：verification/（3 张 PNG + 1 份 spike 报告）vs docs/verification/（21 个文件，
   含三份 bakeoff md）⇒ 图与文档分居两处，而**代码里只引用文档路径**、
   **产物默认落在根 verification/** ⇒ **⇒ 判据 = 目录名相同不等于职责相同**。
3. ⇒⇒ 因此 verification/ 应分两类看：**rust-bakeoff/ 是活产物落点**（有 example 默认写它、
   有 Cargo.toml 引用其结论）；**soullink-p0-report.md 是 spike 留档**（路线已删、只被 CHANGELOG 引）。
   **⇒⇒ 与 B0571 那条「两种不同的问题」区分** 互相印证：**判据不是「在不在仓库里」，
   **而是「有没有活引用指向它」。**

## 未核
docs/verification/ 那 21 个文件本体（之前一直未核）· rust-bakeoff-decision.md 的结论 ·
render_model.rs 其余部分 · arbiter/decision/ledger/presets/plan/staging* 本体 ·
其余 8 个 mod crate 本体 · mod-system 的 tests/ 四行 · shared/ 其余 8 个 json

# BATCH-0054 · 渲染面剩余：帧循环 + `l2d` 姿态栈

Phase 1 · 域覆盖 → 渲染面（`l2d-wasm-demo` 收尾 + `crates/l2d` 首读）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/main.rs` — 962（读 853-962：输入监听收尾 + 帧循环 + rAF 调度）
2. `crates/l2d/src/pose_stack.rs` — 503（读 1-80 + 定点 143-213）

`web/surface/{render,idle}.rs`(821) 与 `crates/l2d/**` 其余 15 文件未读 → 顺延 BATCH-0055

## 跑过的命令（全部只读）
```
sed -n '853,962p' l2d-wasm-demo/src/main.rs
grep -n "mask|Mask|release|neutral|清零|写入" l2d/src/pose_stack.rs
sed -n '1,80p' l2d/src/pose_stack.rs
grep -n "InvalidDt|fn update|fn finalize|is_finite" l2d/src/pose_stack.rs
sed -n '175,189p' l2d/src/pose_stack.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（三项核验通过）
1. **帧循环无每帧分配**（main.rs:919-961）：单一常驻 `Closure` 放 `Rc<RefCell<Option<Closure>>>`，
   每帧 `schedule_frame` 调度；**fps 复用 rAF 时间戳、每秒才发一次**（:922-923
   「复用 rAF 时间戳，**不额外起定时器**」⇒ 没有第二个 60Hz 源）。`serde_json::json!`
   只出现在每秒那一次 `emit_event("fps", …)`（:937-940），不在热路径。
2. **输入交互尊重 `clickEnabled`**：wheel(:877-880) / dblclick(:902-905) / 拖拽
   三处都先查 `bridge.click_enabled`。⇒ B0030 核过 Flutter 侧**发**这个字段，
   本批核了渲染面**真的用它门控交互** —— 跨端偏好的一头一尾都对上了。
3. **`pose_stack.rs` 的分层结构才是「释放所有权」的落点**（:1-21）：五层各自持有独立
   `Pose`（base/idle/input/physics/final_override），**每帧重新合成**最终姿态
   （:184「idle 层：每帧从零重算」）。没被任何层写入的通道**不会出现在合成结果里**
   ⇒ 渲染器用自己的默认/idle —— 正是 B0043 核的 `ParameterMask`「未置位一律不写、
   中性意味着**释放所有权**」的**结构实现**。
   头注还记了它替换掉的旧实现的两条病症（:3-8）：idle 驱动与外部输入曾被合并进
   **一个持久的物理工作姿态**，导致「idle 被任何曾经设置过同键的用户值永久压制」
   与「`clear_parameter` 无法撤销已并入累积态的旧值（撤销不传播）」。
4. **非法 dt 在入口被拒、且不污染状态**：`update(dt)` 首行
   `if !(dt.is_finite() && dt > 0.0) { return Err(InvalidDt(dt)); }`（:178-180），
   检查在 `self.elapsed += dt`（:182）**之前** ⇒ 与它自己 :175 的承诺
   「**任何状态都不变**」一致。NaN/Inf/≤0 不会把整帧姿态污染成 NaN。

## 未核实项
1. `web/surface/render.rs`(551) / `idle.rs`(270) 未读 —— 帧循环里 `surface::tick` 的实体
2. `crates/l2d/**` 其余 15 文件未读（`model.rs` 318 / `renderer/mod.rs` 417 / `asset/mod.rs` 571 …）
3. `l2d-wasm-demo/src/preset/*`(2800) 未读
4. **红线 N 的正向验证仍未做**：首帧 `loaded` / iframe 重挂后的状态恢复
   （Dart 侧 `Live2DStage._attach` 的重发 B0030 核过 stageImage 与 actionScales；
   渲染面侧的重放未核）
5. `pose_stack::finalize`（:213）未读

## 本批新增
**0 条**（净产出：帧循环 / 交互门控 / 分层姿态栈 / dt 校验 四项核验通过）

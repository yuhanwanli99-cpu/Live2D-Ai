# BATCH-0045 · 渲染面：红线 O 接收端

Phase 1 · 域覆盖 → `crates/l2d-wasm-demo` / `crates/l2d`（**0 审区首触**）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/main.rs` — 962（**读 380-544**：消息接收入口 + 协议归一化）
2. `crates/l2d-wasm-demo/src/web/surface/input.rs` — 322（**读 140-199**：`apply_bridge_effects`）

## 跑过的命令（全部只读）
```
git ls-files 'crates/l2d-wasm-demo' 'crates/l2d/' | xargs wc -l | sort -rn
grep -n "\"mouth\"|MessageType::Mouth|Mouth =>|fn.*mouth|audio-volume" main.rs web/surface/*.rs
grep -rn "bridge.volume|\.volume\b" web/surface/render.rs web/surface/input.rs
grep -rn "mouth|MouthOpenY|audio-volume" web/surface/render.rs
sed -n '380,474p' main.rs ; sed -n '475,544p' main.rs ; sed -n '140,199p' input.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 红线 O：**两侧端到端验证通过**（Dart 发送端 B0023 + wasm 接收端本批）
- 红线 Q（契约只增不改）：v1 信封 + 扁平回退 + 逐字段钳位不整条拒收
- 维度 D：30Hz 通道**零纹理/零分配**
- 维度 C：`lipSync` 真的写 0（不是忽略消息）

## 未核实项
1. `l2d-wasm-demo/src/main.rs` 的 545-962 未读（含 **ACK 回执**路径与 `stage-ack`/`loaded`
   ——**红线 N「iframe 重建后状态恢复」的关键落点**）
2. `web/surface/{render,background,input}.rs` 的 render/idle 未读（551/360/270）
3. `l2d-wasm-demo/src/preset/*`（2800+ 行）未读
4. `l2d/**`（16 文件、~3000 行）**本批完全未读**
5. `web/surface/render.rs` 里 `volume` 的最终消费点未定位到行（只确认不在 render.rs 里出现）

## 本批新增
**0 条**（净产出：红线 O 双端验证 + 三处工程记录质量的正面样本）

# BATCH-0046 · ⭐ ACK 回执 / iframe 重建 —— 撞上本审计最重的一条安全发现

Phase 1 · 域覆盖 → `crates/l2d-wasm-demo`（渲染面）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/main.rs` — 962（**读 600-719**：模型切换 / `destroy` / 事件级 ack / `stage-ack`）
2. `shell/flutter/lib/live2d/live2d_host_web.dart` — （定点 41-48 / 78：**协议另一侧的对照**）
3. `crates/live2d-ai-desktop/src/web_api/wasm_assets.rs` — （定点 309-314：响应头）

## 跑过的命令（全部只读）
```
sed -n '600,719p' l2d-wasm-demo/src/main.rs
grep -n "X-Frame-Options|frame-ancestors|Content-Security-Policy" -r desktop/src/ wasm-demo/src/
grep -rn "origin|postMessage" wasm-demo/src/preset/tests/ wasm-demo/tests/
grep -rn "origin" crates/l2d-wasm-demo --include=*.rs | grep -v "^.*://"
grep -rn "postMessage" shell/flutter/lib/ ; grep -rn "location.origin|targetOrigin" shell/flutter/lib/
grep -n "with_header" -A 2 desktop/src/web_api/wasm_assets.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0046-01（P1）** —— 45 批以来第一条真正的安全发现
`postMessage` 的 **origin 校验只做了一半**：Dart（父页）收发两侧都做对了，
wasm（渲染面）**两侧都没做**。三条事实叠加构成可利用路径（全仓零防嵌套响应头 +
接收侧零 origin 校验 + 处理函数能触发任意 URL 抓取）。详见 FINDINGS.md。

**这条为什么重要**：它不是「团队不知道 origin 校验」——`live2d_host_web.dart:41-48`
把「发送钉 origin + 接收校验 origin」明写成了**该协议的契约**，并正确实现了父页那一侧。
**缺的只是属于 wasm 的那一半。** 同仓同协议的对照，让这条既可证伪也可直接照抄修法。

## 维度覆盖（本批）
- 维度 G（注入面）：**命中** —— 任务书 §7-G 明列「postMessage 的 origin/source 校验」
- 红线 N（iframe 重建）：`destroy` 分支（main.rs:638-648）复位 scale/offset/volume 并
  `fields.revoke_all(core)`，注释写明「不补帧（§9.3）」——**待继续核**（见未核实项）
- 维度 D：`stage-ack` 每次应用都构造一次 `serde_json::json!`（:696-704）——应用频率低，非热路径

## 未核实项
1. `main.rs` 的 **720-962**（tick / 帧循环 / 交互 / 首帧就绪）未读
2. 红线 N 的**正向**验证未做：iframe 重建后**舞台状态怎么恢复**（`stage-ack` 之外，
   `stage-bg` / `actionScales` / `mouthSensitivity` 的重发在 **Dart 侧** `_attach` 里，
   B0030 已核 `stageImage` 与 `actionScales` 会重发；**渲染面侧**的 `sync` 字段是否
   全部重放未核）
3. `web/surface/{render,idle}.rs`(821) 未读
4. `l2d-wasm-demo/src/preset/*`（2800+ 行）未读
5. `l2d/**`（16 文件、~3000 行）**本批完全未读**

## 本批新增
P0 0 · **P1 1** · P2 0 · P3 0

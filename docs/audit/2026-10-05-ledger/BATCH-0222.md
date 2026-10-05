# BATCH-0222 · ⭐ 把 F-0111-01 的**可达性**钉成确切形状（**定级不变**）

Phase 1 · 域覆盖 · `mod-voice-input/src/lib.rs`（`settings_spec` —— 决定 F-0111-01 的可达面）

## 跑的命令（全部只读）
```
grep -n "sidecar_python|sidecar_script|settings_spec|key:" mod-voice-input/src/lib.rs
sed -n '212,222p' mod-voice-input/src/lib.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**F-0111-01 补充「可达性的确切形状」**
```rust
ModSettingField::String {
    key: "sidecar_python".to_string(),
    label: "Python 解释器（缺省 python3）".to_string(),
    secret: false,                 // :216  ⇒ 面板按 **String** 渲成**普通文本框**（B0105 已核按 kind 渲染）
    default: None,                 // :217
}
```
⇒ ⇒ **`sidecar_script`(:196) 与 `sidecar_python`(:214) 是两个普通文本框**
⇒ ⇒ **用户不经改文件、不设环境变量，就能在正常设置界面里把本 Mod 指向
**机器上任意可执行文件 + 任意脚本路径**；而 ASR token 随后进入**那个进程**的 argv。

### ⚠ 而这**不构成提权** ⇒ **P2 不变**
用户自己选了解释器与脚本 ⇒ **信任本来就是他的** ⇒ **不是「越权执行」**。
⇒ **真正的问题仍是那两条**（与 B0111 一致）：
1. **无白名单** ⇒ 打错字、或出于好奇，就能让本 Mod 启动任意程序
2. **token 落在那个进程的 argv** ⇒ **同机进程可读**
   ⇒ 而 B0120 核过：`.env` 那侧**已经**用「**先 chmod tmp 再 rename**」关过一次窗口
   ⇒ ⇒ **这一处没有对等处置**

## 未核实项
1. `sidecar.rs` 的 `is_file()` 判据与 `--url` 变体的**完整实现**未重读（B0111/B0112 记过，本批只核 `settings_spec` 侧）
2. Mod crates 逐文件覆盖率（16/61）
3. `dev_tools_section.dart` 余面(1874) · `tokens.dart` 余 ~600 行 · `design_tokens_test.dart` 余 ~32 断言
4. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
5. **前端 `.dart` 183 未读（最大面）**
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核

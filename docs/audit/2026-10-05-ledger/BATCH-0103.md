# BATCH-0103 · `release-build.yml`（门禁轴最后一个未读 workflow）—— 门禁轴**结项**

Phase 1 · 域覆盖 · 门禁轴收尾

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `.github/workflows/release-build.yml` — 225（**读 1-142**：头注 / 触发器 / matrix / 全部 run 步骤）

## 跑过的命令（全部只读）
```
sed -n '1,70p' .github/workflows/release-build.yml
sed -n '71,142p' .github/workflows/release-build.yml
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0103-01（P3）** + **门禁轴结项**
### F-0103-01（P3）：发布构建**本身不跑任何测试**
`:134` 只有 `cargo build -p live2d-ai-desktop --release`，`:136` 只有 `ls -lh`；
**无** test / fmt / clippy / rust-ratio。而触发器是 `push: tags: ["v*"]`（:17-19）——
**不需要 PR、不需要 review** ⇒ **直接推 tag 就会产出从未跑过门禁的发布工件，且 workflow 不会红**。
**范围如实标注**：正常流程下 tag 来自已过 `pr-checks.yml` 的提交 ⇒ 用户拿到的工件对应过了门禁的代码；
且本 workflow **不在** AGENTS.md 的门禁表里 ⇒ **不构成「表与实现不符」**，
问题只在于**「tag 是发布入口」没有任何一处文字说明它不受门禁保护**。

### 正面：**本审计见过最高的事故记录密度**（且带 run ID、可复核）
- Linux apt 步（:82-99）：**两个 run ID**（`runs/34659978040、34660260293`）+ 日期 +
  **「根因至今未确定——不要再写「多余包导致失败」这类未经证实的结论」** +
  **「不用 apt 的退出码当成功判据」**；每个诊断都写明它**区分哪两种可能**
- `+crt-static` 移除（:116-132）：移除日期 + 「此前从未验证过」+ **两个独立问题叠加**的诊断
  （`RUSTFLAGS` 命中过程宏 / `-static-pie` 缺 `libasound.a`）+ 正解（musl）为何不在本 RC
- `defaults: shell: bash`（:42-44）：**2026-09-11 windows PowerShell 失败**的诊断
⇒ 这是我 B0056 提炼的记账卫生判据（「写时核对过的注释至今都真」）在**仓库里**的最强证据。

### ⭐ 门禁轴**结项结论**（5 个 workflow + 4 个脚本 + `xtask` + 2 份加固）
| 类别 | 结论 | 发现 |
|---|---|---|
| 门禁的**设计** | **可靠**：6+2+1 门禁齐全、opt-in 显式 skipped 而非伪造绿、**不拿工具退出码当判据**、事故记录带 run ID | — |
| 门禁的**覆盖** | **三处洞**：红线 K 无构建期网（CI 无 + 本地探针选错）；红线 R 密钥扫描**整体缺失** | F-0048-01 / F-0049-01 / F-0050-01 |
| 门禁的**诚实性** | **两处失真**：一句不存在的 Gitleaks；「静默降级」被四处实现反证 | F-0049-01 / F-0098-01 |
| 门禁的**发布入口** | **不受门禁保护**（tag 可绕过） | F-0103-01 |
⇒ **一句话**：**这个仓库的代码纪律强、门禁设计也强，但门禁的「覆盖面」与「自述的诚实性」各有缺口**；
而**最重的一条（P1）不在门禁里，在跨源边界上**（F-0046-01）。

## 未核实项
1. `release-build.yml:143-225`（wasm 可选构建步 / release note / 上传）未读
2. `stage_bg.rs`(295) · `web/surface/{render,idle}.rs` 余段 · `web/gpu.rs` 未读
3. `preset/` 余：`merge_expressions` · `field_runtime.rs:646+`
4. Mod crates 61 文件 / 24,665 行 —— **仍是最大的 0 审区**
5. 前端 35/217、`shared/`(6)、`tests/`(3 py)、`verification/`(1)

## 本批新增
P0 0 · P1 0 · P2 0 · **P3 1**

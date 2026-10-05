# BATCH-0048 · CI 工作流审计（门禁的定义处）

Phase 1 · 域覆盖 → **`.github/workflows/`**（门禁定义处，0 审）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `.github/workflows/pr-checks.yml` — 128（**全读**）
2. `.github/workflows/nightly.yml` — 133（**全读**）
3. `.github/workflows/flutter-checks.yml` — 51（**全读**）
4. `.github/workflows/build-upload.yml` — 104（**全读**）

`release-build.yml`(225) 未读；`scripts/*`(12) 与 `xtask/src/main.rs`(604) 未读 → 顺延 BATCH-0049

## 跑过的命令（全部只读）
```
git ls-files '.github/workflows' 'scripts' 'xtask' | xargs wc -l | sort -rn
grep -rn "ignite|gstatic|--no-web-resources-cdn|flutter build" .github/workflows/
grep -rn "flutter build|trunk build" .github/workflows/
grep -n "run:" -A 2 .github/workflows/*.yml          # 穷举全部 run 步骤
git ls-files | grep -i requirements
sed -n '44,103p' .github/workflows/build-upload.yml   # 逐块读多行 run
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0048-01（P2）—— 红线 K 没有任何构建期自动门禁**
穷举 5 个 workflow 的每一个 `run:` 步骤（含多行块）后：Rust 六道 + Dart 两道 + 根 pytest
全部在位；E2E 探针**严格 opt-in**（`if: vars.LIVE2D_AI_VERIFY_BASE_URL != ''` ⇒ 未配置时 job 为
**skipped** 而非 passed，「绝不伪造绿灯」实现正确）；
但**没有任何一步构建前端产物**，因此 `--no-web-resources-cdn`（唯一决定 CanvasKit 是否指向
gstatic 的开关）在 CI 里**从未被执行**，而唯一会 grep 构建产物的 `ignite.sh --check` 也不在 CI。
AGENTS.md 为 **wasm** 那半边声明了「已知缺口」，**前端**这半边未声明。

## 维度覆盖（本批）
- 红线 K：命中（构建期无门禁）→ F-0048-01
- 任务书 §2 纪律「缺配置明确跳过，**绝不伪造绿灯**」：**✔ 实现正确**（nightly.yml:74 + :62-70 的理由）
- AGENTS.md「本地必跑 vs CI 必跑」表：逐条核对**属实**（Rust 六道 + Dart 两道 + 根 pytest 都在 CI 里）
- 门禁健壮性：`root-py-tests` 的 `requirements-test.txt` **在树里**（假设证伪，该 job 可运行）
- `pr-checks.yml` 的 `paths-ignore: *.md / docs/** / .pi/**` 合理（纯文档 PR 不必等工具链）

## 未核实项
1. `.github/workflows/release-build.yml`(225) 未读
2. `scripts/` 12 个未读 —— 其中 `check_public_secrets.py`(108) 与 `ignite.sh`(211) 是红线 K/R 的
   **本地**执行点，**必须读**（F-0048-01 的修法②就落在 ignite.sh 的口径上）
3. `xtask/src/main.rs`(604) 未读 —— `rust-ratio` 的统计口径（它决定 ≥95% 这条门禁的真假）
4. `scripts/verify_core_chain.py`(460) 未读 —— E2E 探针本身是否会「假绿」

## 本批新增
P0 0 · P1 0 · **P2 1** · P3 0

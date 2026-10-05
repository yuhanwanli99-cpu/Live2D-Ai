# BATCH-0617 · .github/ 首次审：8 个文件 · pr-checks 三个 job 与 AGENTS 表逐条对得上

Phase 1 · 域覆盖 · CI（.github/ 首次审；同时核 F-0616-01 建议①的落点）

## 跑的命令（全部只读）
```
git ls-files .github/                                    # 8 个
grep -nE "^  [a-z-]+:" .github/workflows/pr-checks.yml
```
未跑任何 CI 命令。

## 清单（8 个）
| 文件 | 状态 |
|---|---|
| workflows/pr-checks.yml | **3 job**：rust-build-test(:30) / msrv-check(:74) / root-py-tests(:102) |
| workflows/flutter-checks.yml | 未核 |
| workflows/nightly.yml | 未核（AGENTS 记 core-chain-verify **仅当**配置 LIVE2D_AI_VERIFY_BASE_URL） |
| workflows/build-upload.yml | 未核（不在 AGENTS 表里） |
| workflows/release-build.yml | 未核（不在 AGENTS 表里） |
| ISSUE_TEMPLATE/bug.yml · feature.yml | 未核 |
| PULL_REQUEST_TEMPLATE.md | 未核 |

## 三个可核点
1. pr-checks.yml 的三个 job 名与 AGENTS 门禁表的「Rust 测试 / MSRV 1.92 / 仓库根历史资产测试」
   逐条对应 ⇒ **那张表不是凭印象写的、它真的对着 workflow** ⇒ 与 B0612 核的「-h 用法是 sed 读出来的」同族
   （声明与实现不许有第二份）。
2. ⇒ **F-0616-01 建议①的落点准确**：check_public_secrets 既不在这 3 个 job 里、也不在 AGENTS 表里
   ⇒ 建议①「接进 pr-checks.yml」有确切位置。
3. ⇒ **而 build-upload.yml / release-build.yml 两个发布 workflow 不在表里**
   ⇒ **AGENTS 那条只约束「本地必跑」侧**（「本地提交前跑的命令必须能在 CI 清单里逐条找到」），
   **不约束 CI 可以有什么** ⇒ **⇒ 两个方向各有一半的纪律，没有一条说「CI 里不许有表外的东西」**
   ⇒ **⇒ 这不是缺陷，是「那张表的管辖范围比名字窄」** ⇒ 记为观察（修法：把表头改成「本地必跑 vs CI 必跑（**本地侧**）」）。

## 未核
flutter-checks.yml / nightly.yml / build-upload.yml / release-build.yml 本体 ·
ISSUE_TEMPLATE 两份 · PULL_REQUEST_TEMPLATE.md · nightly 的「缺配置明确跳过」在代码里的形状
（AGENTS 写的是**政策**、未核**实现**）。

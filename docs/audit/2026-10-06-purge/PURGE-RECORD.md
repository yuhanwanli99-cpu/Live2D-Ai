# 历史清洗记录（2026-10-06）· F-0616-01

> 本文件是**执行记录**，不含任何原始凭据（原文已从全部历史与工作树清除）。
> 独立验证报告（另一位成员从备份 bundle 逐 ref 复核）：[`PURGE-VERIFY.md`](PURGE-VERIFY.md)。

## 1. 触发

审计 `F-0616-01`：`scripts/deploy_android.sh` 自 `v0.1.0-rc.1` 的**根提交**（`a11fe515`，公开历史起点）起，
就在**公开远端** `github.com/yuhanwanli99-cpu/Live2D-Ai` 上，且文件头注与 `PHONE_PASSWORD` 默认值里写死了
维护者**手机锁屏口令**与局域网地址。

## 2. 清洗动作（`git filter-repo`）

```bash
# 备份（先做，可回滚）
git bundle create /home/skystar/backup-2026-10-06/pre-purge-all-refs.bundle --all   # verify: complete, 104 refs
git show-ref > /home/skystar/backup-2026-10-06/refs-before.txt

# 替换表（/tmp/purge-replacements.txt；此处写脱敏后的形态）
#   <5 位数字口令>      ==> <REDACTED-DEVICE-PIN-2026-10-06>
#   <局域网 IP>        ==> 192.168.0.x

python3 /tmp/git-filter-repo --force \
  --invert-paths --path scripts/deploy_android.sh \
  --replace-text /tmp/purge-replacements.txt
```

- `--invert-paths --path scripts/deploy_android.sh`：**从全部历史删除该路径**（不只是改内容）；
- `--replace-text`：把两个标识串（5 位数字口令 / 局域网 IP）在**所有 blob** 里替换（含台账里对它的引用）；本文件按脱敏形态书写，不写原文；
- filter-repo 会移除 `origin` remote（防误推），推送前需重新 `git remote add`。

## 3. 结果（原始数字）

| 项 | 清洗前 | 清洗后 |
| --- | --- | --- |
| `main` 提交 | `d03347b1` | `e497edc7`（+ 之后 1 个 release 提交 `11ded6a`） |
| `main` 提交数 | 188 | 188 |
| 全 ref 提交数 | 574 | **573**（「只删该文件」的提交变空 ⇒ 被丢弃） |
| `main` **树哈希** | `c5f82d3d0e8ed9e2b8289e42c2358f4f902cd47b` | **`c5f82d3d…`（完全相同）** |
| 两个 needle 在新历史命中 | — | **0 / 0** |
| `git log --all -- scripts/deploy_android.sh` | 6 个 blob | **空** |

**逐 ref 验证**（脚本 `per_ref_verify.py`，两边 mirror 对比）：100 个 ref，
**4 个树哈希相同、96 个不同；唯一被删路径 = `scripts/deploy_android.sh`；
202 个 blob 的差异全部由上述替换解释；意外差异 0** ⇒ VERDICT **PASS**。

## 4. 推送（force-push，2026-10-06）

| 远端 ref | 清洗前 | 清洗后 |
| --- | --- | --- |
| `refs/heads/main` | `2d49244`（rc.1 时代） | **`11ded6a`** |
| `refs/heads/mainline/1-core-baseline` | `65e6211` | **`b58b223`**（同样清洗） |
| `v0.1.0-rc.1 … rc.5`、`v0.2.0-rc.1`（6 个 tag） | 旧对象 | **全部重写**（force） |
| `v0.2.0` / `v0.2.1-rc.1` | 远端不存在 | **新增** |

## 5. 仍然残留（如实记录，不许假装解决）

1. **`refs/pull/1/head` = `44da2a4c`**（PR #1，已 merge/关闭）：GitHub 托管，
   **不能 push、不能删**；它可能仍指向清洗前的对象。要彻底抹除需联系 GitHub Support。
2. GitHub 侧旧对象在被 GC 前，仍可能通过**直接 SHA** 访问（无法由仓库方强制过期）。
3. fork / 第三方缓存（本仓库 `forks_count = 0`，暂无）。

⇒ **真正的修复是轮换那台手机的锁屏口令**（维护者动作）。

## 7. 第二轮（同日，仅标识串 B）

**起因**：独立复核（[`PURGE-VERIFY.md`](PURGE-VERIFY.md) §0 结论 4）抓到——替换只作用于**历史 blob**，
而我在写发布说明/本记录时把**第二个标识串（局域网 IP）写回了当前树**，其中一处已随 `v0.2.1-rc.1` 的发布提交推到远端。

**处置**：
1. 当前树三处（发布说明 / 本记录 / 复核报告）改为脱敏描述（已推 `main`）；
2. 对被 tag 固定的那三份历史版本再跑一次 `git filter-repo --replace-text`（只替换该 IP）——
   只有 3 个提交被改写（发布提交 `11ded6a → 19a9f63`、`15271e1 → c59f27a`、`f42fa2c → 2e27a72`），更早历史哈希不变；
3. `v0.2.1-rc.1` 标签随重写移动，远端 tag 与 GitHub release 同步重建。

**验证**：全部 ref 的两个 needle 命中 **0 / 0**；本地对象库（含 cruft）**0 / 0**。

**教训（写进流程）**：脱敏文档里**不许出现原文**——哪怕是在描述「我把它替换成了什么」。
第一轮就违反了这一点，且只有独立复核的全 blob 反向检查才抓得到。

## 6. 发布

清洗后的历史已带 `v0.2.1-rc.1` 预发布：
<https://github.com/yuhanwanli99-cpu/Live2D-Ai/releases/tag/v0.2.1-rc.1>
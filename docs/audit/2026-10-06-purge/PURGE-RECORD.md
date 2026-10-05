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
#   192.168.0.x      ==> 192.168.0.x

python3 /tmp/git-filter-repo --force \
  --invert-paths --path scripts/deploy_android.sh \
  --replace-text /tmp/purge-replacements.txt
```

- `--invert-paths --path scripts/deploy_android.sh`：**从全部历史删除该路径**（不只是改内容）；
- `--replace-text`：把两个标识串在**所有 blob** 里替换（含台账里对它的引用）；
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

## 6. 发布

清洗后的历史已带 `v0.2.1-rc.1` 预发布：
<https://github.com/yuhanwanli99-cpu/Live2D-Ai/releases/tag/v0.2.1-rc.1>
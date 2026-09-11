# Android 归档指针（ANDROID_ARCHIVE_POINTER.md）

> **2026-09-11 更新**：公开仓库**历史重新起算**（`main` 成为单个根提交），
> 因此 Android 归档的**远端分支 `android-archive` 已删除**，归档现在**只存在本地**。
> 本文件记录本地位置与恢复方法。

## 归档位置（2026-09-11 起：仅本地）

| 项目 | 值 |
| --- | --- |
| 归档 commit | `fa48c0ddbab1983d520d45238d3e171ca2bd4936` |
| 本地分支 | `android-archive`（`git branch --list android-archive`） |
| 专用 bundle | `/home/skystar/Live2D-Ai-ANDROID-ARCHIVE.bundle`（731 KB） |
| 专用 bundle SHA256 | `2a2df2b52b2d2b9e997881d82c2e8095ad28d068ff24910ea7ed1f16388e7425` |
| 全历史 bundle（含本归档） | `/home/skystar/Live2D-Ai-LEGACY-FULL-HISTORY.bundle`（110 MB） |
| 全历史 bundle SHA256 | `be29b79d81a0f1071cdd165a0154e14ed4e470004e85c262f85705c09b8155fa` |
| **已失效**：远端分支 | ~~`refs/heads/android-archive`~~（2026-09-11 删除） |
| **已失效**：旧离线 bundle | ~~`/mnt/g/git/Live2D-Ai-Android-archive-20260826.bundle`~~（**已不存在**，`/mnt/g` 为空盘） |

> ⚠️ 本文件 2026-08-26 版本记的 `/mnt/g/git/…bundle` 备份**已经丢了**。
> 现在能保证 Android 源码不丢的只有上表里的**本地分支 + 两个 bundle**。
> 删任何本地 ref 之前先确认 bundle 仍可 `git bundle verify` 通过。

## 相关交接文档

- HANDOFF：`docs/development/HANDOFF-2026-08-25-runtime-fixes.md`
- 公开历史重置说明：`docs/releases/v0.1.0-rc.1.md`「历史重置」

## 恢复方法

从本地分支（最快，前提是本地仓库仍在）：

```bash
git checkout -b android-restored android-archive
```

从专用 bundle 恢复：

```bash
sha256sum /home/skystar/Live2D-Ai-ANDROID-ARCHIVE.bundle   # 应等于上方 SHA256
git clone -b android-archive /home/skystar/Live2D-Ai-ANDROID-ARCHIVE.bundle Live2D-Ai-Android-restored
```

从全历史 bundle 恢复（包含 Android + Python + 全部旧 ref）：

```bash
sha256sum /home/skystar/Live2D-Ai-LEGACY-FULL-HISTORY.bundle   # 应等于上方 SHA256
git clone /home/skystar/Live2D-Ai-LEGACY-FULL-HISTORY.bundle Live2D-Ai-full-restored
```

**不再可用**（远端已删）：

```bash
# git clone -b android-archive https://github.com/yuhanwanli99-cpu/Live2Dai.git   # ← 会失败
```

校验本地归档完整性：

```bash
git rev-parse android-archive^{commit}    # 应输出 fa48c0ddbab1983d520d45238d3e171ca2bd4936
git bundle verify /home/skystar/Live2D-Ai-ANDROID-ARCHIVE.bundle                  # 应报 okay
git bundle verify /home/skystar/Live2D-Ai-LEGACY-FULL-HISTORY.bundle              # 应报 okay
```

## 删除记录（历史）

- **2026-08-26 18:41 (+08:00)**：删除本地 Android 工作副本与临时构建目录
  （`Live2D-Ai-Android/`、`.snap-assets-before-build/`、Windows Temp 下构建暂存、
  `dist/` 下两个 debug APK 等）。当时假定「远端分支 + `/mnt/g` bundle」双备份成立。
- **2026-09-11**：公开仓库历史重新起算，**远端 `android-archive` 分支删除**；
  复核时发现 `/mnt/g` 那份 bundle 已不存在 → 另建上面两个本地 bundle 补足备份。

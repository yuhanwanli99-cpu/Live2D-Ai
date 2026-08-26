# Android 归档指针（ANDROID_ARCHIVE_POINTER.md）

> 本文件由已授权的破坏性清理流程于删除本地 Android 工作副本前创建。
> 作用：Android 源码已完整推送到远端分支并另有 bundle 离线备份，本地副本可安全重建。

## 归档位置

| 项目 | 值 |
| --- | --- |
| 远端 URL | https://github.com/yuhanwanli99-cpu/Live2Dai.git |
| 远端分支 | `refs/heads/android-archive` |
| 归档 commit | `fa48c0ddbab1983d520d45238d3e171ca2bd4936` |
| 离线 bundle | `/mnt/g/git/Live2D-Ai-Android-archive-20260826.bundle`（完整历史，含 `refs/heads/android-archive`） |
| bundle SHA256 | `869d58c88a04b7be4f2538a701b1f0dae71dc6e8bcf9aa6f0deaa6025c30657d` |

## 相关交接文档

- HANDOFF：`docs/development/HANDOFF-2026-08-25-runtime-fixes.md`

## 删除记录

- **删除时间**：2026-08-26 18:41 (+08:00)
- **删除范围**：仅限本次清理白名单中的 Android 工作副本与临时构建目录
  （本仓库内 `Live2D-Ai-Android/`、`.snap-assets-before-build/`、Windows Temp 下 live2dai 构建暂存目录、
  `/mnt/g/git/Live2Dai/Live2Dai-Android/`、`dist/` 下两个 debug APK 与 SHA256SUMS-local.txt 等，详见清理日志）。
- **保留**：Android SDK/NDK、全局 Gradle/Cargo 缓存、根目录 target/.venv、PC 端目录、shared 源码、上述 bundle。

## 恢复方法

从远端恢复（推荐）：

```bash
git clone -b android-archive https://github.com/yuhanwanli99-cpu/Live2Dai.git Live2D-Ai-Android-restored
```

从离线 bundle 恢复：

```bash
sha256sum /mnt/g/git/Live2D-Ai-Android-archive-20260826.bundle   # 应等于上方 SHA256
git clone -b android-archive /mnt/g/git/Live2D-Ai-Android-archive-20260826.bundle Live2D-Ai-Android-restored
```

校验归档完整性：

```bash
git ls-remote origin refs/heads/android-archive        # 应输出 fa48c0ddbab1983d520d45238d3e171ca2bd4936
git bundle verify /mnt/g/git/Live2D-Ai-Android-archive-20260826.bundle   # 在任意该仓库工作副本内执行，应报 okay
```

# Linux 环境定位：开发/测试，不作为发布平台

> 主力平台：Android（手机）+ Windows（桌面）。
> Linux 只用于自动化测试、CI、共享配置/契约开发，不生成 Linux 安装包。

## 为什么值得做 Linux 适配

- 方便跑 root pytest、`shared/check_cross_platform_consistency.py`、`scripts/health_check.py`
- 方便在无 Windows/Android 的机器上做纯逻辑开发
- CI 可以低成本跑“无 Key 地基验证”
- 避免每次都要在 Windows 上才能验证配置/契约

## 不做什么

- 不维护 Linux 版 Live2D 桌面端安装包
- 不承诺 PC 端所有 Windows 专属能力（如便携 Python、ffmpeg.exe、pywin32）在 Linux 可用
- Android 构建仍以 Windows/Android Studio 或 CI 为主

## 快速准备

```bash
bash scripts/setup_linux.sh
source .venv/bin/activate
python scripts/verify_all.py
```

`verify_all.py` 在 Linux 上默认跑：

- health check
- shared registry validation
- cross-platform consistency
- root pytest

需要跑 PC pytest 时，PC 的 Open-LLM-VTuber 依赖较重（torch/onnxruntime），建议在 Windows 或专用 CI 环境跑。

## 目录约定

- `scripts/setup_linux.sh`：Linux 轻量环境准备
- `scripts/health_check.py`：无 Key 健康自检
- `scripts/verify_all.py`：一键验证入口

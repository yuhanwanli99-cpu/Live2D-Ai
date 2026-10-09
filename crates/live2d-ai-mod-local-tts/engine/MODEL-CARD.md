# Fun-CosyVoice3-0.5B-2512（模型卡出处）

| 项 | 值 |
| --- | --- |
| 模型 | `FunAudioLLM/Fun-CosyVoice3-0.5B-2512` |
| 模型卡 | https://huggingface.co/FunAudioLLM/Fun-CosyVoice3-0.5B-2512 |
| 修订号 | `29e01c4e8d000f4bcd70751be16fa94bf3d85a18`（2026-02-03，2026-10-09 核对） |
| 许可 | apache-2.0（模型卡标注） |
| 权重落点 | `crates/live2d-ai-mod-local-tts/weights/`（**进 .gitignore，不进 git、不进 LFS**） |

## 为什么不是别的

计划书口径（2026-10-09）：**就是这一款 0.5B**。不是 CosyVoice2，也不是再去调用
`/home/skystar/CosyVoice 3.0/scripts/3-start.sh`——那条路径属于仓库外的历史安装，
本轮的产品路径是「已启用的 Mod 自己拉起这一款」。

## 怎么拿到权重

```bash
crates/live2d-ai-mod-local-tts/engine/download.sh
```

脚本做两件事，任何一步失败都**非 0 退出**（Mod 不会把失败写成成功）：

1. 把 CosyVoice 源码按 `074ca6dc9e80a2f424f1f74b48bdd7d3fea531cc` 检出一份**干净**副本，
   落在**仓库外**的缓存目录（`$HOME/.cache/live2d-ai/cosyvoice3`，可用
   `LIVE2D_AI_COSYVOICE_HOME` 覆盖）。**不落进仓库**：`*.py` 计入 rust-ratio 分母。
2. 把上表那份权重按修订号下到本 crate 的 `weights/`。

## 合规

- 再分发权重：随包带上 Apache-2.0 许可、模型卡链接与修订号（见同目录 NOTICE）。
- 不要 `git add` 权重（`.pt` 等是数 GB，GitHub 单文件上限 100MB；本仓库没有 Git LFS 规则覆盖它）。
- 不要把这棵树整份拷进仓库；不要把它跟 `/home/skystar/CosyVoice 3.0` 里的个人音色混淆。

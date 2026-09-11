# Live2D-Ai 冒烟测试清单（真机 + 桌面）

> 目标：P0-P3 细化后，按此清单快速验证核心链路是否可用。
> 自动测试入口：`python scripts/verify_all.py --android --pc`

## 1. 本地环境预检

```bash
python scripts/health_check.py
python scripts/verify_all.py --android --pc
```

- [ ] health check 通过
- [ ] consistency 33/33 通过
- [ ] root pytest 通过
- [ ] Android JVM 测试通过
- [ ] PC pytest 通过

## 2. Android 真机冒烟

### 前置
- [ ] 安装最新 debug APK
- [ ] 已授予录音权限
- [ ] 有可用 LLM Key（DeepSeek / 其他）

### 步骤
1. [ ] 打开 App，角色正常显示
2. [ ] 输入文字发送，LLM 回复正常
3. [ ] 回复后 TTS 播放，角色有口型
4. [ ] 顶部显示 `TTS: xxx` 或 `🔊 ...`
5. [ ] 点击麦克风，说话后识别文本自动进入对话
6. [ ] 设置页切换“语音输入”关闭后，麦克风不再启动
7. [ ] 设置页切换“识别引擎 = 离线 SenseVoice”（需先下载模型），说话后能离线识别
8. [ ] 设置页切换渲染后端为 ENGINE，角色仍能显示；若异常自动回 LEGACY

## 3. PC 桌面冒烟

### 前置
- [ ] Windows 便携包已解压或本地 `start.ps1` 可启动
- [ ] `conf.yaml` 中 `asr_model: faster_whisper`
- [ ] Faster-Whisper 模型已存在于 `models/whisper/`
- [ ] 浏览器为 Chrome/Edge（支持 Web Speech API）

### 步骤
1. [ ] 启动 PC 端，浏览器打开 `http://localhost:12393/`
2. [ ] 角色显示正常，待机/眨眼正常
3. [ ] 输入文字发送，LLM 回复 + TTS 播放
4. [ ] 聊天面板出现 `TTS 空闲` / `🔊 播放中` 状态
5. [ ] 点击麦克风按钮，说话后识别文本自动发送
6. [ ] 浏览器不支持语音时，面板出现明确错误提示

## 4. 回归项

- [ ] 无 Key / 无网络时，App/PC 不崩溃，降级提示可见
- [ ] 打断说话后，TTS 停止、口型归零、表情收敛
- [ ] 双端共享 `shared/persona.yaml` 一致

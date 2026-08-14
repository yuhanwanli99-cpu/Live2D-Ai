package com.live2dai.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import com.live2dai.android.ui.theme.Live2DaiTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // V1.2 模块1：注册默认 TTS Provider（CosyVoice → sherpa 离线 → Edge TTS → 系统 TTS
        // 降级链，见 registerDefaultTtsProviders）。必须在 VoiceIoController 构造（组合阶段）
        // 之前注册；同名覆盖幂等，Activity 重建安全。
        registerDefaultTtsProviders(applicationContext)
        // 2C-4 UI 精简：ASR（麦克风语音输入）已随 UI 裁剪删除，不再注册默认 ASR Provider
        // （原调用 registerDefaultAsrProviders(applicationContext) 移除；VoiceIoController
        // 对空 ASR 注册表有兜底，见 VoiceIoController.kt「ASR 降级链兜底」）。
        setContent {
            Live2DaiTheme {
                Live2DaiApp()
            }
        }
    }
}

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
        // V1.2 模块4：注册默认 ASR Provider（sherpa 离线 → 系统语音识别降级链）。
        // 同样需在 VoiceIoController 构造前；VoiceIoController init 另有空表兜底。
        registerDefaultAsrProviders(applicationContext)
        setContent {
            Live2DaiTheme {
                Live2DaiApp()
            }
        }
    }
}

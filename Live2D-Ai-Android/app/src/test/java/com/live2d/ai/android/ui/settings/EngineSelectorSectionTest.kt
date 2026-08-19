package com.live2d.ai.android.ui.settings

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.live2d.ai.android.TtsHealth
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * EngineSelectorSection Compose 组件测试（V1.2 模块5，plan §5.5.2 方案A RadioGroup）。
 *
 * Robolectric 本地运行（与 ModelSelectorTest 同模式）：
 * - 渲染引擎名 + 健康徽标（正常/降级/不可用）+ 离线/联网标识
 * - 点击行触发 onSelect(id)
 * - 选中态通过 RadioButton 呈现
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w400dp-h800dp")
class EngineSelectorSectionTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val options = listOf(
        EngineOptionUi(
            id = "cosyvoice_ws",
            name = "CosyVoice 云端",
            requiresNetwork = true,
            health = TtsHealth.OK,
            configurable = true,
            isSelected = true,
        ),
        EngineOptionUi(
            id = "sherpa_onnx_kokoro",
            name = "Kokoro 离线",
            requiresNetwork = false,
            health = TtsHealth.UNAVAILABLE,
            configurable = false,
            isSelected = false,
        ),
    )

    private fun setContent(onSelect: (String) -> Unit = {}) {
        composeRule.setContent {
            MaterialTheme {
                EngineSelectorSection(options = options, onSelect = onSelect)
            }
        }
    }

    @Test
    fun rendersEngineNamesAndHealthBadges() {
        setContent()
        composeRule.onNodeWithText("CosyVoice 云端").assertIsDisplayed()
        composeRule.onNodeWithText("Kokoro 离线").assertIsDisplayed()
        // 健康徽标：正常 / 不可用
        composeRule.onNodeWithText("正常").assertIsDisplayed()
        composeRule.onNodeWithText("不可用").assertIsDisplayed()
        // 网络/离线标识
        composeRule.onNodeWithText("需联网 · 云端引擎").assertIsDisplayed()
        composeRule.onNodeWithText("离线可用 · 本地引擎").assertIsDisplayed()
    }

    @Test
    fun clickOption_triggersOnSelectWithId() {
        var selected: String? = null
        setContent(onSelect = { selected = it })
        composeRule.onNodeWithText("Kokoro 离线").performClick()
        assertEquals("点击引擎行应触发 onSelect(engineId)", "sherpa_onnx_kokoro", selected)
    }

    @Test
    fun clickSelectedOption_triggersOnSelectAgain() {
        var selected: String? = null
        setContent(onSelect = { selected = it })
        composeRule.onNodeWithText("CosyVoice 云端").performClick()
        assertEquals("点击当前选中项也应触发 onSelect（幂等，UI 层负责去重）", "cosyvoice_ws", selected)
    }
}

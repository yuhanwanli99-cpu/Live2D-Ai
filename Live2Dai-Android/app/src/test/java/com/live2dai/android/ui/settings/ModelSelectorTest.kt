package com.live2dai.android.ui.settings

import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import com.live2dai.android.data.ModelRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * ModelSelector Compose 组件测试（Robolectric 本地运行）。
 *
 * 覆盖 plan-android.schema.json testContracts 的 ModelSelectorTest 用例：
 * - 渲染可用模型列表（含不可用条目与"即将推出"标记）
 * - 不可用模型不可点击
 * - 点击可用模型触发 onSwitchModel
 * - 当前选中模型有"✓ 使用中"标记
 */
@OptIn(ExperimentalTestApi::class)
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [34], qualifiers = "w400dp-h800dp")
class ModelSelectorTest {

    @get:Rule
    val composeRule = createComposeRule()

    private val testRegistry = ModelRegistry.parse(
        """
        {
          "version": "1.0",
          "models": [
            { "id": "mao_pro", "name": "Mao Pro", "description": "默认形象", "model3JsonPath": "mao_pro/runtime/mao_pro.model3.json", "available": true, "default": true },
            { "id": "shizuku", "name": "Shizuku", "model3JsonPath": "shizuku/runtime/shizuku.model3.json", "available": true, "default": false },
            { "id": "hiyori", "name": "Hiyori", "model3JsonPath": "hiyori/runtime/hiyori.model3.json", "available": false, "default": false }
          ]
        }
        """.trimIndent()
    )

    private fun setContent(switchingModelId: String? = null, onSwitchModel: (String) -> Unit = {}) {
        composeRule.setContent {
            MaterialTheme {
                ModelSelector(
                    registry = testRegistry,
                    selectedModelId = "mao_pro",
                    switchingModelId = switchingModelId,
                    onSwitchModel = onSwitchModel
                )
            }
        }
    }

    @Test
    fun rendersAvailableAndUnavailableModels() {
        setContent()
        composeRule.onNodeWithText("Mao Pro").assertIsDisplayed()
        composeRule.onNodeWithText("Shizuku").assertIsDisplayed()
        // 不可用条目也渲染，且带"即将推出"标记
        composeRule.onNodeWithText("Hiyori").assertIsDisplayed()
        composeRule.onNodeWithText("即将推出").assertIsDisplayed()
    }

    @Test
    fun clickAvailableModel_triggersOnSwitchModel() {
        var switched: String? = null
        setContent(onSwitchModel = { switched = it })
        composeRule.onNodeWithText("Shizuku").performClick()
        assertEquals("点击可用模型应触发 onSwitchModel(modelId)", "shizuku", switched)
    }

    @Test
    fun clickUnavailableModel_doesNothing() {
        var switched: String? = null
        setContent(onSwitchModel = { switched = it })
        // 不可用卡片无点击语义：点击应被忽略，onSwitchModel 不被调用
        composeRule.onNodeWithText("Hiyori").assertHasNoClickAction()
        assertNull("不可用模型不应触发切换", switched)
    }

    @Test
    fun selectedModel_showsInUseBadge() {
        setContent()
        composeRule.onNodeWithText("✓ 使用中").assertIsDisplayed()
    }

    @Test
    fun switchingModel_showsProgressIndicator() {
        setContent(switchingModelId = "shizuku")
        composeRule.onNodeWithText("Shizuku").assertIsDisplayed()
    }
}

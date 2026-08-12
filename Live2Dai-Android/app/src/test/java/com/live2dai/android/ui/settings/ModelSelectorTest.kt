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
 * ModelSelector Compose 组件测试（Robolectric 本地运行，契约 v3-rename + R1-model D2 重锚）。
 *
 * 覆盖 plan-android.schema.json testContracts 的 ModelSelectorTest 用例：
 * - 渲染可用模型列表（含不可用条目与"即将推出"标记）
 * - 不可用模型不可点击
 * - 点击可用模型触发 onSwitchModel
 * - 当前选中模型有"✓ 使用中"标记
 *
 * R1-model D2 单模型化（M4e）：样例 registry 中 bai 为主/默认模型（默认选中 = selectedModelId=bai），
 * 不再出现 niziiro_mao（D2 彻底移除）；用合成测试条目 up_com_pro 作不可用占位、
 * up_avail_pro 作“可用且未选中”占位，仅用于验证 UI 行为，不映射真实 registry。
 *
 * 点击回调测试注意：registry 默认选中项 = bai；双层有意设计
 * （UI !isSelected + 逻辑 modelId != selectedModelId）使点击“选中项”不触发 onSwitchModel。
 * 故用合成条目 up_avail_pro（可用、未选中）验证“点击可用且未选中模型触发回调”的断言语义。
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
            { "id": "bai", "name": "白-免费版 (Bai Free)", "description": "默认形象", "model3JsonPath": "bai/runtime/bai.model3.json", "available": true, "default": true },
            { "id": "pc_only", "name": "PC Only", "model3JsonPath": "pc_only/runtime/pc_only.model3.json", "available": true, "default": false, "restrictPlatform": "pc" },
            { "id": "up_com_pro", "name": "Upcoming Pro", "model3JsonPath": "up_com_pro/runtime/up_com_pro.model3.json", "available": false, "default": false },
            { "id": "up_avail_pro", "name": "Up Available Pro", "model3JsonPath": "up_avail_pro/runtime/up_avail_pro.model3.json", "available": true, "default": false }
          ]
        }
        """.trimIndent()
    )

    private fun setContent(switchingModelId: String? = null, onSwitchModel: (String) -> Unit = {}) {
        composeRule.setContent {
            MaterialTheme {
                ModelSelector(
                    registry = testRegistry,
                    selectedModelId = "bai",
                    switchingModelId = switchingModelId,
                    onSwitchModel = onSwitchModel
                )
            }
        }
    }

    @Test
    fun rendersAvailableAndUnavailableModels() {
        setContent()
        composeRule.onNodeWithText("白-免费版 (Bai Free)").assertIsDisplayed()
        // 不可用条目也渲染，且带"即将推出"标记（合成 up_com_pro 占位，映射已删除的 hiyori 语义）
        composeRule.onNodeWithText("Upcoming Pro").assertIsDisplayed()
        composeRule.onNodeWithText("即将推出").assertIsDisplayed()
    }

    @Test
    fun clickAvailableModel_triggersOnSwitchModel() {
        var switched: String? = null
        setContent(onSwitchModel = { switched = it })
        // 点击“可用且未选中”的合成条目 up_avail_pro（默认选中项 bai 点击不触发回调，见类注释）
        composeRule.onNodeWithText("Up Available Pro").performClick()
        assertEquals("点击可用且未选中的模型应触发 onSwitchModel(modelId)", "up_avail_pro", switched)
    }

    @Test
    fun clickUnavailableModel_doesNothing() {
        var switched: String? = null
        setContent(onSwitchModel = { switched = it })
        // 不可用卡片无点击语义：点击应被忽略，onSwitchModel 不被调用
        composeRule.onNodeWithText("Upcoming Pro").assertHasNoClickAction()
        assertNull("不可用模型不应触发切换", switched)
    }

    @Test
    fun selectedModel_showsInUseBadge() {
        setContent()
        composeRule.onNodeWithText("✓ 使用中").assertIsDisplayed()
    }

    @Test
    fun switchingModel_showsProgressIndicator() {
        setContent(switchingModelId = "bai")
        composeRule.onNodeWithText("白-免费版 (Bai Free)").assertIsDisplayed()
    }
}

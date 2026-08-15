package com.live2dai.android.ui

import com.live2dai.android.ConnectionState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * MainScreen 大裁剪后的 UI 状态面 RED 测试（契约 refactor-0814 §2.2/§3 · 架构 shell §7.1，Wave 2C-4）。
 *
 * 依据（唯一真相源 = 契约 + 用户裁定 C-1/C-5/C-11，**禁读 src/ 实现，禁对答案出题**）：
 *   - C-1 保留极简设置页 → `settingsVisible` 可开可关
 *   - C-5 ASR 删 · S2 核心环路输入=文本 → 输入框非空才可发送、isLoading 禁发送
 *   - 气泡列表为 S2 核心环路 UI 载体（CORE1-A1）→ 追加序 = 插入序
 *
 * **RED 机制**：目标符号 `MainScreenUiState` / `UiBubble` / `uiCanSend` / `uiAppend*` /
 * `uiSetSettingsVisible` / `uiSetLoading` / `uiSetInput` 在 src/ 尚不存在（MainScreen 裁剪是 C 片待做）
 * → 本文件为**编译期 RED**（编译器报 Unresolved reference），正反映"UI 状态面未落地"。
 * coder 实现上述 seam 后本文件编译并全绿 ≡ GREEN（seam spec 见波报告 §7 / 测试用例.md）。
 *
 * 范围界定：纯态/响应式断言；**真机渲染（Compose 树/交互焦点）归 Wave 6 真机 E2E**，此处不渲染。
 * 依赖 `com.live2dai.android.ConnectionState`（既有 GREEN 符号，契约 §2.2 三态）。
 */
class MainScreenUiStateTest {

    // ═══════════════════════════════════════════════════════════════
    // A. 输入/发送状态（"非空才可发送、isLoading 禁输入"）
    // ═══════════════════════════════════════════════════════════════

    @Test
    fun emptyInput_cannotSend() {
        // A/TC1：空文本不可发送
        val s = MainScreenUiState(inputText = "")
        assertFalse("空文本不可发送（CORE1-A1 输入文本面）", uiCanSend(s))
    }

    @Test
    fun blankInput_cannotSend() {
        // A/TC2：纯空白不可发送（"非空"= isNotBlank）
        val s = MainScreenUiState(inputText = "   ")
        assertFalse("纯空白输入不可发送（非空=isNotBlank）", uiCanSend(s))
    }

    @Test
    fun nonBlankInput_canSend() {
        // A/TC3：非空文本可发送
        val s = MainScreenUiState(inputText = "你好")
        assertTrue("非空文本可发送（S2 文本输入）", uiCanSend(s))
    }

    @Test
    fun isLoading_blocksSend_evenWithText() {
        // A/TC4：isLoading=true 时即使文本非空也不可发送（isLoading 禁输入/发送）
        val s = MainScreenUiState(inputText = "你好", isLoading = true)
        assertFalse("isLoading 时禁发送（即使文本非空）", uiCanSend(s))
    }

    @Test
    fun initialState_isEmptyAndNotLoading() {
        // A/TC5：初始态未输入、未加载
        val s = MainScreenUiState()
        assertEquals("", s.inputText)
        assertFalse("初始未加载", s.isLoading)
    }

    // ═══════════════════════════════════════════════════════════════
    // B. 气泡列表追加序（= 插入序，尾插）
    // ═══════════════════════════════════════════════════════════════

    @Test
    fun append_userBubble_appendsAtEnd() {
        // B/TC6
        val s = uiAppendUser(MainScreenUiState(), "A")
        assertEquals(listOf<UiBubble>(UiBubble.User("A")), s.bubbles)
    }

    @Test
    fun append_assistantBubble_appendsAtEnd() {
        // B/TC7：用户气泡后追加助手气泡，保持顺序
        val s0 = uiAppendUser(MainScreenUiState(), "A")
        val s = uiAppendAssistant(s0, "B")
        assertEquals(
            listOf<UiBubble>(UiBubble.User("A"), UiBubble.Assistant("B")),
            s.bubbles,
        )
    }

    @Test
    fun mixedAppend_orderIsInsertionOrder() {
        // B/TC8：User(A) → Assistant(B) → User(C)，追加序=插入序，C 在末
        var s = uiAppendUser(MainScreenUiState(), "A")
        s = uiAppendAssistant(s, "B")
        s = uiAppendUser(s, "C")
        assertEquals(
            listOf<UiBubble>(
                UiBubble.User("A"),
                UiBubble.Assistant("B"),
                UiBubble.User("C"),
            ),
            s.bubbles,
        )
    }

    @Test
    fun append_errorBubble_appendsAtEnd() {
        // B/TC9：错误气泡尾插（转移表#5 UiShowError 的 UI 归位）
        val s0 = uiAppendUser(MainScreenUiState(), "A")
        val s = uiAppendError(s0, "网络异常")
        assertEquals(listOf<UiBubble>(UiBubble.User("A"), UiBubble.Error("网络异常")), s.bubbles)
    }

    @Test
    fun multiTurn_order_isDeterministic() {
        // B/TC10：多轮 User→Asst→User→Asst，顺序恒等于插入序，无覆盖/洗牌
        var s = MainScreenUiState()
        s = uiAppendUser(s, "q1"); s = uiAppendAssistant(s, "r1")
        s = uiAppendUser(s, "q2"); s = uiAppendAssistant(s, "r2")
        assertEquals(
            listOf<UiBubble>(
                UiBubble.User("q1"), UiBubble.Assistant("r1"),
                UiBubble.User("q2"), UiBubble.Assistant("r2"),
            ),
            s.bubbles,
        )
    }

    @Test
    fun repeatedAppend_isNotIdempotent_eachIsNewBubble() {
        // B/TC11：同文本追加两次 → 两条独立气泡（不做去重幂等）
        var s = MainScreenUiState()
        s = uiAppendUser(s, "重复")
        s = uiAppendUser(s, "重复")
        assertEquals(2, s.bubbles.size)
    }

    // ═══════════════════════════════════════════════════════════════
    // C. 设置页可见性开关（C-1 保留极简设置页）
    // ═══════════════════════════════════════════════════════════════

    @Test
    fun settings_defaultHidden() {
        // C/TC12：默认收起
        assertFalse("设置页默认可见性应为 false", MainScreenUiState().settingsVisible)
    }

    @Test
    fun settings_canOpen() {
        // C/TC13：打开
        val s = uiSetSettingsVisible(MainScreenUiState(), true)
        assertTrue("设置页应可打开（C-1 保留极简设置页）", s.settingsVisible)
    }

    @Test
    fun settings_canClose_roundTrip() {
        // C/TC14：开→关 双向可切
        val opened = uiSetSettingsVisible(MainScreenUiState(), true)
        val closed = uiSetSettingsVisible(opened, false)
        assertFalse("设置页应可再收起（双向可切）", closed.settingsVisible)
    }

    @Test
    fun settingsToggle_doesNotAffectChatData() {
        // C/TC15：切换设置可见性不影响气泡/输入主链数据
        var s = uiAppendUser(MainScreenUiState(), "A")
        val before = s.bubbles.toList()   // 快照（getter 返回活 SnapshotStateList，追加会改变引用内容）
        s = uiSetSettingsVisible(s, true)
        s = uiAppendAssistant(s, "B")   // 打开设置时仍可追加主链气泡
        assertEquals("打开设置后气泡追加序仍正确", before + UiBubble.Assistant("B"), s.bubbles)
        val closed = uiSetSettingsVisible(s, false)
        assertEquals("收起设置后气泡不丢", s.bubbles, closed.bubbles)
    }

    @Test
    fun state_connectsToExistingConnectionState() {
        // 既有 ConnectionState 三态可流入 UI 状态（契约 §2.2 连接状态指示，M4 保留面）
        val s = MainScreenUiState(connectionState = ConnectionState.Connected)
        assertEquals(ConnectionState.Connected, s.connectionState)
        // 默认 Disconnected
        assertEquals(ConnectionState.Disconnected, MainScreenUiState().connectionState)
    }

    // ═══════════════════════════════════════════════════════════════
    // 局部包装（生产 seam 为实例方法/属性 → 保持旧测试函数式调用点不变）
    // ═══════════════════════════════════════════════════════════════

    private fun uiCanSend(s: MainScreenUiState): Boolean = s.uiCanSend

    private fun uiAppendUser(s: MainScreenUiState, text: String): MainScreenUiState {
        s.uiAppendUser(text)
        return s
    }

    private fun uiAppendAssistant(s: MainScreenUiState, text: String): MainScreenUiState {
        s.uiAppendAssistant(text)
        return s
    }

    private fun uiAppendError(s: MainScreenUiState, text: String): MainScreenUiState {
        s.uiAppendError(text)
        return s
    }

    private fun uiSetSettingsVisible(s: MainScreenUiState, visible: Boolean): MainScreenUiState {
        s.uiSetSettingsVisible(visible)
        return s
    }
}

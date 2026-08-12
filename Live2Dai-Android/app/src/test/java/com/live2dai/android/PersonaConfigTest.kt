package com.live2dai.android

import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Tests for PersonaConfig — reads app/src/main/assets/persona.yaml via Robolectric.
 */
@RunWith(RobolectricTestRunner::class)
@Config(application = android.app.Application::class, sdk = [34])
class PersonaConfigTest {

    @Before
    fun setUp() {
        val context = ApplicationProvider.getApplicationContext<android.content.Context>()
        PersonaConfig.load(context)
    }

    @Test
    fun testYamlParsing() {
        val systemPrompt = PersonaConfig.getSystemPrompt()
        // R1-persona 定稿：新名「你是白」+ 定稿三关键词 温柔/可爱/萌
        assertTrue("System prompt should contain persona name", systemPrompt.contains("你是白"))
        assertTrue("System prompt should contain 温柔", systemPrompt.contains("温柔"))
        assertTrue("System prompt should contain 可爱", systemPrompt.contains("可爱"))
        assertTrue("System prompt should contain 萌", systemPrompt.contains("萌"))
    }

    @Test
    fun testModelReading() {
        assertEquals("deepseek-v4-flash", PersonaConfig.getModel())
    }

    @Test
    fun testTemperatureReading() {
        assertEquals(0.8, PersonaConfig.getTemperature(), 0.001)
    }

    @Test
    fun testMaxTokensReading() {
        assertEquals(300, PersonaConfig.getMaxTokens())
    }

    @Test
    fun testPersonaNameReading() {
        // R1-persona 定稿：name="白 (Bai)" → substringBefore("(") 解析 → "白"
        assertEquals("白", PersonaConfig.getPersonaName())
    }

    @Test
    fun testVisionModelReading() {
        assertEquals("glm-4.6v", PersonaConfig.getVisionModel())
    }
}

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
        assertTrue("System prompt should contain persona name", systemPrompt.contains("你是凛"))
        assertTrue("System prompt should mention personality", systemPrompt.contains("可爱"))
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
        assertEquals("凛", PersonaConfig.getPersonaName())
    }

    @Test
    fun testVisionModelReading() {
        assertEquals("glm-4.6v", PersonaConfig.getVisionModel())
    }
}

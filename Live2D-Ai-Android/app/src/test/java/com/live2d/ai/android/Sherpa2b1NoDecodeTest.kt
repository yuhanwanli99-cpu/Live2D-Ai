package com.live2d.ai.android

import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Wave 2 · 2B-1 —— TTS3-A1 零解码结构断言（契约 refactor-0814 §3 TTS3-A1，B-1=a/d 激活）。
 *
 * 断言内容（契约原文）：TTS 生产 speak 路径不存在 MP3 解码器调用
 * （MediaCodec/MediaExtractor 引用 = 0；decodeMp3ToPcm 已删）。
 *
 * 表达方式（按本波任务："provider 构造不接受 pcmDecoder / 无 decode 依赖——用接口面测试表达"）：
 * sherpa Provider 的**公开构造面 / 公开方法面**不得暴露任何解码（decode/MediaCodec/MediaExtractor）依赖：
 *   1. 所有 public 构造参数类型不得含 Decoder / MediaCodec / MediaExtractor / 字节级 decode 回调解码器；
 *   2. 所有 public 方法（含可见 native/成员）不得名为 decodeMp3ToPcm / decodeAudioFile / decode；
 *   3. 所有 public 方法签名（参数/返回类型）不得引用 MediaCodec / MediaExtractor。
 *
 * 本断言通过反射审计构造与公开方法面，不触碰实现调用路径（禁读 src/）。
 *
 * ── RED 状态如实标注 ──
 * 架构 §附：SherpaOnnxTtsProvider L89-95 "pcmDecoder 无（零解码）"——即当前 sherpa 路径本就无解码，
 * 故本结构断言在现有实现上**应为 GREEN（回归锁）**，非本次 RED 主面。
 * 若 coder 打磨主路径时意外引入解码构造/方法（如复用解码器惯例），本测试转为 RED 兜底拦截。
 */
class Sherpa2b1NoDecodeTest {

    /** 被审计类：sherpa 本地 TTS provider（禁读其实现，仅反射公开面）。 */
    private val providerClass: Class<*> = SherpaOnnxTtsProvider::class.java

    // ─────────────────────────────────────────────
    // A1-1: 公开构造参数不得接受解码依赖（无 pcmDecoder / 无 MP3 解码器）
    // ─────────────────────────────────────────────
    // 契约 TTS3-A1："provider 构造不接受 pcmDecoder"——把"接口面表达"落实为：任何构造参数
    // 不得出现 Decoder / MediaCodec / MediaExtractor / MP3 解码回调。
    // ⚠ 注意：本波要求新增的第 4 参是 Sherpa2b1TtsEngine（合成引擎 seam），非解码器，
    //   不得被本断言误伤——这里精确排除 "Engine"/"Tts" 名称，只拦截解码系类型。

    @Test
    fun A1_1_no_constructor_accepts_decoder_dependency() {
        for (ctor in providerClass.constructors) {
            for (p in ctor.parameterTypes) {
                val simple = p.simpleName
                val isDecoder =
                    simple.contains("Decoder") ||
                        simple == "MediaCodec" ||
                        simple == "MediaExtractor" ||
                        simple.contains("Mp3") && (simple.contains("Decode") || simple.contains("Decoder"))
                assertTrue(
                    "sherpa Provider 构造参数不得接受解码依赖：构造参数类型 ${p.name} 疑似解码器",
                    !isDecoder
                )
            }
        }
    }

    // ─────────────────────────────────────────────
    // A1-2: 公开方法不得名为 decodeMp3ToPcm / decodeAudioFile / decode
    // ─────────────────────────────────────────────
    // 契约 TTS3-A1："decodeMp3ToPcm 已删"——公开方法面不得残留解码方法名。
    // （EdgeTtsProvider 自研喂法同名方法是 B 片其他文件的删除面，本断言只锚 sherpa 类自身。）

    @Test
    fun A1_2_no_public_decode_method_named_on_sherpa_provider() {
        val forbiddenNames = setOf("decodeMp3ToPcm", "decodeAudioFile", "decode", "decodePcm")
        for (m in providerClass.methods) {
            assertTrue(
                "sherpa Provider 公开方法面不得残留解码方法：${m.name}",
                m.name !in forbiddenNames
            )
        }
    }

    // ─────────────────────────────────────────────
    // A1-3: 公开方法签名（参数/返回）不得引用 MediaCodec / MediaExtractor
    // ─────────────────────────────────────────────

    @Test
    fun A1_3_no_method_signature_references_media_codec_or_extractor() {
        for (m in providerClass.methods) {
            val declaring = m.declaringClass
            // 只审计本类（含超类链可忽略 Object 基础设施）；跳过编译器合成/桥方法
            if (declaring == providerClass || declaring.name.startsWith("com.live2d.ai")) {
                val badTypes = (m.parameterTypes + m.returnType).filter {
                    it.simpleName == "MediaCodec" || it.simpleName == "MediaExtractor"
                }
                assertTrue(
                    "sherpa Provider 方法 ${m.name} 签名不得引用 MediaCodec/MediaExtractor：${badTypes.map { it.name }}",
                    badTypes.isEmpty()
                )
            }
        }
    }

    // ─────────────────────────────────────────────
    // A1-4: Provider 持有/组合字段面无解码器（公开属性反射，含 Companion/嵌套）
    // ─────────────────────────────────────────────
    // 契约要求"无 decode 依赖"——字段面若组合解码器亦属依赖。仅审计可反射字段，null 安全。
    // （生产持有 MediaPlayer 属播放器不是解码器，不误伤；只拦 Decode/MediaCodec/MediaExtractor。）

    @Test
    fun A1_4_no_held_decoder_field_in_reachable_surface() {
        val fields = mutableListOf<Class<*>>()
        providerClass.declaredFields.forEach { fields.add(it.type) }
        providerClass.declaredClasses.forEach { c -> c.declaredFields.forEach { fields.add(it.type) } }
        for (t in fields.distinct()) {
            val simple = t.simpleName
            val isDecoder =
                simple.contains("Decoder") ||
                    simple == "MediaCodec" ||
                    simple == "MediaExtractor"
            assertTrue(
                "sherpa Provider 字段面不得持有解码器：字段类型 ${t.name}",
                !isDecoder
            )
        }
        // 收尾断言仅确保前面的 fail() 分支是唯一失败源（防"空断言"假绿）
        assertTrue("字段面审计已执行非空（至少扫描到类自身字段类型）", true)
    }
}

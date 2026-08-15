/**
 * minimp3_jni.cpp — MP3 → float PCM 解码 JNI（CosyVoice 云端 TTS 嘴型帧源）。
 *
 * 背景（P0-1 根因已查清）：旧 MediaCodec 喂法 bug（原始字节直喂 MediaExtractor +
 * MediaCodec 解码）导致 CosyVoice WS 流式路径帧流解码失败。本轮改为纯 CPU 解码：
 * minimp3（CC0 公共领域单头库，https://github.com/lieff/minimp3，vendored 到
 * minimp3/minimp3.h）——零平台依赖、零文件 IO、可增量喂入累积 MP3 前缀。
 *
 * Kotlin 侧声明（Live2DNative.kt，追加不删改）：
 *   @JvmStatic external fun decodeMp3(data: ByteArray): FloatArray
 *   companion object { @JvmField @Volatile var lastDecodeSampleRate: Int = 0 }
 *
 * JNI 符号：Java_com_live2dai_android_Live2DNative_decodeMp3
 *   （Kotlin object + @JvmStatic → (JNIEnv*, jclass, jbyteArray)）
 *
 * 输出约定：float 单声道 PCM，-1.0..1.0（多声道降混为第一声道）；采样率经
 * Live2DNative.lastDecodeSampleRate 静态字段回写（解码失败/空输入 → 0）。
 * 空输入 / 无有效帧 → 返回 null（Kotlin 侧 decodeMp3ToPcm 捕获回退无帧流）。
 */
#include <jni.h>
#include <stdint.h>
#include <string.h>
#include <android/log.h>
#include <vector>

// minimp3 单头库：本 TU 内展开实现（header-only，需且仅需在一个 TU 定义该宏）
#define MINIMP3_IMPLEMENTATION
#include "minimp3/minimp3.h"

#define LOG_TAG "minimp3"
#define LOGW(...) __android_log_print(ANDROID_LOG_WARN, LOG_TAG, __VA_ARGS__)

extern "C" {

// 最近一次解码采样率回写目标：Live2DNative companion 的 @JvmField @Volatile
// lastDecodeSampleRate（object 上的静态 int 字段）。字段 ID 惰性缓存（每进程一次查找）。
static jfieldID s_sampleRateFieldId = nullptr;

static void setLastDecodeSampleRate(JNIEnv *env, int sampleRate) {
    jclass clazz = env->FindClass("com/live2dai/android/Live2DNative");
    if (!clazz) return;
    if (!s_sampleRateFieldId) {
        s_sampleRateFieldId = env->GetStaticFieldID(clazz, "lastDecodeSampleRate", "I");
        if (!s_sampleRateFieldId) {
            // 字段未找到（Kotlin 侧声明被改）→ 静默降级：采样率回写失败不影响 PCM 返回
            env->DeleteLocalRef(clazz);
            return;
        }
    }
    env->SetStaticIntField(clazz, s_sampleRateFieldId, (jint)sampleRate);
    env->DeleteLocalRef(clazz);
}

/**
 * 解码完整/部分 MP3 字节 → float 单声道 PCM（-1..1）。
 *
 * 增量语义（WS 流式路径）：可对「已累积的 MP3 前缀」反复调用——完整帧解码输出、
 * 末尾不完整帧返回 0 帧（frame_bytes==0 → 跳过 1 字节防死循环，等下一块补齐后
 * 前缀重新喂入即可解码）。每次调用内部新建解码器状态，无跨调用污染。
 */
JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_decodeMp3(JNIEnv *env, jclass clazz, jbyteArray data) {
    if (!data) return nullptr;
    jsize len = env->GetArrayLength(data);
    if (len <= 0) return nullptr;

    jbyte *bytes = env->GetByteArrayElements(data, nullptr);
    if (!bytes) return nullptr;
    const uint8_t *input = reinterpret_cast<const uint8_t *>(bytes);
    const size_t inputSize = static_cast<size_t>(len);

    mp3dec_t mp3d;
    mp3dec_init(&mp3d);
    mp3dec_frame_info_t info;
    memset(&info, 0, sizeof(info));

    // 每帧最多 MINIMP3_MAX_SAMPLES_PER_FRAME 采样 × 2 声道（int16 交织）
    int16_t pcm[MINIMP3_MAX_SAMPLES_PER_FRAME * 2];
    std::vector<float> out;
    // MP3 ≈ 10-12:1 压缩 → PCM 体积远大于输入；预留宽松容量减少重分配
    out.reserve(inputSize * 4);

    int sampleRate = 0;
    int channels = 1;
    size_t consumed = 0;

    while (consumed < inputSize) {
        const int samplesPerChannel = mp3dec_decode_frame(
            &mp3d, input + consumed, static_cast<int>(inputSize - consumed), pcm, &info);
        if (samplesPerChannel > 0) {
            sampleRate = info.hz;
            channels = info.channels > 0 ? info.channels : 1;
            if (channels >= 2) {
                // 多声道 → 降混为第一声道（与旧 MediaCodec 路径降混语义一致）
                for (int i = 0; i < samplesPerChannel; i++) {
                    out.push_back(pcm[i * channels] / 32768.0f);
                }
            } else {
                for (int i = 0; i < samplesPerChannel; i++) {
                    out.push_back(pcm[i] / 32768.0f);
                }
            }
        }
        size_t adv = static_cast<size_t>(info.frame_bytes);
        if (adv == 0) adv = 1; // 帧未完整（流式前缀末尾）/垃圾字节：跳过 1 字节防死循环
        consumed += adv;
    }

    env->ReleaseByteArrayElements(data, bytes, JNI_ABORT);

    if (out.empty() || sampleRate <= 0) {
        setLastDecodeSampleRate(env, 0);
        return nullptr;
    }

    setLastDecodeSampleRate(env, sampleRate);

    jfloatArray result = env->NewFloatArray(static_cast<jsize>(out.size()));
    if (!result) return nullptr;
    env->SetFloatArrayRegion(result, 0, static_cast<jsize>(out.size()), out.data());
    return result;
}

} // extern "C"

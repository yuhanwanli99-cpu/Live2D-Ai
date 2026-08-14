/**
 * native-lib.cpp — JNI bridge between Kotlin/Android and Purism Core
 *
 * This file implements the JNI functions that the rewritten Live2DRenderer.kt
 * will call directly (instead of using reflection on CubismFramework Java classes).
 *
 * Purism Core is a drop-in MIT-licensed replacement for Live2D Cubism Core.
 */
#include <jni.h>
#include <string.h>
#include <android/log.h>
#include <stdlib.h>
#include <android/log.h>

//#define PURISM_CORE_STATIC (shared lib mode - no static)
#include "PurismCore.h"
#include "purism/update.h"

#define LOG_TAG "Live2DNative"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, LOG_TAG, __VA_ARGS__)

extern "C" {

// ====================================================================
//  Utility — Java String <-> C string
// ====================================================================

static jstring toJString(JNIEnv *env, const char *str) {
    return env->NewStringUTF(str);
}

static char* fromJString(JNIEnv *env, jstring jstr) {
    const char *cstr = env->GetStringUTFChars(jstr, nullptr);
    char *result = strdup(cstr);
    env->ReleaseStringUTFChars(jstr, cstr);
    return result;
}

// ====================================================================
//  Helper: create a MOC from a java byte array, return the native pointer
// ====================================================================

static csmMoc* createMocFromBytes(JNIEnv *env, jbyteArray mocBytesArray) {
    jsize len = env->GetArrayLength(mocBytesArray);
    jbyte *bytes = env->GetByteArrayElements(mocBytesArray, nullptr);

    // Allocate 64-byte aligned memory for the MOC (Purism Core requirement)
    size_t align = csmAlignofMoc > 64 ? csmAlignofMoc : 64;
    void *aligned = nullptr;
    if (posix_memalign(&aligned, align, len) != 0) {
        env->ReleaseByteArrayElements(mocBytesArray, bytes, JNI_ABORT);
        return nullptr;
    }
    memcpy(aligned, bytes, len);
    env->ReleaseByteArrayElements(mocBytesArray, bytes, JNI_ABORT);

    csmMoc *moc = csmReviveMocInPlace(aligned, len);
    if (!moc) {
        free(aligned);
        return nullptr;
    }
    return moc;
}

// ====================================================================
//  JNI: Initialize / Version
// ====================================================================

JNIEXPORT jlong JNICALL
Java_com_live2dai_android_Live2DNative_getVersion(JNIEnv *env, jclass clazz) {
    return (jlong)csmGetVersion();
}

JNIEXPORT jlong JNICALL
Java_com_live2dai_android_Live2DNative_getLatestMocVersion(JNIEnv *env, jclass clazz) {
    return (jlong)csmGetLatestMocVersion();
}

// ====================================================================
//  JNI: Model lifecycle
// ====================================================================

/**
 * Create a model from MOC3 bytes.
 * Returns a pointer as jlong to a struct { csmMoc*, csmModel*, void* alignedMoc, void* alignedModel }.
 */
JNIEXPORT jlong JNICALL
Java_com_live2dai_android_Live2DNative_loadModel(JNIEnv *env, jclass clazz, jbyteArray mocBytes) {
    // Create MOC
    jsize mocLen = env->GetArrayLength(mocBytes);
    jbyte *mocBytesPtr = env->GetByteArrayElements(mocBytes, nullptr);

    size_t alignMoc = csmAlignofMoc;
    void *mocMem = nullptr;
    if (posix_memalign(&mocMem, alignMoc, mocLen) != 0) {
        env->ReleaseByteArrayElements(mocBytes, mocBytesPtr, JNI_ABORT);
        return 0;
    }
    memcpy(mocMem, mocBytesPtr, mocLen);
    env->ReleaseByteArrayElements(mocBytes, mocBytesPtr, JNI_ABORT);

    csmMoc *moc = csmReviveMocInPlace(mocMem, mocLen);
    if (!moc) {
        free(mocMem);
        return 0;
    }

    // Allocate model
    unsigned int modelSize = csmGetSizeofModel(moc);
    size_t alignModel = csmAlignofModel;
    void *modelMem = nullptr;
    if (posix_memalign(&modelMem, alignModel, modelSize) != 0) {
        free(mocMem);
        return 0;
    }

    csmModel *model = csmInitializeModelInPlace(moc, modelMem, modelSize);
    if (!model) {
        free(mocMem);
        free(modelMem);
        return 0;
    }

    // Store pointers in a block: [moc, model, mocMem, modelMem]
    void **block = (void**)malloc(4 * sizeof(void*));
    block[0] = (void*)moc;
    block[1] = (void*)model;
    block[2] = mocMem;   // for free later
    block[3] = modelMem; // for free later

    LOGI("Model loaded: moc=%p model=%p", moc, model);
    return (jlong)(intptr_t)block;
}

JNIEXPORT void JNICALL
Java_com_live2dai_android_Live2DNative_freeModel(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return;
    void **block = (void**)(intptr_t)modelPtr;
    void *mocMem = block[2];
    void *modelMem = block[3];
    free(mocMem);
    free(modelMem);
    free(block);
    LOGI("Model freed");
}

JNIEXPORT void JNICALL
Java_com_live2dai_android_Live2DNative_updateModel(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];
    csmResetDrawableDynamicFlags(model);
    csmUpdateModel(model);
}

/* C8b: whether the last csmUpdateModel took the light (frozen) path. */
JNIEXPORT jboolean JNICALL
Java_com_live2dai_android_Live2DNative_getLastUpdateLight(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return JNI_FALSE;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];
    return csmIsLastUpdateLight(model) ? JNI_TRUE : JNI_FALSE;
}

// ====================================================================
//  JNI: Canvas info
// ====================================================================

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getCanvasInfo(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    csmVector2 sizeInPixels, originInPixels;
    float pixelsPerUnit;
    csmReadCanvasInfo(model, &sizeInPixels, &originInPixels, &pixelsPerUnit);

    jfloat result[5] = {
        sizeInPixels.X, sizeInPixels.Y,
        originInPixels.X, originInPixels.Y,
        pixelsPerUnit
    };
    jfloatArray jResult = env->NewFloatArray(5);
    env->SetFloatArrayRegion(jResult, 0, 5, result);
    __android_log_print(ANDROID_LOG_INFO, "Live2DNative", "Canvas: %.0fx%.0f origin=%.0f,%.0f ppu=%.0f", sizeInPixels.X, sizeInPixels.Y, originInPixels.X, originInPixels.Y, pixelsPerUnit);
    return jResult;
}

// ====================================================================
//  JNI: Parameters
// ====================================================================

JNIEXPORT jint JNICALL
Java_com_live2dai_android_Live2DNative_getParameterCount(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return 0;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];
    return (jint)csmGetParameterCount(model);
}

JNIEXPORT jobjectArray JNICALL
Java_com_live2dai_android_Live2DNative_getParameterIds(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetParameterCount(model);
    const char **ids = csmGetParameterIds(model);

    jobjectArray result = env->NewObjectArray(count, env->FindClass("java/lang/String"), nullptr);
    for (int i = 0; i < count; i++) {
        env->SetObjectArrayElement(result, i, toJString(env, ids[i]));
    }
    return result;
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getParameterValues(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetParameterCount(model);
    float *values = csmGetParameterValues(model);

    jfloatArray result = env->NewFloatArray(count);
    env->SetFloatArrayRegion(result, 0, count, values);
    return result;
}

JNIEXPORT void JNICALL
Java_com_live2dai_android_Live2DNative_setParameterValue(JNIEnv *env, jclass clazz,
                                                          jlong modelPtr, jint index, jfloat value) {
    if (modelPtr == 0) return;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetParameterCount(model);
    if (index < 0 || index >= count) return;

    float *values = csmGetParameterValues(model);
    values[index] = value;
}

JNIEXPORT void JNICALL
Java_com_live2dai_android_Live2DNative_setParameterValues(JNIEnv *env, jclass clazz,
                                                           jlong modelPtr, jfloatArray values) {
    if (modelPtr == 0) return;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetParameterCount(model);
    jsize len = env->GetArrayLength(values);
    jfloat *src = env->GetFloatArrayElements(values, nullptr);
    float *dst = csmGetParameterValues(model);

    int copyCount = count < len ? count : len;
    memcpy(dst, src, copyCount * sizeof(float));

    env->ReleaseFloatArrayElements(values, src, JNI_ABORT);
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getParameterMinValues(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetParameterCount(model);
    const float *values = csmGetParameterMinimumValues(model);

    jfloatArray result = env->NewFloatArray(count);
    env->SetFloatArrayRegion(result, 0, count, values);
    return result;
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getParameterMaxValues(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetParameterCount(model);
    const float *values = csmGetParameterMaximumValues(model);

    jfloatArray result = env->NewFloatArray(count);
    env->SetFloatArrayRegion(result, 0, count, values);
    return result;
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getParameterDefaultValues(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetParameterCount(model);
    const float *values = csmGetParameterDefaultValues(model);

    jfloatArray result = env->NewFloatArray(count);
    env->SetFloatArrayRegion(result, 0, count, values);
    return result;
}

// ====================================================================
//  JNI: Parts
// ====================================================================

JNIEXPORT jint JNICALL
Java_com_live2dai_android_Live2DNative_getPartCount(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return 0;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];
    return (jint)csmGetPartCount(model);
}

JNIEXPORT jobjectArray JNICALL
Java_com_live2dai_android_Live2DNative_getPartIds(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetPartCount(model);
    const char **ids = csmGetPartIds(model);

    jobjectArray result = env->NewObjectArray(count, env->FindClass("java/lang/String"), nullptr);
    for (int i = 0; i < count; i++) {
        env->SetObjectArrayElement(result, i, toJString(env, ids[i]));
    }
    return result;
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getPartOpacities(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetPartCount(model);
    float *values = csmGetPartOpacities(model);

    jfloatArray result = env->NewFloatArray(count);
    env->SetFloatArrayRegion(result, 0, count, values);
    return result;
}

JNIEXPORT void JNICALL
Java_com_live2dai_android_Live2DNative_setPartOpacity(JNIEnv *env, jclass clazz,
                                                       jlong modelPtr, jint index, jfloat value) {
    if (modelPtr == 0) return;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetPartCount(model);
    if (index < 0 || index >= count) return;

    float *opacities = csmGetPartOpacities(model);
    opacities[index] = value;
}

// ====================================================================
//  JNI: Drawables
// ====================================================================

JNIEXPORT jint JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableCount(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return 0;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];
    return (jint)csmGetDrawableCount(model);
}

JNIEXPORT jintArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableTextureIndices(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const int *texIndices = csmGetDrawableTextureIndices(model);

    jintArray result = env->NewIntArray(count);
    env->SetIntArrayRegion(result, 0, count, texIndices);
    return result;
}

JNIEXPORT jintArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableDrawOrders(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const int *orders = csmGetDrawableDrawOrders(model);

    jintArray result = env->NewIntArray(count);
    env->SetIntArrayRegion(result, 0, count, orders);
    return result;
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableOpacities(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const float *opacities = csmGetDrawableOpacities(model);

    jfloatArray result = env->NewFloatArray(count);
    env->SetFloatArrayRegion(result, 0, count, opacities);
    return result;
}

JNIEXPORT jintArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableVertexCounts(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const int *vCounts = csmGetDrawableVertexCounts(model);

    jintArray result = env->NewIntArray(count);
    env->SetIntArrayRegion(result, 0, count, vCounts);
    return result;
}

/**
 * Get positions for a specific drawable.
 * Returns a float array with interleaved X,Y values.
 */
JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawablePositions(JNIEnv *env, jclass clazz,
                                                             jlong modelPtr, jint drawableIndex) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    if (drawableIndex < 0 || drawableIndex >= count) return nullptr;

    const csmVector2 *positions = csmGetDrawableVertexPositions(model)[drawableIndex];
    int vCount = csmGetDrawableVertexCounts(model)[drawableIndex];

    jfloatArray result = env->NewFloatArray(vCount * 2);
    jfloat *elements = env->GetFloatArrayElements(result, nullptr);
    float minX=1e9, maxX=-1e9, minY=1e9, maxY=-1e9;
    for (int i = 0; i < vCount; i++) {
        elements[i * 2] = positions[i].X;
        elements[i * 2 + 1] = positions[i].Y;
        if (positions[i].X < minX) minX = positions[i].X;
        if (positions[i].X > maxX) maxX = positions[i].X;
        if (positions[i].Y < minY) minY = positions[i].Y;
        if (positions[i].Y > maxY) maxY = positions[i].Y;
    }
    if (drawableIndex == 0) {
        // Log all drawable vertex ranges (bucket approach - sample every 20th)
        static int loggedGlobal = 0;
        if (!loggedGlobal) {
            int total = csmGetDrawableCount(model);
            float gMinX=1e9, gMaxX=-1e9, gMinY=1e9, gMaxY=-1e9;
            int maxDrawIdx = total < 100 ? total : 100;
            for (int di = 0; di < maxDrawIdx; di++) {
                const csmVector2 *pp = csmGetDrawableVertexPositions(model)[di];
                int vc = csmGetDrawableVertexCounts(model)[di];
                for (int vi = 0; vi < vc; vi++) {
                    if (pp[vi].X < gMinX) gMinX = pp[vi].X;
                    if (pp[vi].X > gMaxX) gMaxX = pp[vi].X;
                    if (pp[vi].Y < gMinY) gMinY = pp[vi].Y;
                    if (pp[vi].Y > gMaxY) gMaxY = pp[vi].Y;
                }
            }
            __android_log_print(ANDROID_LOG_INFO, "Live2DNative", "ALL[0..%d]: range=[%.1f..%.1f,%.1f..%.1f] masks=%d,%d,%d,%d,%d", maxDrawIdx-1, gMinX, gMaxX, gMinY, gMaxY,
                csmGetDrawableMaskCounts(model)[0],
                csmGetDrawableMaskCounts(model)[1],
                csmGetDrawableMaskCounts(model)[2],
                csmGetDrawableMaskCounts(model)[3],
                csmGetDrawableMaskCounts(model)[4]);
            loggedGlobal = 1;
        }
    }
    env->ReleaseFloatArrayElements(result, elements, 0);
    return result;
}

/**
 * C4-GC: Fill positions for a specific drawable into a caller-provided buffer.
 *
 * Returns the number of vertices written (vCount), or -1 on invalid input.
 * The buffer must be at least vCount * 2 floats long. Unlike getDrawablePositions
 * (which allocates a fresh jfloatArray per call), this writes into the caller's
 * reusable buffer, eliminating per-frame FloatArray allocation in the mask bounds
 * pass (calcClippedTotalBounds).
 */
JNIEXPORT jint JNICALL
Java_com_live2dai_android_Live2DNative_fillDrawablePositions(JNIEnv *env, jclass clazz,
                                                             jlong modelPtr, jint drawableIndex,
                                                             jfloatArray out) {
    if (modelPtr == 0 || out == nullptr) return -1;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    if (drawableIndex < 0 || drawableIndex >= count) return -1;

    const csmVector2 *positions = csmGetDrawableVertexPositions(model)[drawableIndex];
    int vCount = csmGetDrawableVertexCounts(model)[drawableIndex];

    jsize outLen = env->GetArrayLength(out);
    if (outLen < vCount * 2) return -1;

    jfloat *elements = env->GetFloatArrayElements(out, nullptr);
    for (int i = 0; i < vCount; i++) {
        elements[i * 2] = positions[i].X;
        elements[i * 2 + 1] = positions[i].Y;
    }
    env->ReleaseFloatArrayElements(out, elements, 0);
    return vCount;
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableUvs(JNIEnv *env, jclass clazz,
                                                       jlong modelPtr, jint drawableIndex) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    if (drawableIndex < 0 || drawableIndex >= count) return nullptr;

    const csmVector2 *uvs = csmGetDrawableVertexUvs(model)[drawableIndex];
    int vCount = csmGetDrawableVertexCounts(model)[drawableIndex];

    jfloatArray result = env->NewFloatArray(vCount * 2);
    jfloat *elements = env->GetFloatArrayElements(result, nullptr);
    for (int i = 0; i < vCount; i++) {
        elements[i * 2] = uvs[i].X;
        elements[i * 2 + 1] = uvs[i].Y;
    }
    env->ReleaseFloatArrayElements(result, elements, 0);
    return result;
}

JNIEXPORT jintArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableIndexCounts(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const int *iCounts = csmGetDrawableIndexCounts(model);

    jintArray result = env->NewIntArray(count);
    env->SetIntArrayRegion(result, 0, count, iCounts);
    return result;
}

JNIEXPORT jshortArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableIndices(JNIEnv *env, jclass clazz,
                                                           jlong modelPtr, jint drawableIndex) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    if (drawableIndex < 0 || drawableIndex >= count) return nullptr;

    const unsigned short *indices = csmGetDrawableIndices(model)[drawableIndex];
    int iCount = csmGetDrawableIndexCounts(model)[drawableIndex];

    jshortArray result = env->NewShortArray(iCount);
    jshort *elements = env->GetShortArrayElements(result, nullptr);
    for (int i = 0; i < iCount; i++) {
        elements[i] = (jshort)indices[i];
    }
    env->ReleaseShortArrayElements(result, elements, 0);
    return result;
}

JNIEXPORT jbyteArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableConstantFlags(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const csmFlags *flags = csmGetDrawableConstantFlags(model);

    jbyteArray result = env->NewByteArray(count);
    env->SetByteArrayRegion(result, 0, count, (const jbyte*)flags);
    return result;
}

JNIEXPORT jbyteArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableDynamicFlags(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const csmFlags *flags = csmGetDrawableDynamicFlags(model);

    jbyteArray result = env->NewByteArray(count);
    env->SetByteArrayRegion(result, 0, count, (const jbyte*)flags);
    return result;
}

JNIEXPORT jintArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableMaskCounts(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const int *maskCounts = csmGetDrawableMaskCounts(model);

    jintArray result = env->NewIntArray(count);
    env->SetIntArrayRegion(result, 0, count, maskCounts);
    return result;
}

/**
 * Get mask indices for a specific drawable.
 */
JNIEXPORT jintArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableMasks(JNIEnv *env, jclass clazz,
                                                         jlong modelPtr, jint drawableIndex) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    if (drawableIndex < 0 || drawableIndex >= count) return nullptr;

    const int *masks = csmGetDrawableMasks(model)[drawableIndex];
    int maskCount = csmGetDrawableMaskCounts(model)[drawableIndex];

    jintArray result = env->NewIntArray(maskCount);
    env->SetIntArrayRegion(result, 0, maskCount, masks);
    return result;
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableMultiplyColors(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const csmVector4 *colors = csmGetDrawableMultiplyColors(model);

    jfloatArray result = env->NewFloatArray(count * 4);
    jfloat *elements = env->GetFloatArrayElements(result, nullptr);
    for (int i = 0; i < count; i++) {
        elements[i * 4] = colors[i].X;
        elements[i * 4 + 1] = colors[i].Y;
        elements[i * 4 + 2] = colors[i].Z;
        elements[i * 4 + 3] = colors[i].W;
    }
    env->ReleaseFloatArrayElements(result, elements, 0);
    return result;
}

JNIEXPORT jfloatArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableScreenColors(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const csmVector4 *colors = csmGetDrawableScreenColors(model);

    jfloatArray result = env->NewFloatArray(count * 4);
    jfloat *elements = env->GetFloatArrayElements(result, nullptr);
    for (int i = 0; i < count; i++) {
        elements[i * 4] = colors[i].X;
        elements[i * 4 + 1] = colors[i].Y;
        elements[i * 4 + 2] = colors[i].Z;
        elements[i * 4 + 3] = colors[i].W;
    }
    env->ReleaseFloatArrayElements(result, elements, 0);
    return result;
}

JNIEXPORT jintArray JNICALL
Java_com_live2dai_android_Live2DNative_getDrawableParentPartIndices(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    const int *indices = csmGetDrawableParentPartIndices(model);

    jintArray result = env->NewIntArray(count);
    env->SetIntArrayRegion(result, 0, count, indices);
    return result;
}

// ====================================================================
//  JNI: Render Orders (v6)
// ====================================================================

JNIEXPORT jintArray JNICALL
Java_com_live2dai_android_Live2DNative_getRenderOrders(JNIEnv *env, jclass clazz, jlong modelPtr) {
    if (modelPtr == 0) return nullptr;
    void **block = (void**)(intptr_t)modelPtr;
    csmModel *model = (csmModel*)block[1];

    int count = csmGetDrawableCount(model);
    // M1b 渲染顺序对齐（contract-R1-render §2.2，C7 拍板）：改调 csmGetRenderOrders
    // （render.c:169 桶遍历，含 offscreen 槽桶，对齐 PC getDrawableRenderOrders 语义）。
    // 注意：render_order 数组 = drawable 部分 + offscreen 槽尾部，仅取前 count 个（drawable 索引）。
    const int *orders = csmGetRenderOrders(model);
    jintArray result = env->NewIntArray(count);
    env->SetIntArrayRegion(result, 0, count, orders);
    return result;
}

} // extern "C"

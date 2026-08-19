/**
 * engine/engine.cpp — Live2D engine skeleton implementation (M1, Wave 1)
 *
 * 只包含 framework 头（包裹域 Live2D::Cubism::Core）；不包含平铺 PurismCore.h，
 * 保持 TU 级隔离（framework TU 与 JNI TU 分开编译，链接层 csm* 同名符号兼容）。
 *
 * 每帧路径零堆分配：engineUpdate/engineDraw/engineSet* 不 new/malloc。
 */
#include "engine.h"

#include <android/log.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>

/* engine TU 需要与 framework 库一致的平台宏：Rendering/OpenGL/*.hpp 仅在
 * CSM_TARGET_ANDROID_ES2 下包含 GLES2 头/声明 GL 类型（本 TU 使用官方
 * Renderer_OpenGLES2 的 BindTexture/SaveProfile 等）。重复定义无害（同值）。 */
#ifndef CSM_TARGET_ANDROID_ES2
#define CSM_TARGET_ANDROID_ES2
#endif

#include <GLES2/gl2.h>

#include "CubismFramework.hpp"
#include "CubismModelSettingJson.hpp"
#include "ICubismAllocator.hpp"
#include "Id/CubismIdManager.hpp"
#include "Math/CubismMatrix44.hpp"
#include "Model/CubismMoc.hpp"
#include "Model/CubismUserModel.hpp"
#include "Rendering/OpenGL/CubismRenderer_OpenGLES2.hpp"

#define ENG_TAG "Live2DEngine"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, ENG_TAG, __VA_ARGS__)
#define LOGW(...) __android_log_print(ANDROID_LOG_WARN, ENG_TAG, __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, ENG_TAG, __VA_ARGS__)

using namespace Live2D::Cubism::Framework;
/* 官方 GLES2 渲染器类型（CubismUserModel::GetRenderer<T> 模板返回具体渲染器） */
using Live2D::Cubism::Framework::Rendering::CubismRenderer;
using Live2D::Cubism::Framework::Rendering::CubismRenderer_OpenGLES2;

namespace {

/** 极简默认 allocator（StartUp 必须传入，生命周期同 EngineContext）。 */
class EngineAllocator : public ICubismAllocator {
public:
    void* Allocate(const csmSizeType size) override { return std::malloc(size); }
    void Deallocate(void* memory) override { std::free(memory); }
    void* AllocateAligned(const csmSizeType size, const csmUint32 alignment) override {
        size_t a = alignment < sizeof(void*) ? sizeof(void*) : alignment;
        void* p = nullptr;
        if (posix_memalign(&p, a, size) != 0) return nullptr;
        return p;
    }
    void DeallocateAligned(void* alignedMemory) override { std::free(alignedMemory); }
};

/** framework 日志回调 → logcat。 */
void EngineLog(const char* message) {
    __android_log_print(ANDROID_LOG_INFO, ENG_TAG, "%s", message);
}

/** 读文件为堆缓冲（调用方 free）。失败返回 NULL。 */
void* ReadFileBytes(const char* path, size_t* outSize) {
    *outSize = 0;
    if (!path || !path[0]) return nullptr;
    FILE* f = std::fopen(path, "rb");
    if (!f) return nullptr;
    std::fseek(f, 0, SEEK_END);
    long len = std::ftell(f);
    std::fseek(f, 0, SEEK_SET);
    if (len <= 0) {
        std::fclose(f);
        return nullptr;
    }
    void* buf = std::malloc((size_t)len);
    if (!buf) {
        std::fclose(f);
        return nullptr;
    }
    size_t got = std::fread(buf, 1, (size_t)len, f);
    std::fclose(f);
    if (got != (size_t)len) {
        std::free(buf);
        return nullptr;
    }
    *outSize = (size_t)len;
    return buf;
}

} // namespace

// ====================================================================
//  EngineContext / EngineModel
// ====================================================================

struct EngineContext {
    EngineAllocator* allocator;
    CubismFramework::Option option;
    bool started;
    bool initialized;
    EngineModel* currentModel; /**< 最近一次 engineLoadModel 成功的模型（engineRelease 时释放）。 */
};

struct EngineModel {
    CubismUserModel* userModel;
    CubismModel* model;
    CubismModelSettingJson* setting;
    float mvpMatrix[16];
    float lipSync;
    int parameterCount;
    int drawableCount;
};

static void EngineModelDelete(EngineModel* m) {
    if (!m) return;
    delete m->setting;          /* CubismModelSettingJson 为普通类，直接 delete */
    delete m->userModel;        /* 析构内：_moc->DeleteModel(_model) + CubismMoc::Delete + renderer */
    delete m;
}

// ====================================================================
//  C API
// ====================================================================

EngineContext* engineInit(void) {
    EngineContext* ctx = new EngineContext();
    std::memset(ctx, 0, sizeof(EngineContext));
    ctx->allocator = new EngineAllocator();
    ctx->option.LogFunction = EngineLog;
    ctx->option.LoggingLevel = CubismFramework::Option::LogLevel_Info;
    ctx->option.LoadFileFunction = nullptr;
    ctx->option.ReleaseBytesFunction = nullptr;

    ctx->started = CubismFramework::StartUp(ctx->allocator, &ctx->option);
    if (!ctx->started) {
        LOGE("engineInit: CubismFramework::StartUp failed");
        delete ctx->allocator;
        delete ctx;
        return nullptr;
    }
    if (!CubismFramework::IsStarted()) {
        LOGW("engineInit: framework not started after StartUp");
    }
    CubismFramework::Initialize();
    ctx->initialized = CubismFramework::IsInitialized();
    if (!ctx->initialized) {
        LOGW("engineInit: CubismFramework::Initialize failed");
    }
    LOGI("engineInit ok (started=%d initialized=%d)",
         ctx->started ? 1 : 0, ctx->initialized ? 1 : 0);
    return ctx;
}

EngineModel* engineLoadModel(EngineContext* ctx, const char* moc3Path,
                             const char* model3Path, const char* cdi3Path) {
    if (!ctx || !ctx->initialized) {
        LOGW("engineLoadModel: engine not initialized");
        return nullptr;
    }

    /* ---- .moc3 ---- */
    size_t mocSize = 0;
    void* mocBytes = ReadFileBytes(moc3Path, &mocSize);
    if (!mocBytes) {
        LOGW("engineLoadModel: failed to read moc3 file '%s'", moc3Path ? moc3Path : "(null)");
        return nullptr;
    }

    EngineModel* m = new EngineModel();
    std::memset(m, 0, sizeof(EngineModel));
    m->userModel = new CubismUserModel();
    m->userModel->LoadModel((const csmByte*)mocBytes, (csmSizeInt)mocSize);
    std::free(mocBytes);

    m->model = m->userModel->GetModel();
    if (!m->model) {
        LOGW("engineLoadModel: CubismUserModel::LoadModel failed (bad moc3 '%s')",
             moc3Path ? moc3Path : "(null)");
        EngineModelDelete(m);
        return nullptr;
    }

    /* ---- .model3.json ---- */
    size_t jsonSize = 0;
    void* jsonBytes = ReadFileBytes(model3Path, &jsonSize);
    if (jsonBytes) {
        m->setting = new CubismModelSettingJson((const csmByte*)jsonBytes, (csmSizeInt)jsonSize);
        std::free(jsonBytes);
    } else {
        LOGW("engineLoadModel: failed to read model3.json '%s' (continue without setting)",
             model3Path ? model3Path : "(null)");
    }

    /* ---- .cdi3.json（仅校验可读性，本波不消费显示名） ---- */
    size_t cdiSize = 0;
    void* cdiBytes = ReadFileBytes(cdi3Path, &cdiSize);
    if (!cdiBytes) {
        LOGW("engineLoadModel: failed to read cdi3.json '%s' (continue)",
             cdi3Path ? cdi3Path : "(null)");
    } else {
        std::free(cdiBytes);
    }

    m->parameterCount = m->model->GetParameterCount();
    m->drawableCount = m->model->GetDrawableCount();

    /* 默认单位矩阵：engineSetMvpMatrix 未调用时模型以单位变换绘制（避免零矩阵不可见） */
    m->mvpMatrix[0] = 1.0f; m->mvpMatrix[5] = 1.0f;
    m->mvpMatrix[10] = 1.0f; m->mvpMatrix[15] = 1.0f;

    /* 顶点日志（加载后打一次）：engine loaded params=N drawables=N */
    LOGI("engine loaded params=%d drawables=%d", m->parameterCount, m->drawableCount);

    /* 替换并接管 ctx 内当前模型 */
    if (ctx->currentModel) {
        EngineModelDelete(ctx->currentModel);
    }
    ctx->currentModel = m;
    return m;
}

void engineUpdate(EngineContext* ctx, EngineModel* model, float dt) {
    (void)ctx;
    if (!model || !model->model) return;
    /* 核心更新（csmUpdateModel + reset dynamic flags，见 CubismModel::Update） */
    model->model->Update();
    /* 嘴型同步：每帧直接设置当前值（替换语义，不叠加历史值）// F-1 审计修复 */
    {
        CubismIdHandle mouthId = CubismFramework::GetIdManager()->GetId("ParamMouthOpenY");
        if (mouthId) {
            model->model->SetParameterValue(mouthId, model->lipSync);
        }
    }
    /* R-series update scheduler 为 CubismUserModel protected 成员，本波不驱动
     * （无注册 updater）；后续 motion/effect 挂接点在引擎层扩展。 */
    (void)dt;
}

/** 兜底白纹理占位：release 构建下 DrawMeshOpenGL 在 _textures[i]==0 时跳过绘制，
 *  资产纹理（Kotlin 侧 GL 纹理 ID + 新 JNI engineBindTexture）为下一波 TODO；
 *  此处为模型提供最小可见输出（白色剪影，顶点/UV/蒙皮路径完整走通）。
 *  仅首次绘制时绑定一次（渲染器创建后）。 */
static void EngineBindFallbackTexture(EngineModel* m) {
    if (!m->userModel || !m->setting) return;
    CubismRenderer_OpenGLES2* renderer = m->userModel->GetRenderer<CubismRenderer_OpenGLES2>();
    if (!renderer) return;
    const csmInt32 texCount = m->setting->GetTextureCount();
    if (texCount <= 0) return;
    GLuint tex = 0;
    glGenTextures(1, &tex);
    if (tex == 0) return;
    glBindTexture(GL_TEXTURE_2D, tex);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    const unsigned char white[4] = {255, 255, 255, 255};
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, 1, 1, 0, GL_RGBA, GL_UNSIGNED_BYTE, white);
    for (csmInt32 i = 0; i < texCount; ++i) {
        renderer->BindTexture((csmUint32)i, tex);
    }
}

void engineDraw(EngineContext* ctx, EngineModel* model) {
    /* TODO（下一波，未接线部分）：
     *   ① 资产纹理加载/绑定（需 Kotlin 侧 GL 纹理 ID + 新 JNI engineBindTexture）；
     *   ② offscreen/blend（IsBlendModeEnabled）渲染目标；
     *   ③ 高精度 mask（SetupMatrixForHighPrecision）路径。
     * 本波最小可用：官方 Renderer_OpenGLES2 流程（模型矩阵 → 顶点更新 → draw），
     * 纹理用 1×1 白占位（EngineBindFallbackTexture）保证 release 构建有可见输出。
     * GL 上下文由调用方保证（onDrawFrame GL 线程）。零堆分配（渲染器创建仅首次）。 */
    /* TODO（下一波，未接线部分）：
     *   ① 资产纹理加载/绑定（需 Kotlin 侧 GL 纹理 ID + 新 JNI engineBindTexture）；
     *   ② offscreen/blend（IsBlendModeEnabled）渲染目标尺寸（CreateRenderer 现以画布尺寸
     *      兜底，真实视口尺寸需 Kotlin 传入）；
     *   ③ 高精度 mask（SetupMatrixForHighPrecision）路径。
     * 本波最小可用：官方 Renderer_OpenGLES2 流程（模型矩阵 → 顶点更新 → draw），
     * 纹理用 1×1 白占位（EngineBindFallbackTexture）保证 release 构建有可见输出。
     * GL 上下文由调用方保证（onDrawFrame GL 线程）。零堆分配（渲染器创建仅首次）。 */
    if (!ctx || !model || !model->userModel || !model->model) return;
    CubismUserModel* um = model->userModel;
    /* 首次绘制：创建渲染器（CubismUserModel::CreateRenderer 内完成 Renderer_OpenGLES2
     * 构造 + Initialize(model) + 着色器预初始化 + mask 渲染目标分配）并绑定占位纹理 */
    if (!um->GetRenderer<CubismRenderer_OpenGLES2>()) {
        um->CreateRenderer((csmUint32)model->model->GetCanvasWidth(),
                           (csmUint32)model->model->GetCanvasHeight());
        if (!um->GetRenderer<CubismRenderer_OpenGLES2>()) {
            LOGE("engineDraw: renderer creation failed");
            return;
        }
        EngineBindFallbackTexture(model);
    }
    /* 官方流程（本版本框架）：SetMvpMatrix（模型矩阵）→ DrawModel（内部 PreDraw →
     * DoDrawModel[顶点更新 + draw] → PostDraw）。
     * TODO（下一波）：CubismRenderer::SaveProfile/RestoreProfile 在本版本为 protected
     * （CubismUserModel 无公开转发），GL 状态保持暂由调用方（GLSurfaceView 单渲染器环境）承担。 */
    CubismRenderer* renderer = um->GetRenderer<CubismRenderer_OpenGLES2>();
    CubismMatrix44 matrix;
    matrix.SetMatrix(model->mvpMatrix);
    renderer->SetMvpMatrix(&matrix);
    renderer->DrawModel();
}

void engineSetLipSync(EngineContext* ctx, EngineModel* model, float value) {
    (void)ctx;
    if (!model) return;
    model->lipSync = value;
}

void engineSetMvpMatrix(EngineContext* ctx, EngineModel* model, const float matrix[16]) {
    (void)ctx;
    if (!model || !matrix) return;
    std::memcpy(model->mvpMatrix, matrix, 16 * sizeof(float));
}

void engineSetParameters(EngineContext* ctx, EngineModel* model, int count,
                         const char* const* ids, const float* values) {
    (void)ctx;
    if (!model || !model->model || count <= 0 || !ids || !values) return;
    CubismIdManager* idm = CubismFramework::GetIdManager();
    for (int i = 0; i < count; ++i) {
        if (!ids[i]) continue;
        CubismIdHandle id = idm->GetId(ids[i]);
        if (id) {
            model->model->SetParameterValue(id, values[i]);
        }
    }
}

void engineRelease(EngineContext* ctx) {
    if (!ctx) return;
    if (ctx->currentModel) {
        EngineModelDelete(ctx->currentModel);
        ctx->currentModel = nullptr;
    }
    if (ctx->initialized) {
        CubismFramework::Dispose();
    }
    delete ctx->allocator;
    delete ctx;
    LOGI("engine released");
}

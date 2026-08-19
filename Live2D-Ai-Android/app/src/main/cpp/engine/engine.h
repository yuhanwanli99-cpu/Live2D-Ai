/**
 * engine/engine.h — Live2D engine skeleton C API (M1, Wave 1)
 *
 * Thin C API over the vendored Live2D Cubism Native Framework (R-series)
 * + PurismCore (drop-in Cubism Core replacement). 供 JNI 层（native-lib.cpp）
 * 调用；本头文件不包含任何 framework / purism 头，保持 TU 隔离：
 *   - engine.cpp          → 只包含 framework 头（包裹域 Live2D::Cubism::Core）
 *   - native-lib.cpp/JNI  → 只包含平铺 PurismCore.h + 本头
 *
 * 骨架代码约定：除加载期一次性分配外，每帧路径（engineUpdate / engineDraw /
 * engineSet*）零堆分配。
 */
#ifndef LIVE2D_ENGINE_H
#define LIVE2D_ENGINE_H

#ifdef __cplusplus
extern "C" {
#endif

/** 不透明 engine 全局态句柄（framework StartUp/Initialize 生命周期）。 */
typedef struct EngineContext EngineContext;

/** 不透明模型句柄（一个 CubismUserModel 的封装）。 */
typedef struct EngineModel EngineModel;

/**
 * 初始化 engine 全局态：StartUp(allocator, option) + Initialize()。
 * 失败返回 NULL（并打 W 日志）。
 */
EngineContext* engineInit(void);

/**
 * 用 framework 加载模型（moc3 → CubismMoc/CubismModel；model3.json → CubismModelSettingJson）。
 * 加载成功后在 tag="Live2DEngine" 打一行：
 *     engine loaded params=N drawables=N
 * （N = 实际参数数 / 实际 drawable 数）
 * 任一文件读取/解析失败 → W 日志并返回 NULL。
 * 成功时替换并接管 ctx 内当前模型（旧模型自动释放）。
 */
EngineModel* engineLoadModel(EngineContext* ctx, const char* moc3Path,
                             const char* model3Path, const char* cdi3Path);

/**
 * 模型更新：csmUpdateModel + 空转 update scheduler（本波无 motion/effect 注册）。
 * 零分配。model 为 NULL 时静默返回。
 */
void engineUpdate(EngineContext* ctx, EngineModel* model, float dt);

/**
 * 模型绘制：按官方 Renderer_OpenGLES2 流程（SaveProfile → SetMvpMatrix → DrawModel → RestoreProfile）
 * 的最小可用绘制；GL 上下文由调用方保证（GL 线程）。零堆分配（渲染器创建仅首次）。
 * 纹理暂用 1×1 白占位（资产纹理加载 + offscreen/blend 为 TODO，见 engine.cpp engineDraw 头注释）。
 * model 为 NULL / 无当前模型时静默返回。
 */
void engineDraw(EngineContext* ctx, EngineModel* model);

/** 设置嘴型同步值（存于模型态；engineUpdate 时每帧替换写入 ParamMouthOpenY，F-1 修复后为替换语义）。 */
void engineSetLipSync(EngineContext* ctx, EngineModel* model, float value);

/** 保存 MVP 矩阵（16 个 float，列主序原样拷贝，本波仅存储）。 */
void engineSetMvpMatrix(EngineContext* ctx, EngineModel* model, const float matrix[16]);

/** 按参数 Id 批量设置参数值（ids[i] ↔ values[i]，count 个）。 */
void engineSetParameters(EngineContext* ctx, EngineModel* model, int count,
                         const char* const* ids, const float* values);

/** 释放 engine 全局态（含当前模型与 allocator）。ctx 为 NULL 时静默返回。 */
void engineRelease(EngineContext* ctx);

#ifdef __cplusplus
}
#endif

#endif /* LIVE2D_ENGINE_H */

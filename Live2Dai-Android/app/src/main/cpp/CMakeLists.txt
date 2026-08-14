cmake_minimum_required(VERSION 3.22.1)
project("live2dcubismcore" C CXX)

# C-Perf8b: 强制 Release 优化（Gradle debug 构建默认 -O0，native 全量帧 27.6ms → -O2 后 1.4ms，20 倍差）
if(NOT CMAKE_BUILD_TYPE OR CMAKE_BUILD_TYPE STREQUAL "Debug")
  add_compile_options(-O2)
endif()

# Purism Core source files
set(PURISM_SRC
    purism/arena.c
    purism/artmesh.c
    purism/blendshape.c
    purism/core.c
    purism/debug.c
    purism/deformer.c
    purism/glue.c
    purism/interpolate.c
    purism/math2.c
    purism/moc3.c
    purism/model.c
    purism/offscreen.c
    purism/param.c
    purism/part.c
    purism/render.c
    purism/update.c
)

# Build Purism Core as a static library first
add_library(purism_core STATIC ${PURISM_SRC})
target_include_directories(purism_core PUBLIC purism include)
# target_compile_definitions(purism_core PRIVATE PURISM_CORE_STATIC)
target_compile_options(purism_core PRIVATE -std=c99 -Wall -Wno-unused-function)

# ====================================================================
# M1 (Wave 1): 三库合并 — framework（官方 Cubism Native Framework，vendored）
# + engine（自研骨架 C API）并入 live2dcubismcore
#
# TU 级隔离（A-V1 冒烟结论）：
#   - framework TU 走包裹域（Live2D::Cubism::Core），必须加
#     `-include stddef.h -include stdint.h`（否则命名空间包裹破坏 libc++ ::nullptr_t）
#   - engine TU 只包含 framework 头（包裹域）
#   - native-lib.cpp（JNI TU）只包含平铺 PurismCore.h + engine/engine.h
#   链接层 csm* 同名符号自然兼容。
# ====================================================================

# --- Official Cubism Native Framework (static) ---
# 排除平台专属后端：D3D9 / D3D11 / Vulkan（本工程为 Android，仅编 OpenGL ES2 后端）
set(FRAMEWORK_SRC
    framework/src/CubismCdiJson.cpp
    framework/src/CubismDefaultParameterId.cpp
    framework/src/CubismFramework.cpp
    framework/src/CubismModelSettingJson.cpp
    framework/src/Effect/CubismBreath.cpp
    framework/src/Effect/CubismEyeBlink.cpp
    framework/src/Effect/CubismLook.cpp
    framework/src/Effect/CubismPose.cpp
    framework/src/Id/CubismId.cpp
    framework/src/Id/CubismIdManager.cpp
    framework/src/Math/CubismMath.cpp
    framework/src/Math/CubismMatrix44.cpp
    framework/src/Math/CubismModelMatrix.cpp
    framework/src/Math/CubismTargetPoint.cpp
    framework/src/Math/CubismVector2.cpp
    framework/src/Math/CubismViewMatrix.cpp
    framework/src/Model/CubismMoc.cpp
    framework/src/Model/CubismModel.cpp
    framework/src/Model/CubismModelMultiplyAndScreenColor.cpp
    framework/src/Model/CubismModelUserData.cpp
    framework/src/Model/CubismModelUserDataJson.cpp
    framework/src/Model/CubismUserModel.cpp
    framework/src/Motion/ACubismMotion.cpp
    framework/src/Motion/CubismBreathUpdater.cpp
    framework/src/Motion/CubismExpressionMotion.cpp
    framework/src/Motion/CubismExpressionMotionManager.cpp
    framework/src/Motion/CubismExpressionUpdater.cpp
    framework/src/Motion/CubismEyeBlinkUpdater.cpp
    framework/src/Motion/CubismLipSyncUpdater.cpp
    framework/src/Motion/CubismLookUpdater.cpp
    framework/src/Motion/CubismMotion.cpp
    framework/src/Motion/CubismMotionJson.cpp
    framework/src/Motion/CubismMotionManager.cpp
    framework/src/Motion/CubismMotionQueueEntry.cpp
    framework/src/Motion/CubismMotionQueueManager.cpp
    framework/src/Motion/CubismPhysicsUpdater.cpp
    framework/src/Motion/CubismPoseUpdater.cpp
    framework/src/Motion/CubismUpdateScheduler.cpp
    framework/src/Motion/ICubismUpdater.cpp
    framework/src/Motion/IParameterProvider.cpp
    framework/src/Physics/CubismPhysics.cpp
    framework/src/Physics/CubismPhysicsJson.cpp
    framework/src/Rendering/csmBlendMode.cpp
    framework/src/Rendering/CubismRenderer.cpp
    framework/src/Rendering/OpenGL/CubismOffscreenManager_OpenGLES2.cpp
    framework/src/Rendering/OpenGL/CubismOffscreenRenderTarget_OpenGLES2.cpp
    framework/src/Rendering/OpenGL/CubismRenderer_OpenGLES2.cpp
    framework/src/Rendering/OpenGL/CubismRenderTarget_OpenGLES2.cpp
    framework/src/Rendering/OpenGL/CubismShader_OpenGLES2.cpp
    framework/src/Type/csmRectF.cpp
    framework/src/Type/csmString.cpp
    framework/src/Utils/CubismDebug.cpp
    framework/src/Utils/CubismJson.cpp
    framework/src/Utils/CubismString.cpp
)

# A-V1 冒烟结论：框架 TU 编译必须预包含 stddef.h/stdint.h（命名空间包裹破坏 libc++ ::nullptr_t）。
# 用 CMAKE_CXX_FLAGS 注入（避免 target_compile_options 的 SHELL:/引号合并丢失问题）；
# 对 engine/native-lib TU 同样无害（它们本就包含这些头）。
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -include stddef.h -include stdint.h")

add_library(framework STATIC ${FRAMEWORK_SRC})
target_include_directories(framework PUBLIC
    framework/src
    include
)
# CSM_TARGET_ANDROID_ES2：OpenGL ES2 后端选择 GLES2 头（Android）
target_compile_options(framework PRIVATE
    -std=c++17
    -DCSM_TARGET_ANDROID_ES2
    -w
)

# --- Engine skeleton (static) ---
add_library(engine STATIC engine/engine.cpp)
target_include_directories(engine PUBLIC
    engine
    framework/src
    include
)
target_compile_options(engine PRIVATE -std=c++17)

# Our JNI wrapper library
add_library(live2dcubismcore SHARED native-lib.cpp)
target_include_directories(live2dcubismcore PRIVATE purism)
target_link_libraries(live2dcubismcore purism_core framework engine log GLESv2)
target_compile_options(live2dcubismcore PRIVATE -std=c++17)

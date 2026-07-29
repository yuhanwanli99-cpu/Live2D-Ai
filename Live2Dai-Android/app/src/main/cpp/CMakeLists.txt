cmake_minimum_required(VERSION 3.22.1)
project("live2dcubismcore" C CXX)

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

# Our JNI wrapper library
add_library(live2dcubismcore SHARED native-lib.cpp)
target_include_directories(live2dcubismcore PRIVATE purism)
target_link_libraries(live2dcubismcore purism_core log)
target_compile_options(live2dcubismcore PRIVATE -std=c++17)

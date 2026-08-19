# smoke/ — A-V1 ABI 混编冒烟（M0, Wave 1）

本目录是 **临时冒烟**，不属于 vendored framework 代码，也不参与 CMake 构建
（`app/src/main/cpp/CMakeLists.txt` 未引用本目录；Gradle 只消费该 CMakeLists，
下一波 M1 负责 CMake 合并时也不应把本目录编入产物）。

## 文件
| 文件 | 作用 |
|---|---|
| `flat_consumer.cpp` | 仅包含平铺 `PurismCore.h`（现有 native-lib / purism 自用模式）。期望 PASS。 |
| `framework_consumer.cpp` | 仅包含官方框架头（`Live2DCubismCore.hpp` 包裹头 + `CubismFramework.hpp`/`CubismMoc.hpp`/`CubismModel.hpp`），Core 访问走 `Live2D::Cubism::Core` 命名空间（M1/M2 真实编译模式）。期望 PASS。 |
| `abi_mix_smoke.cpp` | 主冒烟：同一 TU 同时包含官方包裹头 + `CubismFramework.hpp` + 平铺 `PurismCore.h`。默认只引用包裹域符号；`-DPSM_SMOKE_REF_FLAT` 额外引用平铺域符号（观察 include guard 遮蔽）。 |
| `mix_forced_dual_scope.cpp` | 双作用域强制混编：两次包含之间 `#undef` include guard，实测「csmVector2 等类型重复声明 / C 链接声明冲突」（U2 实测点）。 |
| `run_smoke.sh` | NDK clang++ 逐个仅编译（`-c`，不链接），摘要写入 `../vendor-smoke-result.txt`。 |
| `*.log` / `*.o` | 每次运行生成的日志与目标文件（可重新生成，非交付物）。 |

## 运行
```bash
cd app/src/main/cpp/smoke
bash run_smoke.sh          # NDK 取默认 Windows SDK 位置
# 或指定 NDK: bash run_smoke.sh /c/Users/.../ndk/27.0.12077973
```

## 结论速览
见 `app/src/main/cpp/vendor-smoke-result.txt`（含冲突细节与最小解法建议）。

## 关键设计事实（M1 需要知道）
- 官方框架的 Core 依赖 = `src/Live2DCubismCore.hpp`（`namespace Live2D::Cubism::Core { #include "Live2DCubismCore.h" }`）。上游的 `Live2DCubismCore.h` 在独立闭源仓库 Live2D/CubismCore 中；本工程由 **PurismCore 兼容头** `purism/Live2DCubismCore.h`（→ `PurismCore.h`）提供，因此 include 路径必须包含 `purism/`（或 `include/`，两份内容相同）。
- PurismCore.h / Live2DCubismCore.h 使用传统 include guard：**同一 TU 内先包裹包含后，平铺再包含会被静默跳过**（反之亦然）。同一 TU 同时使用两个作用域的符号不可直接表达；M1 的 CMake 合并应保持「框架 TU 走包裹域 / JNI TU 走平铺域」的 TU 级隔离，链接层由 extern "C" 同名符号自然兼容。

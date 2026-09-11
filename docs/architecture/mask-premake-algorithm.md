# Cubism Mask Premake 渲染算法

> 基于 CubismJavaFramework 的逆向分析（Cubism 4 SDK for Android）
> 用途：为 Live2DNative（Kotlin/Purism Core）实现 WebGPU 等效算法

---

## 目录

- [a) 总体流程](#a-总体流程)
- [b) Mask Setup 阶段](#b-mask-setup-阶段)
- [c) Mask Render 阶段](#c-mask-render-阶段)
- [d) Main Draw 阶段](#d-main-draw-阶段)
- [e) 数据流向图](#e-数据流向图)
- [f) 与 Live2DNative 的对应关系](#f-与-live2dnative-的对应关系)

---

## a) 总体流程

`CubismRendererAndroid.doDrawModel()` 每帧的完整调用链：

```
doDrawModel()
  │
  ├── beforeDrawModelRenderTarget()     ← 绑定 offscreen FBO（如果有混合模式）
  │
  ├── [drawableClippingManager != null]
  │   │  预检查 mask buffer 尺寸，不一致则重建
  │   │
  │   ├── isUsingHighPrecisionMask == true
  │   │   └── setupMatrixForHighPrecision()
  │   │       └── 每帧只计算矩阵，不实际渲染 mask（HPM 在 drawDrawable 内按需渲染）
  │   │
  │   └── isUsingHighPrecisionMask == false
  │       └── setupClippingContext()    ← **批量 mask premake**
  │           └── 一次过渲染所有 mask 到 FBO
  │
  ├── [offscreenClippingManager != null]
  │   └── 同上逻辑处理 offscreen 的 mask
  │
  ├── preDraw()                         ← 重置 OpenGL 状态
  │
  ├── drawObjectLoop(lastFBO, lastViewport)
  │   │  按 renderOrder 排序对象，逐个处理
  │   │
  │   └── for each object:
  │       ├── DRAWABLE → drawDrawable(index)
  │       │   ├── [HPM] 渲染当前 drawable 所需的 mask（按需）
  │       │   └── 用 masked/normal shader 渲染 drawable 本身
  │       │
  │       └── OFFSCREEN → addOffscreen → drawOffscreen
  │           └── 类似 drawDrawable 但用到 offscreen 的 clip context
  │
  └── afterDrawModelRenderTarget()      ← 归还 FBO
```

### 关键分支：isUsingHighPrecisionMask

| 模式 | 描述 | 性能 |
|------|------|------|
| `false`（默认） | 一次 setup 全部 mask，每帧只做一次渲染 | 快，但 mask 上限 36 个 |
| `true` | 只在需要时才为单个 drawable 渲染其特定 mask | 慢，但质量高、无上限 |

---

## b) Mask Setup 阶段

### b.1 calcClippedTotalBounds — 计算 mask group 的包围盒

对每个 `clippingContextListForMask[i]`，遍历它覆盖的所有 clipped drawable：

```
for each clippedDrawable in clipContext.clippedDrawableIndexList:
    vertices = model.getDrawableVertices(drawableIndex)
    for each vertex (x, y) in vertices:
        minX = min(minX, x)
        minY = min(minY, y)
        maxX = max(maxX, x)
        maxY = max(maxY, y)

clipContext.allClippedDrawRect = Rect(minX, minY, maxX - minX, maxY - minY)
clipContext.isUsing = (有顶点数据)
```

**Vertex 数据格式**：`float[]` 每 `VERTEX_STEP=2` 个元素为一组 `(x, y)`，从 `VERTEX_OFFSET=0` 开始。

### b.2 setupLayoutBounds — 在 mask texture 内分配子区域

这是算法的核心。每个 mask group 被分配：
- 一个颜色通道（R/G/B/A，即 `layoutChannelIndex = 0/1/2/3`）
- 一块矩形区域（`layoutBounds`，在 0..1 UV 空间）
- 一个 render texture 索引（`bufferIndex`）

**通道颜色定义**（`channelColors`）：

| Index | Channel | RGBA |
|-------|---------|------|
| 0 | R | (1,0,0,0) |
| 1 | G | (0,1,0,0) |
| 2 | B | (0,0,1,0) |
| 3 | A | (0,0,0,1) |

**布局规则**（一页 render texture 内）：

1. **计算每页分配数**：`countPerSheetDiv = ceil(usingClipCount / renderTextureCount)`
2. **每组布局方案**：
   - `countPerSheetDiv <= 4`：按 `divCount` + `modCount` 在 4 个通道间分布
   - 每个通道内再按 `layoutCount` 分割：
     - **1 个**：`layoutBounds = (0, 0, 1, 1)` 占满通道
     - **2 个**：左右两半，`width=0.5`
     - **3-4 个**：2x2 四分割，`width=0.5, height=0.5`
     - **5-9 个**：3x3 九分割，`width=1/3, height=1/3`
3. **多 render texture**：`renderTextureCount > 1` 时每页最多 32 个 mask（8 单元格 × 4 通道），否则最多 36 个（9 单元格 × 4 通道）

**简化示例**（6 个 mask groups，1 页 RT）：

```
countPerSheetDiv = ceil(6/4) = 2
divCount = 2/4 = 0, modCount = 2%4 = 2
→ channel 0: layoutCount=1, channel 1: layoutCount=1, channel 2: 0, channel 3: 0

channel 0 (R): layoutCount=1 → 全通道，1个mask
channel 1 (G): layoutCount=1 → 全通道，1个mask
```

### b.3 createMatrixForMask — 生成 mask 投影矩阵

每个 mask group 生成两个矩阵：

**`matrixForMask`**（用于 SetupMask vertex shader）：
```
1. translate(-1, -1)          // [0,1] → [-1,1] 的视口变换
2. scale(2, 2)                // 缩放去 NDC
3. translate(layoutX, layoutY) // 移到 layoutBounds 的 UV 位置
4. scale(scaleX, scaleY)      // 缩放到包围盒大小
5. translate(-boundsX, -boundsY) // 移到包围盒原点
```

其中 `scaleX = layoutWidth / boundsWidth`, `scaleY = layoutHeight / boundsHeight`

**`matrixForDraw`**（用于 masked drawable 的 vertex shader）：
```
1. translate(layoutX, layoutY)
2. scale(scaleX, scaleY)
3. translate(-boundsX, -boundsY)
```

> `matrixForDraw` 在 offscreen 模式下还会额外左乘 `mvp^-1`。

**High Precision Mask 模式下的特殊缩放**：
当 `boundsWidth * ppu > physicalMaskWidth` 时加 margin，否则 `scaleX = ppu / physicalMaskWidth`。
其中 `ppu = model.getPixelPerUnit()`，`physicalMaskWidth = layoutBounds.width * maskBufferWidth`。

---

## c) Mask Render 阶段

### c.1 入口

非 HPM 模式：`setupClippingContext()` 内的核心循环。
HPM 模式：在 `drawDrawable()` 或 `drawOffscreen()` 内按需执行，逻辑完全相同。

### c.2 FBO 绑定流程

```
for each clipContext in clippingContextListForMask:
    if clipContext.isUsing:
        maskFBO = getDrawableMaskBuffer(clipContext.bufferIndex)
        if maskFBO != currentMaskBuffer:
            currentMaskBuffer.endDraw()         // 结束上一个 FBO
            currentMaskBuffer = maskFBO
            currentMaskBuffer.beginDraw(lastFBO) // 绑定新 FBO
            // 白色清除：1 = 不画（mask area），0 = 画（mask shape）
            glClearColor(1, 1, 1, 1)
            glClear(GL_COLOR_BUFFER_BIT)

        渲染 clipContext 的所有 mask drawable
```

### c.3 使用的 Shader

**顶点着色器** `VertShaderSrcSetupMask.vert`：

```glsl
void main() {
    gl_Position = u_clipMatrix * a_position;      // ← 使用 clipMatrix 而非 MVP!
    v_myPos = u_clipMatrix * a_position;           // 用于片段着色器的边界测试
    v_texCoord = a_texCoord;
    v_texCoord.y = 1.0 - v_texCoord.y;             // 翻转 Y
}
```

**片段着色器** `FragShaderSrcSetupMask.frag`：

```glsl
void main() {
    // 检查当前像素是否在 layout bounds 内
    float isInside =
        step(u_baseColor.x, v_myPos.x/v_myPos.w)
        * step(u_baseColor.y, v_myPos.y/v_myPos.w)
        * step(v_myPos.x/v_myPos.w, u_baseColor.z)
        * step(v_myPos.y/v_myPos.w, u_baseColor.w);

    // 输出 = 通道颜色 × 纹理alpha × 边界测试
    gl_FragColor = u_channelFlag * texture2D(s_texture0, v_texCoord).a * isInside;
}
```

其中 `u_baseColor = (left*2-1, top*2-1, right*2-1, bottom*2-1)`，将 layoutBounds 从 [0,1] 映射到 [-1,1] 视口空间。

### c.4 每个 mask group 的绘制

```
for each clipDrawIndex in clipContext.clippingIdList:
    if model.getDrawableDynamicFlagVertexPositionsDidChange(clipDrawIndex):
        renderer.isCulling(model.getDrawableCulling(clipDrawIndex))
        renderer.setClippingContextBufferForMask(clipContext)
        renderer.drawMeshAndroid(model, clipDrawIndex)
```

注意：
- **Blend mode**：`src=GL_ZERO, dst=GL_ONE_MINUS_SRC_COLOR`, alpha 同理。这是为了在白色背景上"挖"出 mask 形状。
- 每个 mask drawable 使用其本身的 position/uv/texture 数据，但投影矩阵用 `clipMatrix`。
- `u_channelFlag` 决定 mask 写入哪个通道（R/G/B/A）。

---

## d) Main Draw 阶段

### d.1 Shader 选择逻辑

在 `setupShaderProgramForDrawable()` 中：

```java
final boolean isMasked = renderer.getClippingContextBufferForDrawable() != null;
final boolean isInvertedMask = model.getDrawableInvertedMask(index);
final csmBlendMode blendMode = model.getDrawableBlendModeType(index);
final boolean isPremultipliedAlpha = renderer.isPremultipliedAlpha();

shaderIndex = calculateShaderIndex(blendMode, maskState, isPremultipliedAlpha);
```

**MaskType 枚举**（影响 shader 是否采样 `s_texture1` 和 `u_channelFlag`）：

| MaskType | isMasked | isInverted | isPremultiplied |
|----------|----------|------------|-----------------|
| NONE | false | — | false |
| MASKED | true | false | false |
| MASKED_INVERTED | true | true | false |
| PREMULTIPLIED_ALPHA | false | — | true |
| MASKED_PREMULTIPLIED_ALPHA | true | false | true |
| MASKED_INVERTED_PREMULTIPLIED_ALPHA | true | true | true |

### d.2 带 mask 的 drawable shader

**顶点着色器** `VertShaderSrcMasked.vert`：

```glsl
void main() {
    gl_Position = u_matrix * a_position;            // ← 标准 MVP 矩阵
    v_clipPos = u_clipMatrix * a_position;           // ← mask clip 矩阵
    v_texCoord = a_texCoord;
    v_texCoord.y = 1.0 - v_texCoord.y;
}
```

**片段着色器** `FragShaderSrcMask.frag`：

```glsl
void main() {
    vec4 texColor = texture2D(s_texture0, v_texCoord);
    // Multiply & Screen 颜色叠加
    texColor.rgb = texColor.rgb * u_multiplyColor.rgb;
    texColor.rgb = texColor.rgb + u_screenColor.rgb - (texColor.rgb * u_screenColor.rgb);

    vec4 col_formask = texColor * u_baseColor;
    col_formask.rgb = col_formask.rgb * col_formask.a;  // 预乘 alpha

    // 从 mask texture 读取（逆时针 - W 除法）
    vec4 clipMask = (1.0 - texture2D(s_texture1, v_clipPos.xy / v_clipPos.w)) * u_channelFlag;
    float maskVal = clipMask.r + clipMask.g + clipMask.b + clipMask.a;

    col_formask = col_formask * maskVal;
    gl_FragColor = col_formask;
}
```

关键点：
- `s_texture0` = 主纹理（ArtMesh 贴图）
- `s_texture1` = mask texture（render target 的 color attachment）
- `u_clipMatrix` = `clipContext.matrixForDraw` — 把顶点变换到 mask texture 的 UV 空间
- `v_clipPos.xy / v_clipPos.w` = 透视校正后的 mask UV 坐标
- `(1.0 - texture2D(...))` = mask 反转（因为白色=不画，黑色=画）
- `u_channelFlag` = 从 mask texel 中提取本 mask group 对应通道的值
- `maskVal` = 四个通道的 mask 值求和（因为每个通道存不同的 mask group）
- 如果不带 mask（MaskType.NONE），片段着色器 `FragShaderSrc.frag` 没有采样 `s_texture1`，直接输出 `texColor * u_baseColor`

### d.3 完整绘制伪码（drawDrawable）

```
drawDrawable(drawableIndex):
    if not model.getDrawableDynamicFlagIsVisible(drawableIndex):
        return

    // 检查是否需要提交当前 offscreen 到父 offscreen
    submitDrawToParentOffscreen(drawableIndex, DRAWABLE)

    clipContext = drawableClippingManager.getClippingContextListForDraw()[drawableIndex]

    // [HPM] 按需渲染 mask
    if clipContext != null && isUsingHighPrecisionMask() && clipContext.isUsing:
        glViewport(0, 0, maskBufferWidth, maskBufferHeight)
        preDraw()
        getDrawableMaskBuffer(clipContext.bufferIndex).beginDraw(currentFBO)
        glClearColor(1, 1, 1, 1)
        glClear(GL_COLOR_BUFFER_BIT)

        for each clipDrawIndex in clipContext.clippingIdList:
            if model.getDrawableDynamicFlagVertexPositionsDidChange(clipDrawIndex):
                renderer.setClippingContextBufferForMask(clipContext)
                drawMeshAndroid(model, clipDrawIndex)    // ← SetupMask shader

        maskFBO.endDraw()
        setClippingContextBufferForMask(null)
        glViewport(0, 0, renderTargetWidth, renderTargetHeight)
        preDraw()

    // 实际渲染 drawable
    setClippingContextBufferForDrawable(clipContext)
    isCulling(model.getDrawableCulling(drawableIndex))
    drawMeshAndroid(model, drawableIndex)    // ← Normal/Masked shader
```

---

## e) 数据流向图

### 输入数据来源

```
┌─────────────────────────────────────────────────────────────┐
│                          CubismModel                        │
│                                                             │
│  drawable 几何数据：                                         │
│    getDrawableVertexCount(i) → int                          │
│    getDrawableVertices(i) → float[] (x,y交错)               │
│    getDrawableVertexUvs(i) → float[] (u,v交错)              │
│    getDrawableVertexIndices(i) → short[]                    │
│    getDrawableVertexIndexCount(i) → int                     │
│                                                             │
│  mask 关系数据：                                              │
│    getDrawableMasks() → int[][]    [drawableIndex][]maskId  │
│    getDrawableMaskCounts() → int[] 各 drawable 的 mask 数量  │
│    getDrawableInvertedMask(i) → boolean                     │
│                                                             │
│  drawable 属性数据：                                         │
│    getDrawableTextureIndex(i) → int                         │
│    getDrawableOpacity(i) → float                            │
│    getDrawableMultiplyColor(i) → float[4]                   │
│    getDrawableScreenColor(i) → float[4]                     │
│    getDrawableBlendModeType(i) → csmBlendMode               │
│    getDrawableCulling(i) → boolean                          │
│    getDrawableParentPartIndex(i) → int                      │
│                                                             │
│  canvas/投影数据：                                            │
│    getCanvasWidth() → float                                 │
│    getCanvasHeight() → float                                │
│    getPixelPerUnit() → float                                │
│    getRenderOrders() → int[]                                │
│                                                             │
│  flag 数据：                                                 │
│    getDrawableDynamicFlagIsVisible(i) → boolean             │
│    getDrawableDynamicFlagVertexPositionsDidChange(i) → bool │
│    getDrawableDynamicFlagBlendColorDidChange(i) → boolean   │
└─────────────────────────────────────────────────────────────┘
```

### 数据结构关系

```
CubismModel (raw model data)
   │
   ├── RenderOrders ──────────────→ drawObjectLoop 的排序依据
   │
   ├── DrawableMasks/DrawableMaskCounts
   │   └── ACubismClippingManager.initialize()
   │       ├── clippingContextListForMask[]  ← 每个唯一的 mask 组一个
   │       └── clippingContextListForDraw[]  ← 每个 drawable 一个（或 null）
   │
   ├── DrawableVertices
   │   └── calcClippedTotalBounds()
   │       └── allClippedDrawRect (包围盒)
   │
   ├── CanvasWidth/PPU
   │   └── setupMatrixForHighPrecision()
   │       └── 用于计算物理尺寸 vs 实际尺寸
   │
   └── DrawableVertices + DrawableVertexUvs + DrawableVertexIndices
       └── setVertexAttributes() → glVertexAttribPointer
```

### mask 数据依赖链

```
┌─────────────┐     getDrawableMasks()     ┌──────────────────┐
│  CubismModel │ ─────────────────────────→ │ ClippingManager  │
│             │                            │  clippingContexts │
│  getCanvasW │                            └────────┬─────────┘
│  getPPU     │ ───────────┐                       │
└─────────────┘            │                       │
                           ▼                       ▼
                    ┌──────────────┐    ┌─────────────────────┐
                    │ setupLayout  │    │ calcClippedTotal    │
                    │ Bounds()     │    │ Bounds()            │
                    │   UV分配     │    │   包围盒计算         │
                    └──────┬───────┘    └──────────┬──────────┘
                           │                       │
                           ▼                       ▼
                    ┌───────────────────────────────────┐
                    │ createMatrixForMask()              │
                    │   matrixForMask (SetupMask VS)     │
                    │   matrixForDraw (Masked VS)        │
                    └───────────────┬───────────────────┘
                                    │
                  ┌─────────────────┼────────────────────┐
                  ▼                 ▼                     ▼
          ┌────────────┐   ┌──────────────┐   ┌────────────────┐
          │ SetupMask  │   │ MASKED       │   │ NORMAL         │
          │ Shader     │   │ Shader       │   │ Shader         │
          │ (mask FBO) │   │ (main pass)  │   │ (main pass)    │
          └────────────┘   └──────────────┘   └────────────────┘
```

### Mask Texture 内存布局示例

1 页 render texture (256×256)，6 个 mask groups：

```
┌───────┬───────┐
│  R.0  │  R.1  │     R channel:
│ mask0 │ mask1 │     2 columns × 1 row = 2 masks
├───────┼───────┤
│  G.0  │  G.1  │     G channel:
│ mask2 │ mask3 │     2 columns × 1 row = 2 masks
├───────┼───────┤
│  B.0  │  B.1  │     B channel:
│ mask4 │ mask5 │     2 columns × 1 row = 2 masks
├───────┴───────┤
│  A: unused    │     A channel: 0 masks
└───────────────┘
```

每个像素颜色值 = 4 个 mask 值（分别存在 R/G/B/A 通道）。
当 shader 读取时，只提取自己 channel 的值，其余为 0。

---

## f) 与 Live2DNative 的对应关系

以下是 CubismJavaFramework 调用 CubismModel 的每个方法，映射到 Live2DNative 的 external fun：

| CubismJavaFramework 方法 | Live2DNative 对应 | 说明 |
|---|---|---|
| `model.getDrawableCount()` | `getDrawableCount(modelPtr)` | 返回 drawable 总数 |
| `model.getDrawableMasks()` | `getDrawableMaskCounts(modelPtr)` + `getDrawableMasks(modelPtr, i)` | 先获取每个 drawable 的 mask 数量数组，再逐个获取 |
| `model.getDrawableMaskCounts()` | `getDrawableMaskCounts(modelPtr)` | mask 数量数组 |
| `model.getDrawableVertices(i)` | `getDrawablePositions(modelPtr, i)` | float[] 每两个一组 (x,y) |
| `model.getDrawableVertexUvs(i)` | `getDrawableUvs(modelPtr, i)` | float[] 每两个一组 (u,v) |
| `model.getDrawableVertexIndices(i)` | `getDrawableIndices(modelPtr, i)` | short[] 索引数组 |
| `model.getDrawableVertexCount(i)` | `getDrawableVertexCounts(modelPtr)[i]` | 顶点数量 |
| `model.getDrawableVertexIndexCount(i)` | `getDrawableIndexCounts(modelPtr)[i]` | 索引数量 |
| `model.getDrawableTextureIndex(i)` | `getDrawableTextureIndices(modelPtr)[i]` | 纹理索引 |
| `model.getDrawableOpacity(i)` | `getDrawableOpacities(modelPtr)[i]` | 不透明度 |
| `model.getDrawableMultiplyColor(i)` | `getDrawableMultiplyColors(modelPtr)` | float[] 每 4 个一组 (r,g,b,a) |
| `model.getDrawableScreenColor(i)` | `getDrawableScreenColors(modelPtr)` | float[] 每 4 个一组 (r,g,b,a) |
| `model.getDrawableCulling(i)` | `getDrawableConstantFlags(modelPtr)[i] & FLAG_DOUBLE_SIDED` | 双面 != culling |
| `model.getDrawableInvertedMask(i)` | `getDrawableConstantFlags(modelPtr)[i] & FLAG_INVERTED_MASK` | 倒置 mask 标记 |
| `model.getDrawableBlendModeType(i)` | `getDrawableConstantFlags(modelPtr)[i] & (FLAG_ADDITIVE\|FLAG_MULTIPLICATIVE)` | 混合模式 |
| `model.getDrawableParentPartIndex(i)` | `getDrawableParentPartIndices(modelPtr)[i]` | 父部件索引 |
| `model.getDrawableDynamicFlagIsVisible(i)` | `getDrawableDynamicFlags(modelPtr)[i] & DYNAMIC_IS_VISIBLE` | 可见性 |
| `model.getDrawableDynamicFlagVertexPositionsDidChange(i)` | `getDrawableDynamicFlags(modelPtr)[i] & DYNAMIC_VERTEX_POSITIONS_CHANGED` | 顶点变化标记 |
| `model.getDrawable DynamicFlagBlendColorDidChange(i)` | `getDrawableDynamicFlags(modelPtr)[i] & DYNAMIC_BLEND_COLOR_CHANGED` | 混合颜色变化标记 |
| `model.getCanvasWidth()` | `getCanvasInfo(modelPtr)[0]` | getCanvasInfo 返回 [width, height, ppu] |
| `model.getCanvasHeight()` | `getCanvasInfo(modelPtr)[1]` | |
| `model.getPixelPerUnit()` | `getCanvasInfo(modelPtr)[2]` | Pixels Per Unit |
| `model.getRenderOrders()` | `getDrawableDrawOrders(modelPtr)` | 绘制顺序 |
| `model.getPartParentPartIndex(i)` | `getPartParentPartIndices(modelPtr)` (需添加) | 部件父子关系 |

### 需要添加的 Live2DNative 函数

Live2DNative 目前缺少以下函数（需添加到 Kotlin JNI 层或自行解析 Cubism Core 数据结构）：

1. **`getPartParentPartIndices()`** — 部件父子关系，用于 offscreen 层级处理
2. **`getDrawableParentPartIndices()`** — drawable 所属部件索引
3. **`getOffscreenCount()` / `getOffscreenMaskCounts()` / `getOffscreenMasks()`** — offscreen 相关
4. **`getOffscreenOwnerIndices()`** — offscreen 所属部件
5. **`getPartsHierarchy()`** — 部件层级（可基于 partParentPartIndices 自行建树）

也可以选择**不添加**这些函数，而是：
- 在 Kotlin 层解析 `*.model3.json` 的 `PartGroups` 和 `Groups` 来重建层级关系
- 或简化实现：只支持 drawable mask，暂不支持 offscreen mask

### Vertex 数据结构对比

| CubismJavaFramework | Live2DNative |
|---|---|
| `float[]` 交错格式：`[x0,y0, x1,y1, ...]` | `getDrawablePositions()` 返回 `float[]`，也是 `[x0,y0, x1,y1, ...]` |
| `VERTEX_OFFSET = 0, VERTEX_STEP = 2` | 相同步长 |
| `short[]` indices | `getDrawableIndices()` 返回 `ShortArray` |

---

## 附录 A：关键常量

| 常量 | 值 | 说明 |
|---|---|---|
| `COLOR_CHANNEL_COUNT` | 4 | RGBA |
| `CLIPPING_MASK_MAX_COUNT_ON_DEFAULT` | 36 | 单 RT 时最大 mask 数（9 格 × 4 通道） |
| `CLIPPING_MASK_MAX_COUNT_ON_MULTI_RENDER_TEXTURE` | 32 | 多 RT 时每页最大 mask 数（8 格 × 4 通道） |
| `clippingMaskBufferSize` 默认值 | (256, 256) | mask texture 分辨率 |
| `MASK_MARGIN` | 0.05 | 包围盒外扩 5% margin |

## 附录 B：HPM vs 批量模式选择指南

| 场景 | 推荐模式 | 原因 |
|---|---|---|
| mask 数量 ≤ 36，质量要求一般 | 批量（isUsingHighPrecisionMask=false） | 一帧一次绘制，性能好 |
| mask 数量 > 36，或有 blend mode | 高精度（isUsingHighPrecisionMask=true） | 无上限，且 blend mode 强制 HPM |
| WebGPU 适配初期 | 批量 | 实现简单，跑通为主 |

## 附录 C：参考源码文件

| 文件 | 关键方法 |
|---|---|
| `CubismRenderer.java` | `drawModel()`, `isUsingHighPrecisionMask()` |
| `CubismRendererAndroid.java` | `doDrawModel()`, `drawDrawable()`, `drawObjectLoop()`, `preDraw()`, `drawMeshAndroid()` |
| `ACubismClippingManager.java` | `setupLayoutBounds()`, `calcClippedTotalBounds()`, `createMatrixForMask()`, `setupMatrixForHighPrecision()` |
| `CubismClippingManagerAndroid.java` | `setupClippingContext()` — 完整 mask premake 流程 |
| `CubismShaderAndroid.java` | `setupShaderProgramForMask()`, `setupShaderProgramForDrawable()` |
| `CubismRenderTargetAndroid.java` | `beginDraw()`, `endDraw()`, `createRenderTarget()` |
| `FragShaderSrcSetupMask.frag` | SetupMask fragment shader |
| `VertShaderSrcSetupMask.vert` | SetupMask vertex shader |
| `FragShaderSrcMask.frag` | Masked drawable fragment shader |
| `VertShaderSrcMasked.vert` | Masked drawable vertex shader |
| `FragShaderSrc.frag` | Normal (unmasked) fragment shader |

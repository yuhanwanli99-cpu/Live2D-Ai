# CubismJavaFramework Android Renderer Architecture Analysis

> Source: Live2D/CubismJavaFramework (develop branch)
> Analyzed files: 8 Android rendering support classes

---

## 1. File-by-File Breakdown

### 1.1 `ACubismClippingManager.java` (Base Class)

**Package:** `com.live2d.sdk.cubism.framework.rendering`

**Key class:** `ACubismClippingManager<T_ClippingContext, T_RenderTarget>` (abstract)

**Purpose:** Abstract skeleton for the clipping mask system. Handles mask registration, layout, matrix computation, and bounding box calculation.

**Key fields:**

| Field | Type | Purpose |
| --- | --- | --- |
| `channelColors` | `List<CubismTextureColor>` | RGBA channel colors: R=(1,0,0,0), G=(0,1,0,0), B=(0,0,1,0), A=(0,0,0,1) |
| `clippingContextListForMask` | `List<T_ClippingContext>` | All unique clip contexts used as masks |
| `clippingContextListForDraw` | `List<T_ClippingContext>` | Clip contexts per drawable (may share) |
| `clippingContextListForOffscreen` | `List<T_ClippingContext>` | Clip contexts per offscreen object |
| `clippingMaskBufferSize` | `CubismVector2` | Default 256x256 mask buffer size |
| `renderTextureCount` | `int` | Number of render textures for masks |
| `clearedMaskBufferFlags` | `boolean[]` | Per-texture clear tracking |
| `tmpMatrix / tmpMatrixForMask / tmpMatrixForDraw` | `CubismMatrix44` | Reusable matrices |

**Key methods:**

#### `initialize(type, model, maskBufferCount, drawableObjectType)`

- Called during renderer setup
- Determines object count: `model.getDrawableCount()` (DRAWABLE) or `model.getOffscreenCount()` (OFFSCREEN)
- Gets mask arrays: `model.getDrawableMasks()` / `model.getOffscreenMasks()`
- For each object with masks > 0, calls `findSameClip()` to deduplicate
- Creates new `ACubismClippingContext` via factory if no match found
- Adds to `clippingContextListForMask` and `clippingContextListForDraw`/`clippingContextListForOffscreen`

**CubismModel methods called:**

- `model.getDrawableCount()`
- `model.getDrawableMasks()`
- `model.getDrawableMaskCounts()`
- `model.getOffscreenCount()`
- `model.getOffscreenMasks()`
- `model.getOffscreenMaskCounts()`

#### `setupLayoutBounds(usingClipCount)`

- Core mask packing algorithm (see Section 2)
- Determines how many masks fit per render texture per RGBA channel
- Splits each render texture into 1, 2, 4, or 9 regions
- Sets `layoutBounds` (UV rect on texture) for each clip context
- Assigns `layoutChannelIndex` (0=R, 1=G, 2=B, 3=A)
- Assigns `bufferIndex` (which render texture to use)

#### `createMatrixForMask(isRightHanded, layoutBoundsOnTex01, scaleX, scaleY)`

- Computes two matrices:
  - `tmpMatrixForMask`: transforms model coords -> clip texture coords (used during mask rendering)
  - `tmpMatrixForDraw`: transforms model coords -> draw coords (used during final rendering with mask)
- Mask matrix pipeline: identity -> translate to [-1,-1] -> scale to [2,2] -> translate to layout bounds -> scale by (scaleX, scaleY) -> translate to model bounds origin
- Draw matrix similar but handles right-handed coordinate flip

#### `calcClippedTotalBounds(model, clippingContext, drawableObjectType)`

- Computes axis-aligned bounding box of all drawables that use a given clip context
- For DRAWABLE: iterates `clippingContext.clippedDrawableIndexList`
- For OFFSCREEN: collects child drawables via `collectOffscreenChildDrawableIndexList`
- Calls `model.getDrawableVertexCount()`, `model.getDrawableVertices()` per drawable
- Sets `clippingContext.isUsing` = false if no valid vertices found

**CubismModel methods called:**

- `model.getDrawableVertexCount(index)`
- `model.getDrawableVertices(index)`
- `model.getOffscreenOwnerIndices()`
- `model.getPartsHierarchy()`
- `model.getPixelPerUnit()`

#### `setupMatrixForHighPrecision(model, isRightHanded, drawableObjectType, mvp)`

- Advanced version that accounts for PPU (pixels per unit) vs physical mask pixel size
- Same basic flow as `setupClippingContext` but computes scale differently:
  - If model bounds (in PPU) > physical mask size: use bounds-based scaling with margin
  - Else: use PPU-to-physical-pixel ratio
- For OFFSCREEN: multiplies `matrixForDraw` by inverse MVP

#### `findSameClip(drawableMasks, drawableMaskCounts)`

- Deduplication: checks if an existing clip context has the same set of mask drawable IDs
- Compares count first, then verifies all IDs match regardless of order

---

### 1.2 `CubismClippingManagerAndroid.java` (Platform Implementation)

**Package:** `com.live2d.sdk.cubism.framework.rendering.android`

**Key class:** `CubismClippingManagerAndroid extends ACubismClippingManager<CubismClippingContextAndroid, CubismRenderTargetAndroid>`

**Purpose:** Android/OpenGL ES implementation of the clipping manager.

**Key method:**

#### `setupClippingContext(model, renderer, lastFBO, lastViewport, drawableObjectType)`

The main per-frame mask rendering pipeline:

1. **Calculate bounds** — for each mask context, call `calcClippedTotalBounds(model, clipContext, drawableObjectType)`
2. **Early exit** — if no masks are active (`usingClipCount == 0`), return
3. **Set viewport** — `glViewport(0, 0, maskWidth, maskHeight)`
4. **Get mask buffer** — `renderer.getDrawableMaskBuffer(0)` or `renderer.getOffscreenMaskBuffer(0)`
5. **Begin mask draw** — `currentMaskBuffer.beginDraw(lastFBO)` (binds FBO)
6. **Clear buffer** — `renderer.preDraw()` (glClear)
7. **Layout masks** — `setupLayoutBounds(usingClipCount)`
8. **Reset clear flags** — per-frame initialization
9. **Render each mask:**
   a. Get clip context, bounds, layout
   b. If mask buffer changed, endDraw/beginDraw on new buffer + preDraw
   c. Expand bounds by 5% margin
   d. Compute scale and call `createMatrixForMask()`
   e. For OFFSCREEN: multiply draw matrix by inverse MVP (`renderer.getMvpMatrix().getInvert()`)
   f. **Draw each clip drawable:**
      - Check `model.getDrawableDynamicFlagVertexPositionsDidChange(clipDrawIndex)` — skip if vertices unchanged
      - Set culling: `renderer.isCulling(model.getDrawableCulling(clipDrawIndex))`
      - Clear mask if not yet cleared this frame: `glClearColor(1,1,1,1); glClear(GL_COLOR_BUFFER_BIT)` (white = masked out, black = masked in via shader multiply)
      - Set clipping context for mask: `renderer.setClippingContextBufferForMask(clipContext)`
      - Draw: `renderer.drawMeshAndroid(model, clipDrawIndex)`
10. **End mask draw** — `currentMaskBuffer.endDraw()`
11. **Clear clipping context** — `renderer.setClippingContextBufferForMask(null)`
12. **Restore viewport** — `glViewport(lastViewport[0], lastViewport[1], lastViewport[2], lastViewport[3])`

**Renderer methods called:**

- `renderer.getDrawableMaskBuffer(n)`
- `renderer.getOffscreenMaskBuffer(n)`
- `renderer.preDraw()`
- `renderer.isCulling(boolean)`
- `renderer.setClippingContextBufferForMask(context)`
- `renderer.drawMeshAndroid(model, index)`
- `renderer.getMvpMatrix()`

**CubismModel methods called:**

- `model.getDrawableDynamicFlagVertexPositionsDidChange(index)`
- `model.getDrawableCulling(index)`

---

### 1.3 `CubismShaderAndroid.java` (Singleton Shader Manager)

**Package:** `com.live2d.sdk.cubism.framework.rendering.android`

**Key class:** `CubismShaderAndroid` (singleton, private constructor + `getInstance()`)

**Purpose:** Manages all shader programs (~100+ variants), selects and binds the correct shader for each draw call.

**Internal data structure:**

```
CubismShaderSet:
  - shaderProgram: int  (GL program handle)
  - attributePositionLocation: int
  - attributeTexCoordLocation: int
  - uniformMatrixLocation: int
  - uniformClipMatrixLocation: int
  - samplerTexture0Location: int
  - samplerTexture1Location: int
  - samplerBlendTextureLocation: int
  - uniformBaseColorLocation: int
  - uniformMultiplyColorLocation: int
  - uniformScreenColorLocation: int
  - uniformChannelFlagLocation: int
```

**Shader index layout (via `CubismShaderIndexConstants`):**

| Range | Count | Description |
| --- | --- | --- |
| 0 | 1 | COPY (utility) |
| 1 | 1 | SETUP_MASK (utility) |
| 2-7 | 6 | Normal/Over base: none, masked, masked_inverted, PMA, PMA_masked, PMA_masked_inverted |
| 8-13 | 6 | Add (compatible) — reuses Normal shaders |
| 14-19 | 6 | Multiply (compatible) — reuses Normal shaders |
| 20+ | many | Blend mode variants: (ColorBlendMode x AlphaBlendMode) x 6 mask variants |

**Total shaders:** ~6 base + 2 utility + up to (7 color x 6 alpha x 6 mask) = ~252+ if fully generated, but only non-Normal color blends generate unique programs.

**Key methods:**

#### `generateShaders()`

- Called once in `getInstance()`
- Loads shader source from asset files via `CubismFramework.getLoadFileFunction()`
- Base path: `com/live2d/sdk/cubism/framework/shaders/standardES`
- For each slot, calls `loadShaderProgramFromFile(vertName, fragName)`
- For blend mode variants: appends `#define CSM_COLOR_BLEND_MODE N` and `#define CSM_ALPHA_BLEND_MODE N` + merges `ColorBlend.frag` and `AlphaBlend.frag`
- Reads shader variable locations via `glGetAttribLocation`/`glGetUniformLocation`
- Add/Multiply compatible modes reuse Normal shaders

**Vertex shader files used:**

- `VertShaderSrc.vert` — no mask
- `VertShaderSrcMasked.vert` — with mask
- `VertShaderSrcCopy.vert` — copy
- `VertShaderSrcSetupMask.vert` — mask generation
- `VertShaderSrcBlend.vert` — blend mode (no mask)
- `VertShaderSrcMaskedBlend.vert` — blend mode (with mask)

**Fragment shader files used:**

- `FragShaderSrc.frag` / `FragShaderSrcMask.frag` / `FragShaderSrcMaskInverted.frag`
- `FragShaderSrcPremultipliedAlpha.frag` / `FragShaderSrcMaskPremultipliedAlpha.frag` / `FragShaderSrcMaskInvertedPremultipliedAlpha.frag`
- `FragShaderSrcCopy.frag`
- `FragShaderSrcSetupMask.frag`
- `FragShaderSrcBlend.frag` / `FragShaderSrcMaskBlend.frag` / `FragShaderSrcMaskInvertedBlend.frag` / `FragShaderSrcPremultipliedAlphaBlend.frag` / `FragShaderSrcMaskPremultipliedAlphaBlend.frag` / `FragShaderSrcMaskInvertedPremultipliedAlphaBlend.frag`
- `FragShaderSrcColorBlend.frag` (appended for blend modes)
- `FragShaderSrcAlphaBlend.frag` (appended for blend modes)
- Tegra variants (/*Tegra.frag) also exist

#### `setupShaderProgramForDrawable(renderer, model, index)`

Called per drawable. Algorithm:

1. **Determine mask state** — check `renderer.getClippingContextBufferForDrawable() != null`
2. **Check inverted mask** — `model.getDrawableInvertedMask(index)`
3. **Check premultiplied alpha** — `renderer.isPremultipliedAlpha()`
4. **Get blend mode** — `model.getDrawableBlendModeType(index)`
5. **Calculate shader index** — via `CubismShaderIndexCalculator.calculateShaderIndex(blendMode, maskState, isPremultipliedAlpha)`
6. **Handle blend mode** (v5.3+ advanced blend):
   - If `blendMode.isBlendMode()`: set src=ONE, dst=ZERO; copy render target to get blend texture
7. **Handle legacy blend** (v5.2-):
   - NORMAL: ONE, ONE_MINUS_SRC_ALPHA
   - ADD: ONE, ONE (srcAlpha=ZERO)
   - MULTIPLY: DST_COLOR, ONE_MINUS_SRC_ALPHA (srcAlpha=ZERO)
8. **Bind shader** — `glUseProgram(shaderSet.shaderProgram)`
9. **Set texture** — `setUpTexture()`: bind `renderer.getBoundTextureId(model.getDrawableTextureIndex(index))` to GL_TEXTURE0
10. **Set vertices** — `setVertexAttributes()`: get `renderer.getDrawableInfoCachesHolder()`, call `setUpVertexArray/setUpUvArray`
11. **If masked:** bind mask texture to GL_TEXTURE1, set clip matrix, set channel flag color
12. **If blend mode:** bind blend texture to GL_TEXTURE2
13. **Set MVP matrix** — `renderer.getMvpMatrix()`
14. **Set colors:** baseColor (with opacity), multiplyColor, screenColor
15. **Set blend function** — `glBlendFuncSeparate()`

**CubismModel methods called:**

- `model.getDrawableInvertedMask(index)`
- `model.getDrawableBlendModeType(index)`
- `model.getDrawableTextureIndex(index)`
- `model.getDrawableVertexCount(index)`
- `model.getDrawableVertices(index)`
- `model.getDrawableVertexUvs(index)`
- `model.getDrawableVertexIndices(index)`
- `model.getDrawableOpacity(index)`
- `model.isBlendModeEnabled()`
- `model.getOverrideMultiplyAndScreenColor()`

**Renderer methods called:**

- `renderer.getClippingContextBufferForDrawable()`
- `renderer.isPremultipliedAlpha()`
- `renderer.getCurrentOffscreen()`
- `renderer.copyRenderTarget()`
- `renderer.copyOffscreenRenderTarget()`
- `renderer.getDrawableMaskBuffer(index)`
- `renderer.getClippingContextBufferForDrawable().bufferIndex`
- `renderer.getClippingContextBufferForDrawable().matrixForDraw`
- `renderer.getClippingContextBufferForDrawable().layoutChannelIndex`
- `renderer.getClippingContextBufferForDrawable().getClippingManager().getChannelFlagAsColor()`
- `renderer.getMvpMatrix()`
- `renderer.getModelColorWithOpacity()`
- `renderer.getDrawableInfoCachesHolder()`

#### `setupShaderProgramForMask(renderer, model, index)`

For mask generation rendering:

1. Uses fixed shader: `SETUP_MASK` index
2. Blend mode: ZERO, ONE_MINUS_SRC_COLOR (srcAlpha=ZERO, ONE_MINUS_SRC_ALPHA)
3. Binds texture and vertex attributes (same as drawable)
4. Sets clip matrix from `renderer.getClippingContextBufferForMask().matrixForMask`
5. Sets baseColor encoding layout bounds: `r = rect.x*2-1, g = rect.y*2-1, b = rect.right*2-1, a = rect.bottom*2-1`

#### `setupShaderProgramForOffscreen(renderer, model, offscreen)`

Shaders for offscreen rendering with same variant selection as drawables:

- Always uses premultiplied alpha
- Gets offscreen-specific blend mode via `model.getOffscreenBlendModeType(offscreenIndex)`
- Uses fixed vertex buffer `RENDER_TARGET_VERTEX_BUFFER` and `RENDER_TARGET_REVERSE_UV_BUFFER`
- Gets old offscreen for blend mode: `offscreen.getOldOffscreen()`

**CubismModel methods called:**

- `model.getOffscreenInvertedMask(index)`
- `model.getOffscreenBlendModeType(index)`
- `model.getOffscreenOpacity(index)`
- `model.getOverrideMultiplyAndScreenColor()`

#### `copyTexture(texture, ...)`

Renders a full-screen quad to copy a texture (used for blend mode frame capture):

- Binds COPY shader
- Uses `RENDER_TARGET_VERTEX_BUFFER` (4 vertices: [-1,-1] to [1,1])
- Uses `RENDER_TARGET_UV_BUFFER` (standard UV: [0,0] to [1,1])
- Applies baseColor tint

---

### 1.4 `CubismDrawableInfoCachesHolder.java`

**Package:** `com.live2d.sdk.cubism.framework.rendering.android`

**Key class:** `CubismDrawableInfoCachesHolder`

**Purpose:** Pre-allocates and caches native FloatBuffer/ShortBuffer for each drawable's vertex, UV, and index data to avoid per-frame allocation.

**Construction (`CubismDrawableInfoCachesHolder(model)`):**

- `model.getDrawableCount()` — determines array sizes
- Pre-allocates arrays: `FloatBuffer[drawableCount]` for vertices, `FloatBuffer[drawableCount]` for UVs, `ShortBuffer[drawableCount]` for indices
- Each buffer is allocated via `ByteBuffer.allocateDirect(...)` in native byte order

**Key methods:**

| Method | What it does |
| --- | --- |
| `setUpVertexArray(drawableIndex, drawableVertices)` | clear + put + flip vertex buffer |
| `setUpUvArray(drawableIndex, drawableVertexUvs)` | clear + put + flip UV buffer |
| `setUpIndexArray(drawableIndex, drawableIndices)` | clear + put + flip index buffer |

**CubismModel methods called at construction:**

- `model.getDrawableCount()`
- `model.getDrawableVertices(index)`
- `model.getDrawableVertexUvs(index)`
- `model.getDrawableVertexIndices(index)`

---

### 1.5 `ACubismOffscreenRenderTarget.java` (Base Class)

**Package:** `com.live2d.sdk.cubism.framework.rendering`

**Key class:** `ACubismOffscreenRenderTarget<T, U>`

**Purpose:** Generic base for offscreen rendering targets. T is the subclass type, U is the render target type (e.g., `CubismRenderTargetAndroid`).

**Fields:**

| Field | Type | Purpose |
| --- | --- | --- |
| `renderTarget` | `U` (protected) | The underlying render target |
| `offscreenIndex` | `int` | Index in the offscreen array, default -1 |
| `parentRenderTarget` | `T` | Parent offscreen (for hierarchy) |
| `oldOffscreen` | `T` | Previous offscreen (for blend mode compositing) |

**Key methods:**

- `setOffscreenIndex(int)` / `getOffscreenIndex()`
- `setOldOffscreen(T)` / `getOldOffscreen()` — links to previous frame's offscreen for blend operations
- `setParentPartOffscreen(T)` / `getParentPartOffscreen()` — parent-child offscreen hierarchy
- `getRenderTarget()` — returns the underlying `U` render target

---

### 1.6 `CubismOffscreenRenderTargetAndroid.java`

**Package:** `com.live2d.sdk.cubism.framework.rendering.android`

**Key class:** `CubismOffscreenRenderTargetAndroid extends ACubismOffscreenRenderTarget<CubismOffscreenRenderTargetAndroid, CubismRenderTargetAndroid>`

**Key methods:**

#### `setOffscreenRenderTarget(width, height)`

- If render target already in use (`getUsingRenderTextureState()`): only check/resize existing `renderTarget`
- Otherwise: acquire from pool — `CubismOffscreenManagerAndroid.getInstance().getOffscreenRenderTarget(width, height)`

#### `getUsingRenderTextureState()`

- Delegates to `CubismOffscreenManagerAndroid.getInstance().getUsingRenderTextureState(renderTarget)`

#### `stopUsingRenderTexture()`

- Releases back to pool: `CubismOffscreenManagerAndroid.getInstance().stopUsingRenderTexture(renderTarget)`
- Sets `renderTarget = null`

---

### 1.7 `CubismOffscreenManagerAndroid.java`

**Package:** `com.live2d.sdk.cubism.framework.rendering.android`

**Key class:** `CubismOffscreenManagerAndroid extends ACubismOffscreenManager<CubismRenderTargetAndroid>`

**Purpose:** Singleton pool of offscreen render targets. Manages allocation, reuse, and lifecycle.

**Key methods:**

#### `getOffscreenRenderTarget(width, height)`

1. Call `updateRenderTargetCount()` (inherited — tracks peak usage)
2. Try to get unused target: `getUnusedOffscreenRenderTarget()`
   - If found and same size: return it
   - If found and different size: call `createRenderTarget(width, height)` then return
3. If none available: `createOffscreenRenderTarget()` then `createRenderTarget(width, height)`
4. Uses factory method `createRenderTargetInstanceInternal()` returning `new CubismRenderTargetAndroid()`

---

### 1.8 `CubismRendererProfileAndroid.java`

**Package:** `com.live2d.sdk.cubism.framework.rendering.android`

**Key class:** `CubismRendererProfileAndroid`

**Purpose:** Save/restore GL state around Cubism model rendering to avoid interfering with the host application's GL state.

**Fields saved/restored (~15 states):**

| State | GL Query |
| --- | --- |
| Array buffer binding | `GL_ARRAY_BUFFER_BINDING` |
| Element buffer binding | `GL_ELEMENT_ARRAY_BUFFER_BINDING` |
| Current program | `GL_CURRENT_PROGRAM` |
| Active texture unit | `GL_ACTIVE_TEXTURE` |
| Texture 2D binding (unit 0) | `GL_TEXTURE_BINDING_2D` (unit 0) |
| Texture 2D binding (unit 1) | `GL_TEXTURE_BINDING_2D` (unit 1) |
| Vertex attrib arrays [0-3] | `GL_VERTEX_ATTRIB_ARRAY_ENABLED` |
| Scissor test | `glIsEnabled(GL_SCISSOR_TEST)` |
| Stencil test | `glIsEnabled(GL_STENCIL_TEST)` |
| Depth test | `glIsEnabled(GL_DEPTH_TEST)` |
| Cull face | `glIsEnabled(GL_CULL_FACE)` |
| Blend | `glIsEnabled(GL_BLEND)` |
| Front face | `GL_FRONT_FACE` |
| Color write mask | `GL_COLOR_WRITEMASK` |
| Blend factors (RGB + Alpha) | `GL_BLEND_SRC_RGB/DST_RGB/SRC_ALPHA/DST_ALPHA` |

**`save()`:** reads all states via `glGetIntegerv`/`glIsEnabled`/`glGetBooleanv`
**`restore()`:** writes all states back in reverse-sensitive order (program first, then attrib arrays, then enables, then textures, then blend)

---

## 2. Clipping Mask Algorithm

### Overview

Cubism uses a **multi-mask packing** approach where multiple clipping masks are packed into one or more render textures, using separate RGBA channels to allow up to 4 masks per texture.

### Step-by-step pipeline

```
[Initialization]
  1. ACubismClippingManager.initialize() is called from renderer setup
  2. For each drawable/offscreen with mask count > 0:
     a. Deduplicate via findSameClip() — same set of mask IDs = same clip context
     b. Create ACubismClippingContext if new
     c. Add to clippingContextListForMask + clippingContextListForDraw/ForOffscreen

[Per-frame Mask Setup — CubismClippingManagerAndroid.setupClippingContext()]
  3. For each mask context: calcClippedTotalBounds() computes AABB of all clipped drawables
  4. Skip if no masks are active (usingClipCount == 0)

[Layout — setupLayoutBounds()]
  5. Determine max masks: 
     - 1 render texture: 9 regions x 4 channels = 36 max
     - Multi render textures: 8 regions x 4 channels x N textures
  6. Distribute masks across render textures:
     - countPerSheetDiv = ceil(usingClipCount / renderTextureCount)
     - divCount = countPerSheetDiv / 4 (per channel base count)
     - modCount = countPerSheetDiv % 4 (channels that get +1)
  7. For each render texture, for each channel (R->G->B->A):
     - Layout mask(s) in 1, 2, 4, or 9 equal regions
     - Set layoutBounds (UV rect), layoutChannelIndex, bufferIndex on clip context

[Mask Matrix — createMatrixForMask()]
  8. Compute matrixForMask (used when rendering the mask):
     identity -> translate(-1,-1) -> scale(2,2) -> translate(layoutX, layoutY) -> scale(scaleX, scaleY) -> translate(-boundsX, -boundsY)
  9. Compute matrixForDraw (used when applying the mask):
     identity -> translate(layoutX, layoutY * handFlip) -> scale(scaleX, scaleY * handFlip) -> translate(-boundsX, -boundsY)

[Mask Rendering — per mask context]
  10. glViewport(0, 0, maskBufferWidth, maskBufferHeight)
  11. For each mask context:
      a. Expand bounds by 5% margin
      b. Compute scale = layoutSize / boundsSize (or PPU-based for high-precision)
      c. If mask buffer changed: endDraw previous, beginDraw new
      d. Clear buffer if not yet cleared: glClearColor(1,1,1,1) (white = no mask)
      e. For each clip drawable:
         - Skip if model.getDrawableDynamicFlagVertexPositionsDidChange() == false
         - Bind SETUP_MASK shader
         - Set blend: ZERO, ONE_MINUS_SRC_COLOR (inverts white/black)
         - Draw mesh
      f. After all masks rendered: endDraw()
```

### Channel Encoding

Each mask is assigned to one of 4 RGBA channels. The shader selects which channel to read:

- Channel 0 (R): colorFlag = (1,0,0,0)
- Channel 1 (G): colorFlag = (0,1,0,0)
- Channel 2 (B): colorFlag = (0,0,1,0)
- Channel 3 (A): colorFlag = (0,0,0,1)

The mask generation shader writes mask values to the assigned channel. The final rendering shader reads the channel via `dot(maskTexture, u_channelFlag)`.

### Inverted Masks

When a drawable has an inverted mask, the shader variant `FragShaderSrcMaskInverted.frag` is used instead, which inverts the mask sampling.

---

## 3. Shader Selection Algorithm

`CubismShaderIndexCalculator.calculateShaderIndex()` computes the final index based on three factors:

```
shaderIndex = calculateShaderIndex(blendMode, maskState, isPremultipliedAlpha)
```

**Mask states (3):**

- `NONE` — no clipping mask
- `NORMAL` — has clipping mask
- `INVERTED` — has inverted clipping mask

**Blend mode types (3 legacy + N color x N alpha advanced):**

- `NORMAL` (normal/over)
- `ADD_COMPATIBLE` (additive, v5.2 compatible)
- `MULTIPLY_COMPATIBLE` (multiply, v5.2 compatible)
- Advanced blend modes: combination of `ColorBlendMode` x `AlphaBlendMode`

**Premultiplied alpha (2 states):** enabled or disabled

**Total base variants:** 3 x 3 x 2 = 18 non-blend shader setups, plus per-blend-mode variants.

### Shader binding flow per draw call

```
1. Get blend mode from model
2. Get mask state from renderer's clipping context buffer
3. Calculate shader index
4. Get CubismShaderSet from the pre-loaded list
5. If blend mode (v5.3+): copy offscreen to get blend texture
6. glUseProgram(shaderProgram)
7. setUpTexture(): bind drawable texture to TEXTURE0
8. setVertexAttributes(): bind vertex + UV buffers
9. If masked: bind mask texture to TEXTURE1, set clipMatrix + channelFlag
10. If blend mode: bind blend texture to TEXTURE2
11. Set MVP matrix
12. Set baseColor, multiplyColor, screenColor uniforms
13. glBlendFuncSeparate()
```

---

## 4. Drawable Info Caching

`CubismDrawableInfoCachesHolder` provides zero-allocation vertex/UV/index buffer management:

**Construction (`CubismModel model`):**

```
drawableCount = model.getDrawableCount()
for each drawable:
    vertexArrayCaches[i]  = ByteBuffer.allocateDirect(vertices.length * 4).asFloatBuffer()
    uvArrayCaches[i]      = ByteBuffer.allocateDirect(uvs.length * 4).asFloatBuffer()
    indexArrayCaches[i]   = ByteBuffer.allocateDirect(indices.length * 4).asShortBuffer()
```

**Per-frame usage:**

```
buffer.clear()      // reset position
buffer.put(data)    // fill from Java array
buffer.position(0)  // rewind for GL
return buffer       // pass directly to glVertexAttribPointer
```

This avoids creating new `FloatBuffer` objects every frame for each drawable. The holder is created once during `CubismRendererAndroid` initialization and reused thereafter.

**Usage in CubismShaderAndroid:**

- `setVertexAttributes()` accesses the holder via `renderer.getDrawableInfoCachesHolder()`
- Calls `setUpVertexArray(index, model.getDrawableVertices(index))` for position attribute
- Calls `setUpUvArray(index, model.getDrawableVertexUvs(index))` for texture coordinate attribute
- Index buffer is used separately during `glDrawElements`

---

## 5. Offscreen Render Target / FBO Management

### Architecture

```
CubismOffscreenManagerAndroid (singleton pool)
  └── ACubismOffscreenManager<CubismRenderTargetAndroid>
        ├── List of CubismRenderTargetAndroid (pooled)
        ├── getOffscreenRenderTarget(w, h)  → acquire from pool or create new
        ├── getUsingRenderTextureState(rt)   → is this target in use?
        ├── stopUsingRenderTexture(rt)       → release back to pool
        └── updateRenderTargetCount()        → track peak usage

CubismOffscreenRenderTargetAndroid (per-offscreen wrapper)
  └── extends ACubismOffscreenRenderTarget<T, U>
        ├── offscreenIndex: int              → index in model's offscreen array
        ├── oldOffscreen: T                  → previous frame's offscreen (for blends)
        ├── parentRenderTarget: T            → parent for hierarchical offscreens
        ├── renderTarget: CubismRenderTargetAndroid  → the actual GL resources
        └── setOffscreenRenderTarget(w, h)   → acquire from pool or resize
```

### Lifecycle

```
[Frame N]
  1. For each offscreen object in model:
     - CubismOffscreenRenderTargetAndroid.setOffscreenRenderTarget(w, h)
       → Pool provides or creates a CubismRenderTargetAndroid (FBO + texture)
     - setOldOffscreen(previousFrameOffscreen) → links to previous for blend compositing
     - Render into this offscreen:
       → renderer.beginDraw(offscreen) → bind FBO
       → renderer.drawMesh() for child drawables → render with offscreen shaders
       → renderer.endDraw() → unbind FBO

[Frame N+1]
     - The old offscreen becomes the "old" for blend mode operations
     - When blend mode is active, the COPY shader renders a full-screen quad:
       old offscreen texture → blend shader → new offscreen result

[Cleanup]
  - stopUsingRenderTexture() → returns CubismRenderTargetAndroid to pool
  - Pool.releaseInstance() → disposes all targets
  - ACubismOffscreenManager handles reference counting via useCount
```

### Render Target Pool Details

`CubismRenderTargetAndroid` (not directly in the fetched files, but referenced) encapsulates:

- FBO ID (GL framebuffer object)
- Color texture ID (GL texture)
- Width/height
- `createRenderTarget(w, h)` — creates FBO + texture
- `isSameSize(w, h)` — comparison
- `beginDraw(lastFBO)` / `endDraw()` — bind/unbind FBO
- `getColorBuffer()` — returns `int[1]` with texture ID

The pool automatically grows to meet peak demand and reuses existing targets of the same size. If size changes, the existing FBO/texture is recreated rather than allocating new ones.

---

## 6. End-to-End Rendering Flow

```
CubismRendererAndroid.drawModel()
  │
  ├─ 1. rendererProfile.save()              ← save GL state
  │
  ├─ 2. preDraw()                            ← clear buffers, set viewport
  │
  ├─ 3. setupClippingContext()               ← render all masks to mask FBOs
  │     ├─ calcClippedTotalBounds()          ← compute AABB per mask
  │     ├─ setupLayoutBounds()               ← pack masks into RGBA channels
  │     ├─ createMatrixForMask()             ← compute transform matrices
  │     ├─ render each mask drawable:
  │     │     ├─ shaderProgramForMask()       ← bind SETUP_MASK shader
  │     │     └─ drawMeshAndroid()           ← draw individual mask mesh
  │     └─ endDraw() on mask FBO
  │
  ├─ 4. For each offscreen object:
  │     ├─ setOffscreenRenderTarget()        ← acquire FBO from pool
  │     ├─ setupShaderProgramForOffscreen()  ← select/bind offscreen shader
  │     ├─ draw mesh to offscreen FBO
  │     └─ endDraw()
  │
  ├─ 5. For each drawable:
  │     ├─ setupShaderProgramForDrawable()   ← select based on blend+mask+PMA
  │     │     ├─ setUpTexture()              ← bind model texture
  │     │     ├─ setVertexAttributes()       ← bind cached vertex/UV buffers
  │     │     ├─ bind mask texture if masked ← read from mask FBO
  │     │     └─ bind blend texture if blend ← read from offscreen copy
  │     └─ drawMeshAndroid()                 ← glDrawElements with cached index buffer
  │
  └─ 6. rendererProfile.restore()           ← restore GL state
```

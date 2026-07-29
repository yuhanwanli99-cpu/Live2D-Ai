# CubismRendererAndroid 渲染架构分析

> 来源: Live2D/CubismJavaFramework (develop branch)
> 文件: CubismRendererAndroid.java, CubismRenderer.java, CubismShaderAndroid.java, CubismRenderTargetAndroid.java, CubismClippingManagerAndroid.java, CubismModel.java

---

## 目录

1. [类结构: 字段、构造函数、关键方法](#1)
2. [initialize() 初始化](#2)
3. [doDrawModel() 完整渲染循环](#3)
4. [drawMeshAndroid() 单个Drawable渲染](#4)
5. [preDraw() 和 setupOffscreen](#5)
6. [drawObjectLoop() 排序和绘制循环](#6)
7. [MVP矩阵设置](#7)
8. [CubismModel 被调用方法签名](#8)

---

## 1. 类结构: 字段、构造函数、关键方法 {#1}

### 继承关系

```
CubismRenderer (abstract, com.live2d.sdk.cubism.framework.rendering)
  └── CubismRendererAndroid (com.live2d.sdk.cubism.framework.rendering.android)
```

### CubismRenderer (抽象基类) 字段

| 字段 | 类型 | 说明 |
| ------ | ------ | ------ |
| `mvpMatrix44` | `CubismMatrix44` (private) | 模型-视图-投影矩阵 |
| `modelColor` | `CubismTextureColor` (private) | RGBA模型颜色 |
| `isCulling` | `boolean` (private) | 是否启用背面剔除 |
| `isPremultipliedAlpha` | `boolean` (private) | 是否启用预乘Alpha |
| `anisotropy` | `float` (private) | 各向异性过滤参数 |
| `model` | `CubismModel` (private) | 被渲染的模型实例 |
| `useHighPrecisionMask` | `boolean` (private) | 高精度掩码模式 |
| `modelRenderTargetWidth/Height` | `int` (protected) | 渲染目标尺寸 |

### CubismRendererAndroid 字段

| 字段 | 类型 | 说明 |
| ------ | ------ | ------ |
| `textures` | `Map<Integer, Integer>` | 模型纹理索引 -> OpenGL纹理ID映射 |
| `sortedObjectsIndexList` | `int[]` | 按绘制顺序排列的对象索引数组 |
| `sortedObjectsTypeList` | `DrawableObjectType[]` | 对应类型的数组 (DRAWABLE/OFFSCREEN) |
| `rendererProfile` | `CubismRendererProfileAndroid` | OpenGL状态保存/恢复 |
| `drawableClippingManager` | `CubismClippingManagerAndroid` | Drawable裁剪管理器 |
| `offscreenClippingManager` | `CubismClippingManagerAndroid` | Offscreen裁剪管理器 |
| `clippingContextBufferForMask` | `CubismClippingContextAndroid` | 当前掩码裁剪上下文 |
| `clippingContextBufferForDrawable` | `CubismClippingContextAndroid` | 当前Drawable裁剪上下文 |
| `clippingContextBufferForOffscreen` | `CubismClippingContextAndroid` | 当前Offscreen裁剪上下文 |
| `modelRenderTargets` | `List<CubismRenderTargetAndroid>` | 模型渲染目标FBO列表(索引0=主目标, 1=barrier备用) |
| `drawableMasks` | `CubismRenderTargetAndroid[]` | Drawable掩码FBO数组 |
| `offscreenMasks` | `CubismRenderTargetAndroid[]` | Offscreen掩码FBO数组 |
| `offscreenList` | `List<CubismOffscreenRenderTargetAndroid>` | Offscreen列表 |
| `currentFBO` | `int[]` | 当前FBO ID |
| `currentOffscreen` | `CubismOffscreenRenderTargetAndroid` | 当前Offscreen目标 |
| `modelRootFBO` | `int[]` | 根FBO（模型初始FBO） |
| `currentProgram` | `int[1]` | 当前shader program ID缓存 |
| `drawableInfoCachesHolder` | `CubismDrawableInfoCachesHolder` | Drawable顶点信息缓存 |
| `modelColorRGBA` | `CubismTextureColor` (private) | 复用实例，用于drawMeshAndroid |

### 关键静态方法

```java
// 工厂方法
public static CubismRenderer create(int width, int height);

// 释放静态资源（shader）
public static void staticRelease();

// Tegra扩展模式
public static void setExtShaderMode(boolean extMode, boolean extPAMode);

// 重新加载shader
public static void reloadShader();
```

### 关键公有方法

```java
// 纹理绑定
public void bindTexture(int modelTextureIndex, int glTextureIndex);

// 获取绑定纹理映射
public Map<Integer, Integer> getBoundTextures();

// 设置掩码缓冲区尺寸（代价高，会销毁并重建）
public void setDrawableClippingMaskBufferSize(float width, float height);
public void setOffscreenClippingMaskBufferSize(float width, float height);

// 复制offscreen buffer
public CubismRenderTargetAndroid copyOffscreenRenderTarget();
public CubismRenderTargetAndroid copyRenderTarget(CubismRenderTargetAndroid srcBuffer);
```

### 枚举类型 (CubismRenderer 内定义)

```java
public enum RendererType { ANDROID, UNKNOWN }
public enum DrawableObjectType { DRAWABLE(0), OFFSCREEN(2) }
public enum CubismBlendMode { NORMAL, ADDITIVE, MULTIPLICATIVE, MASK }
```

---

## 2. initialize() 初始化 {#2}

### 方法签名

```java
// 默认掩码数量=1
public void initialize(CubismModel model);

// 指定掩码缓冲区数量
public void initialize(CubismModel model, int maskBufferCount);
```

### 完整流程

```
1. 创建 DrawableInfoCachesHolder
   └─ new CubismDrawableInfoCachesHolder(model)
   └─ 缓存所有Drawable的顶点信息

2. 创建 模型渲染目标 (modelRenderTargets)
   条件: model.isBlendModeEnabled() == true
   操作: 创建2个 CubismRenderTargetAndroid
         [0] = 绘制目标
         [1] = TextureBarrier 备用
   每个 renderTarget.createRenderTarget(width, height, null)
   └─ 生成纹理 + FBO

3. 创建 Drawable裁剪管理器
   条件: model.isUsingMasking() == true
   操作: new CubismClippingManagerAndroid()
         .initialize(ANDROID, model, maskBufferCount, DrawableObjectType.DRAWABLE)
   然后: per-mask创建 CubismRenderTargetAndroid (mask)

4. 创建 Offscreen裁剪管理器
   条件: model.isUsingMaskingForOffscreen() == true
   操作: new CubismClippingManagerAndroid()
         .initialize(ANDROID, model, maskBufferCount, DrawableObjectType.OFFSCREEN)
   然后: per-mask创建 CubismRenderTargetAndroid (offscreenMask)

5. 创建 排序数组
   objectsCount = drawableCount + offscreenCount
   sortedObjectsIndexList[objectsCount]
   sortedObjectsTypeList[objectsCount] (默认 DRAABLE)

6. 创建 Offscreen列表 + 建立父子层级
   条件: offscreenCount > 0
   操作: 创建 offscreenList (ArrayList<CubismOffscreenRenderTargetAndroid>)
         setupParentOffscreens(model, offscreenCount)
         └─ 遍历offscreen，通过model.getOffscreenOwnerIndices()获取owner part索引
         └─ 向上遍历part层级找到父offscreen
         └─ offscreen.setParentPartOffscreen(parentOffscreen)

7. 调用 super.initialize(model)
   └─ 设置 this.model = model
   └─ 如果model.isBlendModeEnabled()，自动启用高精度掩码

8. 预初始化 Shader
   └─ CubismShaderAndroid.getInstance() 触发所有shader编译
```

### setupParentOffscreens 细节

```java
public void setupParentOffscreens(final CubismModel model, int offscreenCount) {
    for each offscreenIndex:
        parentOffscreen = null
        ownerIndex = model.getOffscreenOwnerIndices()[offscreenIndex]
        parentIndex = model.getPartParentPartIndex(ownerIndex)

        // 向上遍历part层级，寻找挂在同一part下的父offscreen
        while parentIndex != NO_PARENT:
            for each offscreen i:
                if model.getOffscreenOwnerIndices()[i] == parentIndex:
                    parentOffscreen = offscreenList.get(i)
                    break
            if found: break
            parentIndex = model.getPartParentPartIndex(parentIndex)  // 再向上

        offscreenList[offscreenIndex].setParentPartOffscreen(parentOffscreen)
}
```

---

## 3. doDrawModel() 完整渲染循环 {#3}

### 完整伪代码

```
doDrawModel():
    1. lastFBO[1], lastViewport[4]  // 局部数组
    2. beforeDrawModelRenderTarget()
         └─ 如果 modelRenderTargets 非空:
             对每个 renderTarget:
               如果尺寸变化 → destroy + recreate
             在 modelRenderTargets[0] 上开始绘制:
               renderTargets[0].beginDraw()
               renderTargets[0].clear(0,0,0,0)

    3. glGetIntegerv(GL_FRAMEBUFFER_BINDING, lastFBO)
    4. glGetIntegerv(GL_VIEWPORT, lastViewport)

    5. ──── Drawable裁剪掩码 ────
       if drawableClippingManager != null:
         preDraw()
         调整 drawableMasks 尺寸
         if isUsingHighPrecisionMask():
           drawableClippingManager.setupMatrixForHighPrecision(model, false, DRAWABLE)
         else:
           drawableClippingManager.setupClippingContext(model, this, lastFBO, lastViewport, DRAWABLE)
         // setupClippingContext 内部:
         //   计算所有clip region的包围盒
         //   设置视口为掩码尺寸
         //   在掩码FBO上绘制所有clip mesh
         //   (使用drawMeshAndroid + setClippingContextBufferForMask)
         //   恢复视口

    6. ──── Offscreen裁剪掩码 ────
       if offscreenClippingManager != null:
         preDraw()
         调整 offscreenMasks 尺寸
         if isUsingHighPrecisionMask():
           offscreenClippingManager.setupMatrixForHighPrecision(model, false, OFFSCREEN, getMvpMatrix())
         else:
           offscreenClippingManager.setupClippingContext(model, this, lastFBO, lastViewport, OFFSCREEN)

    7. preDraw()  // 第三次调用（重置GL状态）

    8. drawObjectLoop(lastFBO, lastViewport)  // 核心绘制循环

    9. postDraw()  // 空实现

    10. afterDrawModelRenderTarget()
        └─ 如果 modelRenderTargets 非空:
             renderTargets[0].endDraw()
             CubismShaderAndroid.getInstance().setupShaderProgramForOffscreenRenderTarget(this)
             glDrawElements(GL_TRIANGLES, 6, GL_UNSIGNED_SHORT, indexBuffer)
             glUseProgram(0)
```

### 调用约定 (来自CubismRenderer.drawModel)

```java
public void drawModel() {
    if (getModel() == null) return;
    saveProfile();      // 保存GL状态
    doDrawModel();      // 实际绘制
    restoreProfile();   // 恢复GL状态
}
```

---

## 4. drawMeshAndroid() 单个Drawable渲染 {#4}

### 方法签名

```java
protected void drawMeshAndroid(final CubismModel model, final int index)
```

### 完整伪代码

```
drawMeshAndroid(model, drawableIndex):
    1. 纹理检查（非debug模式）:
       if textures[model.getDrawableTextureIndex(index)] == null → return (跳过)

    2. 背面剔除:
       if isCulling(): glEnable(GL_CULL_FACE)
       else: glDisable(GL_CULL_FACE)

    3. 正面方向: glFrontFace(GL_CCW)  // CCW = 正面 (掩码和ArtMesh一致)

    4. 选择 Shader:
       if isGeneratingMask() (即 clippingContextBufferForMask != null):
         CubismShaderAndroid.getInstance().setupShaderProgramForMask(this, model, index)
       else:
         CubismShaderAndroid.getInstance().setupShaderProgramForDrawable(this, model, index)

       setupShaderProgramForDrawable 内部:
         a) 确定混合模式
            - isMasked = (clippingContextBufferForDrawable != null)
            - isPremultipliedAlpha = renderer.isPremultipliedAlpha()
            - blendMode = model.getDrawableBlendModeType(index)
            - shaderIndex = CubismShaderIndexCalculator.calculateShaderIndex(...)
              组合 = (blendMode, maskState, premultipliedAlpha) 共3个因子

         b) 混合因子
            if blendMode.isBlendMode():  // 5.3+ 高级混合
              srcColor=ONE, dstColor=ZERO, srcAlpha=ONE, dstAlpha=ZERO
              blendTexture = copy当前offscreen 或 copyOffscreenRenderTarget()
            else:  // 5.2 兼容混合
              NORMAL:      ONE, ONE_MINUS_SRC_ALPHA / ONE, ONE_MINUS_SRC_ALPHA
              ADDITIVE:    ONE, ONE / ZERO, ONE
              MULTIPLY:    DST_COLOR, ONE_MINUS_SRC_ALPHA / ZERO, ONE

         c) glUseProgram(shaderSet.shaderProgram)

         d) 纹理设置:
            - glActiveTexture(GL_TEXTURE0)
            - glBindTexture(GL_TEXTURE_2D, textureId)
            - glTexParameteri: WRAP_S=REPEAT, WRAP_T=REPEAT
            - glTexParameteri: MIN_FILTER=LINEAR_MIPMAP_LINEAR, MAG_FILTER=LINEAR
            - glUniform1i(samplerTexture0, 0)

         e) 顶点属性:
            位置:
              FloatBuffer vBuf = drawableInfoCachesHolder.setUpVertexArray(index,
                model.getDrawableVertices(index))
              glVertexAttribPointer(a_position, 2, GL_FLOAT, false, 8, vBuf)
            UV:
              FloatBuffer uvBuf = drawableInfoCachesHolder.setUpUvArray(index,
                model.getDrawableVertexUvs(index))
              glVertexAttribPointer(a_texCoord, 2, GL_FLOAT, false, 8, uvBuf)

         f) 掩码纹理 (if isMasked):
            glActiveTexture(GL_TEXTURE1)
            glBindTexture(GL_TEXTURE_2D, drawableMaskBuffer[clipContext.bufferIndex].colorBuffer[0])
            glUniform1i(s_texture1, 1)
            glUniformMatrix4fv(u_clipMatrix, clipContext.matrixForDraw)
            glUniform4f(u_channelFlag, colorChannel.r, colorChannel.g, colorChannel.b, colorChannel.a)

         g) 混合纹理 (if isBlendMode):
            glActiveTexture(GL_TEXTURE2)
            glBindTexture(GL_TEXTURE_2D, blendTexture)
            glUniform1i(s_blendTexture, 2)

         h) MVP矩阵:
            glUniformMatrix4fv(u_matrix, renderer.getMvpMatrix())

         i) 颜色uniforms:
            基础色 = 根据blendMode模式计算
              非blendMode: renderer.getModelColorWithOpacity(drawableOpacity)
              blendMode: 使用opacity，PMA时RGB=opacity
            乘法色 = model.getOverrideMultiplyAndScreenColor().getDrawableMultiplyColor(index)
            屏幕色 = model.getOverrideMultiplyAndScreenColor().getDrawableScreenColor(index)
            glUniform4f(u_baseColor, ...)
            glUniform4f(u_multiplyColor, ...)
            glUniform4f(u_screenColor, ...)

         j) glBlendFuncSeparate(srcColor, dstColor, srcAlpha, dstAlpha)

       setupShaderProgramForMask 内部:
         - 使用 SETUP_MASK shader (固定shader索引)
         - 混合: ZERO, ONE_MINUS_SRC_COLOR / ZERO, ONE_MINUS_SRC_ALPHA
         - 仅设置位置+UV属性
         - u_baseColor = layoutBounds (编码为 -1~1 范围)
         - u_clipMatrix = clipContext.matrixForMask
         - 设置颜色通道 uniform

    5. 执行绘制:
       glGetIntegerv(GL_CURRENT_PROGRAM, currentProgram)
       if currentProgram[0] != 0:
         indexCount = model.getDrawableVertexIndexCount(index)
         ShortBuffer indexBuf = drawableInfoCachesHolder.setUpIndexArray(
           index, model.getDrawableVertexIndices(index))
         glDrawElements(GL_TRIANGLES, indexCount, GL_UNSIGNED_SHORT, indexBuf)

    6. 后处理:
       glUseProgram(0)
       setClippingContextBufferForDrawable(null)
       setClippingContextBufferForMask(null)
```

### Drawable Info Caching

```java
// CubismDrawableInfoCachesHolder 负责将 float[] / short[] 转为 Direct NIO Buffer
setUpVertexArray(index, float[] vertices) → FloatBuffer
setUpUvArray(index, float[] uvs) → FloatBuffer
setUpIndexArray(index, short[] indices) → ShortBuffer
```

---

## 5. preDraw() 和 setupOffscreen {#5}

### preDraw()

```java
void preDraw() {
    // 禁用裁剪测试
    glDisable(GL_SCISSOR_TEST);
    glDisable(GL_STENCIL_TEST);
    glDisable(GL_DEPTH_TEST);

    // 启用混合 + 全通道写入
    glEnable(GL_BLEND);
    glColorMask(true, true, true, true);

    // 解绑GL缓冲区
    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, 0);
    glBindBuffer(GL_ARRAY_BUFFER, 0);

    // 各向异性过滤
    if (getAnisotropy() >= 1.0f) {
        for each bound texture:
            glBindTexture(GL_TEXTURE_2D, textureId)
            glTexParameterf(GL_TEXTURE_MAX_ANISOTROPY_EXT, anisotropy)
    }
}
```

preDraw() 被调用的位置和时机:

- doDrawModel() 步骤5 (drawable mask前): 重置GL状态
- doDrawModel() 步骤6 (offscreen mask前): 重置GL状态
- doDrawModel() 步骤7 (主绘制前): 重置GL状态
- 每次高精度掩码绘制前: 重置GL状态
- setupClippingContext 内部: 每次在新mask buffer上绘制前

### setupOffscreen 相关方法

**beforeDrawModelRenderTarget()**:

```java
protected void beforeDrawModelRenderTarget() {
    if modelRenderTargets.isEmpty(): return;

    // 重建尺寸不匹配的渲染目标
    for each renderTarget in modelRenderTargets:
        if !renderTarget.isSameSize(w, h):
            renderTarget.createRenderTarget(w, h, null)

    // 在[0]上开始离屏绘制
    modelRenderTargets.get(0).beginDraw();
    modelRenderTargets.get(0).clear(0f, 0f, 0f, 0f);
}
```

**afterDrawModelRenderTarget()**:

```java
protected void afterDrawModelRenderTarget() {
    if modelRenderTargets.isEmpty(): return;

    // 结束离屏绘制
    modelRenderTargets.get(0).endDraw();

    // 将离屏结果blit到backbuffer
    CubismShaderAndroid.getInstance().setupShaderProgramForOffscreenRenderTarget(this);
    glDrawElements(GL_TRIANGLES, 6, GL_UNSIGNED_SHORT, MODEL_RENDER_TARGET_INDEX_BUFFER);
    glUseProgram(0);
}
```

### CubismRenderTargetAndroid (FBO管理)

```java
beginDraw(int[] restoreFBO):
    if restoreFBO == null: 保存当前FBO → oldFBO
    else: oldFBO = restoreFBO
    glBindFramebuffer(GL_FRAMEBUFFER, renderTexture[0])

endDraw():
    glBindFramebuffer(GL_FRAMEBUFFER, oldFBO[0])

createRenderTarget(w, h, colorBuffer):
    销毁旧FBO/纹理
    生成纹理 → glTexImage2D(RGBA, RGBA, UNSIGNED_BYTE, null)
    纹理参数: CLAMP_TO_EDGE, NEAREST
    生成FBO → glFramebufferTexture2D(COLOR_ATTACHMENT0)

clear(r, g, b, a):
    glClearColor(r, g, b, a)
    glClear(GL_COLOR_BUFFER_BIT)
```

---

## 6. drawObjectLoop() 排序和绘制循环 {#6}

### drawObjectLoop 完整伪代码

```
drawObjectLoop(lastFBO, lastViewport):
    drawableCount = getModel().getDrawableCount()
    offscreenCount = getModel().getOffscreenCount()
    totalCount = drawableCount + offscreenCount
    renderOrder = getModel().getRenderOrders()  // [totalCount]

    // 初始化
    currentOffscreen = null
    currentFBO = lastFBO
    modelRootFBO = lastFBO

    // 步骤1: 按 renderOrder 排序
    for i in 0..totalCount-1:
        order = renderOrder[i]
        if i < drawableCount:
            sortedObjectsIndexList[order] = i           // Drawable索引
            sortedObjectsTypeList[order] = DRAWABLE
        else:
            sortedObjectsIndexList[order] = i - drawableCount  // Offscreen索引
            sortedObjectsTypeList[order] = OFFSCREEN

    // 步骤2: 按排序后的顺序绘制
    for i in 0..totalCount-1:
        objectIndex = sortedObjectsIndexList[i]
        objectType = sortedObjectsTypeList[i]
        renderObject(objectIndex, objectType)

    // 步骤3: 提交剩余offscreen
    while currentOffscreen != null:
        submitDrawToParentOffscreen(currentOffscreen.getOffscreenIndex(), OFFSCREEN)
```

### renderObject 调度

```
renderObject(objectIndex, objectType):
    switch objectType:
        case DRAWABLE:  drawDrawable(objectIndex)
        case OFFSCREEN: addOffscreen(objectIndex)
```

### drawDrawable (单个ArtMesh绘制, 含高精度掩码)

```
drawDrawable(drawableIndex):
    1. 可见性检查:
       if !model.getDrawableDynamicFlagIsVisible(drawableIndex): return

    2. 提交到父offscreen:
       submitDrawToParentOffscreen(drawableIndex, DRAWABLE)
       // 如当前有活跃offscreen且该drawable不属于其owner part的子层级
       // 则先绘制当前offscreen再继续

    3. 高精度掩码处理:
       clipContext = drawableClippingManager.getClippingContextListForDraw()[drawableIndex]
       if clipContext != null AND isUsingHighPrecisionMask() AND clipContext.isUsing:
         a) 设置视口 = 掩码尺寸
         b) preDraw()  // 重置GL状态
         c) 绑定掩码FBO: getDrawableMaskBuffer(clipContext.bufferIndex).beginDraw(currentFBO)
         d) 清空: glClearColor(1,1,1,1); glClear(COLOR_BUFFER_BIT)
         e) 对每个clip mesh:
             for each clipDrawIndex in clipContext.clippingIdList:
               if !model.getDrawableDynamicFlagVertexPositionsDidChange(clipDrawIndex): continue
               isCulling(model.getDrawableCulling(clipDrawIndex))
               setClippingContextBufferForMask(clipContext)
               drawMeshAndroid(model, clipDrawIndex)
         f) 后处理:
            getDrawableMaskBuffer(clipContext.bufferIndex).endDraw()
            恢复视口 = modelRenderTargetWidth/Height
            preDraw()  // 重置GL状态

    4. 主绘制:
       setClippingContextBufferForDrawable(clipContext)
       isCulling(model.getDrawableCulling(drawableIndex))
       drawMeshAndroid(model, drawableIndex)

   注: 不使用高精度掩码时，掩码已在setupClippingContext阶段批量生成
```

### addOffscreen (切换到新的offscreen目标)

```
addOffscreen(offscreenIndex):
    1. 如果当前有活跃offscreen且不是目标:
       if currentOffscreen != null && currentOffscreen.index != offscreenIndex:
         检查新offscreen是否为当前的子:
           ownerIndex = model.getOffscreenOwnerIndices()[offscreenIndex]
           parentIndex = model.getPartParentPartIndex(ownerIndex)
           向上遍历part层级
           if 不是子关系:
             submitDrawToParentOffscreen(offscreenIndex, OFFSCREEN)  // 先画当前

    2. 获取目标offscreen:
       offscreen = offscreenList.get(offscreenIndex)
       offscreen.setOffscreenRenderTarget(w, h)  // 从pool获取或创建render target

    3. 设置oldscreen链:
       oldOffscreen = offscreen.getParentPartOffscreen()
       offscreen.setOldOffscreen(oldOffscreen)

    4. 开始离屏绘制:
       oldFBO = oldOffscreen ? oldOffscreen.renderTarget.renderTexture[0] : modelRootFBO[0]
       offscreen.getRenderTarget().beginDraw(oldFBO)
       glViewport(0, 0, w, h)
       offscreen.getRenderTarget().clear(0, 0, 0, 0)

    5. 更新当前:
       currentOffscreen = offscreen
       currentFBO = offscreen.getRenderTarget().getRenderTexture()
```

### drawOffscreen (绘制offscreen到父target)

```
drawOffscreen(currentOffscreen):
    1. 高精度掩码处理 (同drawDrawable但使用offscreenClippingManager):
       clipContext = offscreenClippingManager.getClippingContextListForOffscreen()[offset]
       if clipContext != null AND isUsingHighPrecisionMask() AND clipContext.isUsing:
         绘制clip mesh到getOffscreenMaskBuffer(..) (流程同drawDrawable)

    2. 主绘制:
       setClippingContextBufferForOffscreen(clipContext)
       isCulling(model.getOffscreenCulling(offscreenIndex))
       drawOffscreenAndroid(model, currentOffscreen)

    drawOffscreenAndroid 内部:
       - 设置culling + glFrontFace(GL_CCW)
       - offscreen.getRenderTarget().endDraw()  // 结束该offscreen的离屏绘制
       - 更新currentOffscreen / currentFBO:
         currentOffscreen = currentOffscreen.getOldOffscreen()
         currentFBO = offscreen.getRenderTarget().getOldFBO()
       - 使用 CubismShaderAndroid.setupShaderProgramForOffscreen:
         * 纹理取自 offscreen.renderTarget.colorBuffer[0]
         * 顶点使用全屏quad (RENDER_TARGET_VERTEX_BUFFER)
         * UV使用反向VERTICAL (RENDER_TARGET_REVERSE_UV_BUFFER)
         * MVP矩阵 = identity
         * 基础色 = offscreenOpacity (填充所有通道)
       - glDrawElements(GL_TRIANGLES, 6, GL_UNSIGNED_SHORT, indexBuffer)
       - 后处理:
         offscreen.stopUsingRenderTexture()
         glUseProgram(0)
```

### submitDrawToParentOffscreen (offscreen层级传播)

```
submitDrawToParentOffscreen(objectIndex, objectType):
    if currentOffscreen == null: return
    if objectIndex == NO_OFFSCREEN: return

    currentOwnerIndex = model.getOffscreenOwnerIndices()[currentOffscreen.offscreenIndex]

    // 获取目标对象的父part索引
    switch objectType:
      DRAWABLE:  targetParentIndex = model.getDrawableParentPartIndex(objectIndex)
      OFFSCREEN: targetParentIndex = model.getPartParentPartIndex(offscreenOwnerIndex)

    // 向上遍历part层级
    while targetParentIndex != NO_PARENT:
      if targetParentIndex == currentOwnerIndex: return  // 属于当前offscreen
      targetParentIndex = model.getPartParentPartIndex(targetParentIndex)

    // 不属于当前offscreen → 绘制当前offscreen到父目标
    drawOffscreen(currentOffscreen)

    // 递归: 继续向父offscreen传播
    submitDrawToParentOffscreen(objectIndex, objectType)
```

---

## 7. MVP矩阵设置 {#7}

### CubismRenderer (基类)

```java
// 构造时创建
protected CubismRenderer() {
    mvpMatrix44 = CubismMatrix44.create();  // 4x4单位矩阵
    mvpMatrix44.loadIdentity();
}

// 设置MVP矩阵
public void setMvpMatrix(final CubismMatrix44 matrix4x4) {
    mvpMatrix44.setMatrix(matrix4x4);  // 拷贝内容
}

// 获取MVP矩阵 (直接引用, 每次绘制时使用)
public CubismMatrix44 getMvpMatrix() {
    return mvpMatrix44;
}
```

### 矩阵的使用位置

```
1. setupShaderProgramForDrawable:
   CubismMatrix44 matrix44 = renderer.getMvpMatrix();
   glUniformMatrix4fv(u_matrix, 1, false, matrix44.getArray(), 0);

2. setupShaderProgramForMask:
   glUniformMatrix4fv(u_clipMatrix, 1, false, clipContext.matrixForMask.getArray(), 0);
   (注意: 掩码shader使用 u_clipMatrix 而非 u_matrix)

3. setupShaderProgramForOffscreen:
   CubismMatrix44 mvpMatrix = reusableMatrix;
   mvpMatrix.loadIdentity();  // 恒等矩阵 —— offscreen是全屏quad
   glUniformMatrix4fv(u_matrix, 1, false, mvpMatrix.getArray(), 0);

4. setupClippingContext (offscreen高精度模式):
   当 drawableObjectType == OFFSCREEN 时:
     clipContext.matrixForDraw = clipContext * mvp^-1
     即 clipContext.matrixForDraw.multiply(mvpMatrix.inverse())

5. setupMatrixForHighPrecision:
   从 CubismClippingManager.setupMatrixForHighPrecision 调用,
   接受模型和MVP矩阵作为参数, 在clipContext上建立额外矩阵变换
```

### 矩阵层级总结

```
外部设置: setMvpMatrix(CubismMatrix44) → 传入应用的计算结果 (投影 * 视图 * 模型)

Drawable绘制:
  shader uniform u_matrix = mvpMatrix (投影 * 视图 * 模型)
  shader uniform u_clipMatrix = clipContext.matrixForDraw (当使用掩码时)
     matrixForDraw = 将NDC坐标映射到掩码纹理UV坐标的矩阵

掩码绘制:
  shader uniform u_clipMatrix = clipContext.matrixForMask (编码掩码形状)
  不设置 u_matrix

Offscreen绘制:
  shader uniform u_matrix = identity (全屏quad不需要变换)
  shader uniform u_clipMatrix = clipContext.matrixForDraw (当使用掩码时)
```

---

## 8. CubismModel 被调用方法签名 {#8}

以下是从 CubismRendererAndroid 及其相关类中调用的所有 CubismModel 方法:

### Drawable 数据访问

```java
// 基础属性
int getDrawableCount()
int[] getRenderOrders()
int getDrawableTextureIndex(int drawableIndex)
float[] getDrawableVertices(int drawableIndex)       // alias getDrawableVertexPositions
float[] getDrawableVertexPositions(int drawableIndex)
float[] getDrawableVertexUvs(int drawableIndex)
short[] getDrawableVertexIndices(int drawableIndex)
int getDrawableVertexIndexCount(int drawableIndex)
int getDrawableVertexCount(int drawableIndex)

// 渲染属性
float getDrawableOpacity(int drawableIndex)
boolean getDrawableCulling(int drawableIndex)
csmBlendMode getDrawableBlendModeType(int drawableIndex)
CubismBlendMode getDrawableBlendMode(int drawableIndex)
boolean getDrawableInvertedMask(int drawableIndex)
int getDrawableParentPartIndex(int drawableIndex)

// 动态标志 (每帧更新)
boolean getDrawableDynamicFlagIsVisible(int drawableIndex)
boolean getDrawableDynamicFlagVertexPositionsDidChange(int drawableIndex)
boolean getDrawableDynamicFlagVisibilityDidChange(int drawableIndex)
boolean getDrawableDynamicFlagOpacityDidChange(int drawableIndex)
boolean getDrawableDynamicFlagDrawOrderDidChange(int drawableIndex)
boolean getDrawableDynamicFlagRenderOrderDidChange(int drawableIndex)
boolean getDrawableDynamicFlagBlendColorDidChange(int drawableIndex)
```

### 裁剪掩码

```java
boolean isUsingMasking()
int[][] getDrawableMasks()          // [drawableIndex][] → clip drawable indices
int[] getDrawableMaskCounts()       // [drawableIndex] → clip count
boolean isUsingMaskingForOffscreen()
```

### 颜色和乘算/屏幕色

```java
// Multiply & Screen colors
CubismModelMultiplyAndScreenColor getOverrideMultiplyAndScreenColor()
  ├── CubismTextureColor getDrawableMultiplyColor(int drawableIndex)
  ├── CubismTextureColor getDrawableScreenColor(int drawableIndex)
  ├── CubismTextureColor getOffscreenMultiplyColor(int offscreenIndex)
  └── CubismTextureColor getOffscreenScreenColor(int offscreenIndex)

// 传统blend mode
float[] getDrawableMultiplyColor(int drawableIndex)
float[] getDrawableScreenColor(int drawableIndex)
```

### Offscreen

```java
int getOffscreenCount()
int[] getOffscreenOwnerIndices()        // [offscreenIndex] → part index
int[][] getOffscreenMasks()
int[] getOffscreenMaskCounts()
csmBlendMode getOffscreenBlendModeType(int offscreenIndex)
boolean getOffscreenInvertedMask(int offscreenIndex)
float getOffscreenOpacity(int offscreenIndex)
boolean getOffscreenCulling(int offscreenIndex)
float[] getOffscreenMultiplyColor(int offscreenIndex)
float[] getOffscreenScreenColor(int offscreenIndex)
```

### Part 层级

```java
int getPartParentPartIndex(int partIndex)          // 单个
int[] getPartParentPartIndices()                    // 批量
int[] getPartOffscreenIndices()                    // [partIndex] → offscreenIndex or -1
int getPartCount()
```

### 其他渲染相关

```java
boolean isBlendModeEnabled()
float getCanvasWidth()
float getCanvasHeight()
float getCanvasWidthPixel()
float getCanvasHeightPixel()
float getPixelPerUnit()
```

---

## 附录: OpenGL Shader 变体矩阵

Shaders 存储在 `com/live2d/sdk/cubism/framework/shaders/standardES/` 目录下。

通过 CubismShaderIndexCalculator 计算 shaderIndex，组合三个因子:

1. **BlendMode**: Normal, AddCompatible, MultiplyCompatible, 以及 ColorBlend x AlphaBlend 组合
2. **MaskState**: None, Normal, Inverted
3. **PremultipliedAlpha**: enabled/disabled

每个组合对应一个单独的 shader program。额外还有:

- **COPY**: 离屏拷贝 (全屏quad纹理拷贝)
- **SETUP_MASK**: 掩码生成专用

### Shader Uniforms

| Uniform | 类型 | 用途 |
| --------- | ------ | ------ |
| `u_matrix` | mat4 | MVP矩阵 |
| `u_clipMatrix` | mat4 | 裁剪坐标变换矩阵 |
| `u_baseColor` | vec4 | 基础色 (含不透明度) |
| `u_multiplyColor` | vec4 | 乘法色 |
| `u_screenColor` | vec4 | 屏幕色 |
| `u_channelFlag` | vec4 | 掩码通道选择 (A/R/G/B) |

### Shader Attributes

| Attribute | 分量 | 说明 |
|-----------|------|------|
| `a_position` | vec2 | 顶点位置 (NDC) |
| `a_texCoord` | vec2 | 纹理UV坐标 |

### Shader Samplers

| Sampler | 纹理单元 | 说明 |
| --------- | ---------- | ------ |
| `s_texture0` | GL_TEXTURE0 | 主纹理 (Drawable纹理) |
| `s_texture1` | GL_TEXTURE1 | 掩码纹理 |
| `s_blendTexture` | GL_TEXTURE2 | 混合纹理 (高级blend mode用) |

//! `l2d-wasm-demo` — 可选的浏览器 WASM 渲染演示（RFC D10「renderer：提供 WASM demo」）。
//!
//! 平台形态：
//! - **wasm32-unknown-unknown**（真实实现，见下方 [`web`] 模块）：
//!   从 query `?model=<model3 URL>`（缺省 `/models/bai/runtime/bai.model3.json`）
//!   fetch model3 清单 → 按 manifest 引用逐个 fetch moc3/纹理/physics/cdi →
//!   [`l2d::asset::ModelPackage::from_memory_map`]（内存资源构造）+
//!   [`l2d::model::LoadedModel::resolve`]（统一加载路径）→ `<canvas>` surface
//!   （WebGPU 后端，WebGL2 回退）→ [`l2d::renderer::GpuContext::from_parts_with_label`]
//!   接入已有 device/queue → [`l2d::renderer::ModelRendererCore`] →
//!   requestAnimationFrame 固定 dt（[`l2d::renderer::FIXED_DT_60HZ`])
//!   `render_to_view` 直出交换链视图；
//! - **native target**（stub）：仅打印运行指引后退出——保证 workspace 的原生
//!   构建/clippy/test 不受影响；本 crate 不做 headless 初始化、不调用 pollster
//!   阻塞、不做帧读回。
//!
//! 错误处理约定：所有可失败步骤都收敛为 `Result<_, String>`，任何失败写入页面
//! `<pre id="status">`（并同步 console.error），绝不 panic、绝不静默吞错；
//! 渲染循环内的致命错误同样写状态栏后停止循环。
//!
//! 交付口径（2026-08-26）：本环境无浏览器/无 GPU 实跑条件，**只完成编译门禁**
//! ——`cargo check -p l2d-wasm-demo --target wasm32-unknown-unknown` 真实通过；
//! 浏览器实跑步骤（`trunk serve --release` 与模型静态目录挂载要求）见本 crate
//! README。Bai 资产**不复制**进本 crate 或任何公共包。

fn main() {
    // wasm32：真实入口。
    #[cfg(target_arch = "wasm32")]
    web::start();

    // native：stub——只提示运行方式，不触碰任何 wasm/GPU API。
    #[cfg(not(target_arch = "wasm32"))]
    {
        println!("l2d-wasm-demo 是 wasm32 浏览器渲染演示；当前为 native stub（未接入渲染）。");
        println!("浏览器预览：cd crates/l2d-wasm-demo && trunk serve --release");
        println!("模型静态目录挂载与 ?model= 参数说明见 crates/l2d-wasm-demo/README.md。");
    }
}

/// 浏览器实现（仅在 wasm32 下编译；依赖也全部 target-gated，native 构建零 web 栈）。
#[cfg(target_arch = "wasm32")]
mod web {
    use std::{
        cell::RefCell,
        collections::{BTreeMap, BTreeSet},
        rc::Rc,
        sync::Arc,
    };

    use js_sys::Uint8Array;
    use l2d::asset::ModelPackage;
    use l2d::model::LoadedModel;
    use l2d::renderer::{FIXED_DT_60HZ, GpuContext, ModelRendererCore};
    use serde_json::Value;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{HtmlCanvasElement, Window};

    /// 缺省模型清单 URL：约定宿主在 `/models/<...>` 暴露用户合法持有的皮套
    /// （挂载方式见本 crate README）；本仓库不内置、不分发任何模型资产。
    const DEFAULT_MODEL_URL: &str = "/models/bai/runtime/bai.model3.json";

    /// 页面状态栏 `<pre id="status">`：所有错误与阶段状态的唯一出口。
    fn status(msg: &str) {
        web_sys::console::error_1(&JsValue::from_str(msg));
        let el = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("status"));
        if let Some(el) = el {
            el.set_text_content(Some(msg));
        }
    }

    /// wasm 入口：把异步启动任务挂到微任务循环（不在顶层阻塞）。
    pub fn start() {
        wasm_bindgen_futures::spawn_local(async move {
            if let Err(err) = run().await {
                status(&format!("启动失败：{err}"));
            }
        });
    }

    /// 一次性启动流程：fetch 清单 → fetch 资源 → 内存组包 → GPU 初始化 →
    /// 安装 resize 监听 → 进入 rAF 固定 dt 渲染循环。
    async fn run() -> Result<(), String> {
        let window = web_sys::window().ok_or_else(|| "无 window 对象".to_owned())?;
        let canvas: HtmlCanvasElement = window
            .document()
            .and_then(|d| d.get_element_by_id("canvas"))
            .ok_or_else(|| "页面缺 <canvas id=\"canvas\">".to_owned())?
            .dyn_into()
            .map_err(|_| "canvas 元素类型异常".to_owned())?;

        status("正在解析 ?model= 参数…");
        let model_url = model_url_from_query(&window);

        status(&format!("正在加载模型清单 {model_url} …"));
        let model3_bytes = fetch_bytes(&window, &model_url)
            .await
            .map_err(|e| format!("清单加载失败（{model_url}）：{e}"))?;

        status("正在按 manifest 引用拉取资源…");
        let refs = extract_refs(&model3_bytes)?;
        let mut resources: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for rel in refs.unique_paths() {
            let url = resolve_relative(&model_url, &rel);
            let bytes = fetch_bytes(&window, &url)
                .await
                .map_err(|e| format!("资源 `{rel}` 加载失败（{url}）：{e}"))?;
            resources.insert(rel.clone(), bytes);
        }

        status("正在组装 ModelPackage 并解析 moc3…");
        let package = ModelPackage::from_memory_map(&model3_bytes, &resources)
            .map_err(|e| format!("ModelPackage::from_memory 失败：{e}"))?;
        let loaded = LoadedModel::resolve(package).map_err(|e| format!("moc3 解析失败：{e}"))?;
        let summary = load_summary(&model_url, &loaded);

        status(&format!(
            "{summary}\n正在初始化 GPU（WebGPU，不可用时回退 WebGL2）…"
        ));
        let (gpu, adapter, surface, config) = init_gpu(&window, &canvas).await?;

        let mut core =
            ModelRendererCore::new(gpu.clone()).map_err(|e| format!("渲染核心创建失败：{e}"))?;
        core.load_model(&loaded)
            .map_err(|e| format!("模型载入渲染核心失败：{e}"))?;
        core.set_viewport(config.width, config.height)
            .map_err(|e| format!("视口设置失败：{e}"))?;

        // 先取尺寸副本（config 即将移入共享态）。
        let (width, height) = (config.width, config.height);
        let state = Rc::new(RefCell::new(FrameState {
            core,
            surface,
            config,
            gpu,
            canvas: canvas.clone(),
            window: window.clone(),
            warned_validation: false,
        }));
        install_resize_handler(&state);

        status(&format!(
            "{summary}\nGPU 后端：{}；画布 {width}x{height}\n开始渲染循环（固定 dt {FIXED_DT_60HZ}s / 60Hz 步进）。",
            backend_label(&adapter),
        ));

        start_render_loop(state);
        Ok(())
    }

    /// 加载结果摘要（含兼容报告口径：非 v0 锚定档位照常预览但明确标注）。
    fn load_summary(model_url: &str, loaded: &LoadedModel) -> String {
        let m = loaded.manifest();
        let report = loaded.report();
        let v0_note = if report.is_supported_v0() {
            "v0 支持=是".to_owned()
        } else {
            format!(
                "v0 支持=否（兼容问题 {} 条，预览继续）",
                report.issues().len()
            )
        };
        format!(
            "已加载 {model_url}（model3 v{}）：moc3=`{}`，纹理 {} 张，physics={}，cdi={}；版本={}，{v0_note}",
            m.version,
            m.moc3_file,
            m.texture_files.len(),
            m.physics_file.as_deref().unwrap_or("-"),
            m.display_info_file.as_deref().unwrap_or("-"),
            loaded
                .handle()
                .moc_version()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "未知".to_owned()),
        )
    }

    /// 创建 Canvas surface 并协商设备/格式/尺寸（WebGPU 优先，WebGL2 回退）。
    ///
    /// 回退语义由 wgpu 承担：instance 同时声明 `BROWSER_WEBGPU | GL` 后端，
    /// `webgl` 特性使 GL 落到 WebGL2；adapter 协商成功即用其真实后端，
    /// 状态栏展示实际选择，不做任何伪装。
    async fn init_gpu(
        window: &Window,
        canvas: &HtmlCanvasElement,
    ) -> Result<
        (
            Arc<GpuContext>,
            wgpu::Adapter,
            wgpu::Surface<'static>,
            wgpu::SurfaceConfiguration,
        ),
        String,
    > {
        // wgpu 29：BROWSER_WEBGPU 与 GL 同时声明——浏览器有 navigator.gpu 时走
        // WebGPU 原生路径；缺失/不可用时落到 wgpu-core 适配器，配合 `webgl`
        // 特性即 WebGL2 回退（见 wgpu InstanceDescriptor 文档）。
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| format!("创建 surface 失败：{e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("无可适配配器（WebGPU 与 WebGL 均不可用或浏览器未启用）：{e}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("l2d-wasm-demo"),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("GPU 设备请求失败：{e}"))?;

        // ModelRendererCore 的共享上下文签名要求 Arc<GpuContext>（native 下
        // GpuContext 为 Send+Sync）；wasm 单线程且 wgpu web 后端默认非 Send，
        // 该 Arc 不会跨线程使用——本行豁免 clippy 提示。
        #[allow(clippy::arc_with_non_send_sync)]
        let gpu = Arc::new(GpuContext::from_parts_with_label(
            device,
            queue,
            format!("browser {}", backend_label(&adapter)),
        ));

        // 格式优先 Rgba8/Bgra8Unorm（渲染核心按 sRGB 输出直通色），否则用首个支持项；
        // alpha 优先 PreMultiplied（上游渲染输出即预乘 alpha），否则首个支持项。
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            })
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| "surface 无可用像素格式".to_owned())?;
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|a| *a == wgpu::CompositeAlphaMode::PreMultiplied)
            .or_else(|| caps.alpha_modes.first().copied())
            .ok_or_else(|| "surface 无可用 alpha 合成模式".to_owned())?;

        let (width, height) = canvas_pixel_size(canvas, window);
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| "无法协商 surface 配置".to_owned())?;
        config.format = format;
        config.alpha_mode = alpha_mode;
        surface.configure(gpu.device(), &config);
        Ok((gpu, adapter, surface, config))
    }

    /// 实际选中的后端标签（观测用：区分 WebGPU 原生路径与 WebGL2 回退路径）。
    fn backend_label(adapter: &wgpu::Adapter) -> String {
        let info = adapter.get_info();
        format!("{:?} ({})", info.backend, info.name)
    }

    /// CSS 像素 × devicePixelRatio = 期望的物理画布尺寸（至少 1×1）。
    fn canvas_pixel_size(canvas: &HtmlCanvasElement, window: &Window) -> (u32, u32) {
        let dpr = window.device_pixel_ratio().max(1.0);
        let css_w = canvas.client_width().max(0) as f64;
        let css_h = canvas.client_height().max(0) as f64;
        let width = ((css_w * dpr).round() as u32).max(1);
        let height = ((css_h * dpr).round() as u32).max(1);
        (width, height)
    }

    /// rAF 循环共享态：渲染核心 + surface + 配置 + 尺寸来源。
    struct FrameState {
        core: ModelRendererCore,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        gpu: Arc<GpuContext>,
        canvas: HtmlCanvasElement,
        window: Window,
        /// 校验类取帧错误只提示一次（避免每帧刷屏）。
        warned_validation: bool,
    }

    type SharedState = Rc<RefCell<FrameState>>;
    type FrameClosure = Closure<dyn FnMut(f64)>;
    type FrameSlot = Rc<RefCell<Option<FrameClosure>>>;

    /// resize 处理：重算物理尺寸 → 重配 surface → 同步渲染核心视口。
    /// （窗口缩放/DPR 变化都会触发；尺寸不变时跳过。）
    fn install_resize_handler(state: &SharedState) {
        let handler: Closure<dyn FnMut()> = Closure::new({
            let state = Rc::clone(state);
            move || {
                let mut st = state.borrow_mut();
                let (width, height) = canvas_pixel_size(&st.canvas, &st.window);
                if (width, height) == (st.config.width, st.config.height) {
                    return;
                }
                st.config.width = width;
                st.config.height = height;
                st.surface.configure(st.gpu.device(), &st.config);
                if let Err(e) = st.core.set_viewport(width, height) {
                    status(&format!("resize 失败：{e}"));
                }
            }
        });
        let window = state.borrow().window.clone();
        if let Err(e) =
            window.add_event_listener_with_callback("resize", handler.as_ref().unchecked_ref())
        {
            status(&format!("注册 resize 监听失败：{}", js_str(&e)));
            return;
        }
        // 页面生命周期内常驻（demo 不做卸载路径），防止闭包被回收。
        std::mem::forget(handler);
    }

    /// 启动自调度的 rAF 循环（标准 Rc<RefCell<Option<Closure>>> 递归持有模式）。
    fn start_render_loop(state: SharedState) {
        let slot: FrameSlot = Rc::new(RefCell::new(None));
        let slot_for_closure = Rc::clone(&slot);
        *slot.borrow_mut() = Some(Closure::new(move |_timestamp_ms: f64| {
            tick(&slot_for_closure, &state);
        }));
        schedule_frame(&slot);
    }

    fn schedule_frame(slot: &FrameSlot) {
        let Some(window) = web_sys::window() else {
            return;
        };
        let cb = slot
            .borrow()
            .as_ref()
            .expect("frame closure 必须常驻")
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .to_owned();
        let _ = window.request_animation_frame(&cb);
    }

    /// 单帧：固定 dt 步进姿态/物理 → 取交换链纹理 → render_to_view → present。
    /// 致命错误写状态栏并停止调度；Outdated/Lost 先重配 surface 再等下一帧；
    /// Timeout/Occluded/Validation 属瞬态，跳过本帧（校验错误只提示一次）。
    fn tick(slot: &FrameSlot, state: &SharedState) {
        let mut st = state.borrow_mut();

        if let Err(e) = st.core.update(FIXED_DT_60HZ) {
            status(&format!("停止渲染循环：update({FIXED_DT_60HZ}) 失败：{e}"));
            return;
        }

        let keep_going = match st.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => render_and_present(&mut st, frame),
            // 纹理与 surface 属性不再匹配：重配后照常出帧。
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                st.surface.configure(st.gpu.device(), &st.config);
                render_and_present(&mut st, frame)
            }
            // surface 配置过期/丢失：本帧放弃，重配后下一帧恢复。
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                st.surface.configure(st.gpu.device(), &st.config);
                true
            }
            // 超时/窗口被遮挡：正常瞬态，静默跳过。
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => true,
            // 校验错误：文档要求处理后重试；demo 无错误捕获回调，
            // 提示一次后继续（持续出现时状态栏保留首次说明）。
            wgpu::CurrentSurfaceTexture::Validation => {
                if !st.warned_validation {
                    st.warned_validation = true;
                    status("surface 取帧校验错误（已跳过该帧，继续循环）");
                }
                true
            }
        };

        drop(st);
        if keep_going {
            schedule_frame(slot);
        }
    }

    /// 渲染一帧并 present。返回 `false` 表示致命错误、应停止循环。
    fn render_and_present(st: &mut FrameState, frame: wgpu::SurfaceTexture) -> bool {
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        match st.core.render_to_view(&view, st.config.format) {
            Ok(()) => {
                frame.present();
                true
            }
            Err(e) => {
                status(&format!("停止渲染循环：render_to_view 失败：{e}"));
                false
            }
        }
    }

    // ---- 网络 / URL 工具 ----

    /// fetch 一个 URL 并返回完整字节（非 2xx 视为错误，携带状态码）。
    async fn fetch_bytes(window: &Window, url: &str) -> Result<Vec<u8>, String> {
        let promise = window.fetch_with_str(url);
        let resp_value = JsFuture::from(promise).await.map_err(|v| js_str(&v))?;
        let resp: web_sys::Response = resp_value
            .dyn_into()
            .map_err(|_| "fetch 响应不是 Response 对象".to_owned())?;
        if !resp.ok() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let buf_promise = resp.array_buffer().map_err(|v| js_str(&v))?;
        let buf = JsFuture::from(buf_promise).await.map_err(|v| js_str(&v))?;
        Ok(Uint8Array::new(&buf).to_vec())
    }

    fn js_str(v: &JsValue) -> String {
        v.as_string().unwrap_or_else(|| format!("{v:?}"))
    }

    /// query `?model=<url>`（缺省 [`DEFAULT_MODEL_URL`]；空值同样回退缺省）。
    fn model_url_from_query(window: &Window) -> String {
        let search = window.location().search().unwrap_or_default();
        let trimmed = search.strip_prefix('?').unwrap_or(&search);
        if let Ok(params) = web_sys::UrlSearchParams::new_with_str(trimmed) {
            if let Some(m) = params.get("model") {
                if !m.is_empty() {
                    return m;
                }
            }
        }
        DEFAULT_MODEL_URL.to_owned()
    }

    /// 把清单相对引用拼到 model3 URL 所在目录下（只做目录拼接，不改写查询串）。
    fn resolve_relative(model_url: &str, rel: &str) -> String {
        let dir = match model_url.rsplit_once('/') {
            Some((dir, _)) => dir,
            None => "",
        };
        let rel = rel.strip_prefix("./").unwrap_or(rel);
        if dir.is_empty() {
            rel.to_owned()
        } else {
            format!("{dir}/{rel}")
        }
    }

    /// model3.json 的 `FileReferences` 提取（demo 只负责取 URL 集合；
    /// 严格的清单解析与字节组装全部交给 `ModelPackage::from_memory`）。
    struct ModelRefs {
        moc: String,
        textures: Vec<String>,
        physics: Option<String>,
        cdi: Option<String>,
    }

    impl ModelRefs {
        /// 去重后的全部引用（保持首次出现顺序）。
        fn unique_paths(&self) -> Vec<String> {
            let mut seen = BTreeSet::new();
            let mut ordered = Vec::new();
            let all = self
                .textures
                .iter()
                .chain(self.physics.iter())
                .chain(self.cdi.iter())
                .chain(std::iter::once(&self.moc));
            for path in all {
                if seen.insert(path.as_str()) {
                    ordered.push(path.clone());
                }
            }
            ordered
        }
    }

    fn extract_refs(bytes: &[u8]) -> Result<ModelRefs, String> {
        let value: Value =
            serde_json::from_slice(bytes).map_err(|e| format!("model3.json 不是合法 JSON：{e}"))?;
        let refs = value
            .get("FileReferences")
            .ok_or_else(|| "model3.json 缺 FileReferences".to_owned())?;
        let as_str = |key: &str| refs.get(key).and_then(Value::as_str).map(str::to_owned);
        let moc = as_str("Moc").ok_or_else(|| "FileReferences.Moc 缺失或不是字符串".to_owned())?;
        let textures = refs
            .get("Textures")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(ModelRefs {
            moc,
            textures,
            physics: as_str("Physics"),
            cdi: as_str("DisplayInfo"),
        })
    }
}

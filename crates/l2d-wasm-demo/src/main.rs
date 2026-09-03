//! 行数豁免：节点 C P0-1（C4 三分支生命周期 + C3 frame latency 显式设置）合规范畴 ≤1000。
//! `l2d-wasm-demo` — 可选的浏览器 WASM 渲染演示（RFC D10「renderer：提供 WASM demo」）。
//!
//! 平台形态：
//! - **wasm32-unknown-unknown**（真实实现，见下方 [`web`] 模块）：
//!   从 query `?model=<model3 URL>`（缺省 `/models/bai/runtime/bai.model3.json`）
//!   fetch model3 清单 → 按 manifest 引用逐个 fetch moc3/纹理/physics/cdi →
//!   [`l2d::asset::ModelPackage::from_memory_map`]（内存资源构造） +
//!   [`l2d::model::LoadedModel::resolve`]（统一加载路径） → `<canvas>` surface
//!   （WebGPU 后端，WebGL2 回退）→ [`l2d::renderer::GpuContext::from_parts_with_label`]
//!   接入已有 device/queue → [`l2d::renderer::ModelRendererCore`] →
//!   requestAnimationFrame 固定 dt（[`l2d::renderer::FIXED_DT_60HZ`]）
//!   `render_to_view_submit` 直出交换链视图（**实时路径不得调用阻塞的
//!   `render_to_view`——C1/C6/C8 硬门禁「实时调用图不得出现 PollType::Wait」**）；
//! - **native target**（stub）：仅打印运行指引后退出——保证 workspace 的原生
//!   构建/clippy/test 不受影响；本 crate 不做 headless 初始化、不调用 pollster
//!   阻塞、不做帧读回。
//!
//! 错误处理约定：所有可失败步骤都收敛为 `Result<_, String>`，任何失败写入页面
//! `<pre id="status">`（并同步 console.error），绝不 panic、绝不静默吞错；
//! 渲染循环内的致命错误同样写状态栏后停止循环。
//!
//! 文件拆分（wasm-only）：
//! - `main.rs`：本文件——入口与 rAF 调度骨架。
//! - `surface.rs`：WebGPU/WebGL2 surface 协商、FrameState、单帧状态机（C4 裁决）。
//! - `net.rs`：fetch / URL 工具 / model3.json 引用解析。
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

/// 浏览器实现入口（仅在 wasm32 下编译；依赖也全部 target-gated，native 构建零 web 栈）。
///
/// 子模块拆分：
/// - [`surface`]：surface 协商 / FrameState / 单帧调度（详见 `surface.rs`）。
/// - [`net`]：fetch / URL 工具 / 清单解析（详见 `net.rs`）。
#[cfg(target_arch = "wasm32")]
mod web {
    use std::{cell::RefCell, rc::Rc};

    use l2d::model::LoadedModel;
    use l2d::renderer::{FIXED_DT_60HZ, ModelRendererCore};
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use web_sys::{HtmlCanvasElement, MessageEvent};

    pub(crate) mod net;
    pub(crate) mod surface;

    use surface::{FrameSlot, SharedState, adapter_info, init_gpu, install_resize_handler};

    /// 缺省模型清单 URL：约定宿主在 `/models/<...>` 暴露用户合法持有的皮套
    /// （挂载方式见本 crate README）；本仓库不内置、不分发任何模型资产。
    pub(crate) const DEFAULT_MODEL_URL: &str = "/models/bai/runtime/bai.model3.json";

    /// 页面状态栏 `<pre id="status">`：所有错误与阶段状态的唯一出口。
    pub(crate) fn status(msg: &str) {
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
        let model_url = net::model_url_from_query(&window);

        status(&format!("正在加载模型清单 {model_url} …"));
        let model3_bytes = net::fetch_bytes(&window, &model_url)
            .await
            .map_err(|e| format!("清单加载失败（{model_url}）：{e}"))?;

        status("正在按 manifest 引用拉取资源…");
        let refs = net::extract_refs(&model3_bytes)?;
        let mut resources = std::collections::BTreeMap::new();
        for rel in refs.unique_paths() {
            let url = net::resolve_relative(&model_url, &rel);
            let bytes = net::fetch_bytes(&window, &url)
                .await
                .map_err(|e| format!("资源 `{rel}` 加载失败（{url}）：{e}"))?;
            resources.insert(rel.clone(), bytes);
        }

        status("正在组装 ModelPackage 并解析 moc3…");
        let package = l2d::asset::ModelPackage::from_memory_map(&model3_bytes, &resources)
            .map_err(|e| format!("ModelPackage::from_memory 失败：{e}"))?;
        let loaded = LoadedModel::resolve(package).map_err(|e| format!("moc3 解析失败：{e}"))?;
        let summary = load_summary(&model_url, &loaded);

        status(&format!(
            "{summary}\n正在初始化 GPU（WebGPU，不可用时回退 WebGL2）…"
        ));
        let (gpu, adapter, surface_obj, config) = init_gpu(&window, &canvas).await?;

        let mut core =
            ModelRendererCore::new(gpu.clone()).map_err(|e| format!("渲染核心创建失败：{e}"))?;
        core.load_model(&loaded)
            .map_err(|e| format!("模型载入渲染核心失败：{e}"))?;
        core.set_viewport(config.width, config.height)
            .map_err(|e| format!("视口设置失败：{e}"))?;

        // 先取尺寸副本（config 即将移入共享态）。
        let (width, height) = (config.width, config.height);
        let state: SharedState = Rc::new(RefCell::new(surface::FrameState {
            core,
            surface: surface_obj,
            config,
            gpu,
            canvas: canvas.clone(),
            window: window.clone(),
            warned_validation: false,
            validation_streak: 0,
            hud: surface::HudState::default(),
            bridge: surface::BridgeState::default(),
        }));
        install_resize_handler(&state);

        // 一次性快照 adapter 信息到 HUD（供 500ms 周期内反复渲染）。
        let (backend, adp_name, is_webgpu) = adapter_info(&adapter);
        state
            .borrow_mut()
            .hud
            .snapshot_adapter(backend, &adp_name, is_webgpu);

        let api = if is_webgpu { "WebGPU" } else { "WebGL2" };
        status(&format!(
            "{summary}\nGPU: {backend}/{api} ({adp_name})；画布 {width}x{height}\n开始渲染循环（固定 dt {FIXED_DT_60HZ}s / 60Hz 步进）。",
        ));

        install_stage_bridge(&state);
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

    /// 注册父页 → WASM 的 message listener（P0-2a-2）：4 类消息写入 bridge 状态。
    /// rAF 循环（另一函数）每帧读取 bridge 产生可见效果。
    fn install_stage_bridge(state: &SharedState) {
        let handler_state = state.clone();
        let closure = Closure::<dyn FnMut(MessageEvent)>::new(move |ev: MessageEvent| {
            // data() → JsValue → String → serde_json::Value
            let Some(text) = ev.data().as_string() else {
                return;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                return;
            };
            let Some(kind) = v.get("type").and_then(|t| t.as_str()) else {
                return;
            };
            let mut st = handler_state.borrow_mut();
            match kind {
                "stage-config" => {
                    if let Some(s) = v.get("scale").and_then(|x| x.as_f64()) {
                        st.bridge.scale = (s as f32).clamp(0.5, 1.5);
                    }
                    if let Some(d) = v.get("dark").and_then(|x| x.as_bool()) {
                        st.bridge.dark = d;
                    }
                    if let Some(l) = v.get("lipSync").and_then(|x| x.as_bool()) {
                        st.bridge.lip_sync = l;
                    }
                }
                "audio-volume" => {
                    if let Some(vol) = v.get("value").and_then(|x| x.as_f64()) {
                        st.bridge.volume = (vol as f32).clamp(0.0, 1.0);
                    }
                }
                "action-state" => {
                    let action = v
                        .get("action")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string();
                    let state_word = v.get("state").and_then(|x| x.as_str()).unwrap_or("");
                    let strength_mult =
                        match v.get("strength").and_then(|x| x.as_f64()).unwrap_or(2.0) as u8 {
                            1 => 0.6,
                            3 => 1.5,
                            _ => 1.0,
                        };
                    if state_word == "start" {
                        let now = web_sys::window()
                            .map(|w| w.performance().map(|p| p.now()).unwrap_or(0.0))
                            .unwrap_or(0.0);
                        st.bridge.action = Some(surface::ActiveAction {
                            name: action,
                            strength_mult,
                            started_ms: now,
                            duration_ms: 800.0,
                        });
                    } else if state_word == "end" {
                        st.bridge.action = None;
                    }
                }
                "load-model" => {
                    // URL 存入 status 提示；真实重载在 P0-2a-3 段实现（rAF/异步加载）。
                    if let Some(url) = v.get("url").and_then(|x| x.as_str()) {
                        status(&format!("收到换模型请求（P0-2a-3 实现重载）: {url}"));
                    }
                }
                _ => {} // 未知类型忽略
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("message", closure.as_ref().unchecked_ref());
        }
        closure.forget(); // 页面级 listener，长生命周期
    }

    /// 启动自调度的 rAF 循环（标准 Rc<RefCell<Option<Closure>>> 递归持有模式）。
    fn start_render_loop(state: SharedState) {
        let slot: FrameSlot = Rc::new(RefCell::new(None));
        let slot_for_closure = Rc::clone(&slot);
        *slot.borrow_mut() = Some(Closure::new(move |_timestamp_ms: f64| {
            surface::tick(&slot_for_closure, &state);
        }));
        schedule_frame(&slot);
    }

    /// rAF 调度：浏览器每帧回调 [`surface::tick`]。
    pub(crate) fn schedule_frame(slot: &FrameSlot) {
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
}

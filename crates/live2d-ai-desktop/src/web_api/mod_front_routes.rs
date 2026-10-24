//! 前置路由的**编译期分派**（0.4.6 带 Mod 的 Windows 部署）。
//!
//! 三条前置路由（外部事件 / 语音转写 / A2A）在缺省提交里是**未挂载存根**
//! （命中即 404 `mod_disabled`）；只有打包临时树（`feature = "packed-mods"`，
//! 七个 Mod crate 静态链进本 binary）才切到真实宿主接线。
//!
//! 为什么用 Cargo feature 而不是运行时判断：主仓缺省树**不引用任何 Mod 实现**
//! （`AVAILABLE_MOD_FACTORIES` 为空、`[dependencies]` 18 条），所以接线代码
//! 在缺省树里根本不能引用 `live2d_ai_mod_*` 的符号；打包脚本把七个依赖改成
//! optional 并打开 `packed-mods`，接线代码才参与编译。缺省树的三条路径仍是
//! 存根 404、外部注入泵不起——与 `docs/versions/0.4.md` §0.4.6 一致。
//!
//! 本文件**只做转发**：不复制校验 / 队列 / 协议逻辑。

use std::io::Cursor;

use tiny_http::{Method, Response};

use crate::web_api::ServerContext;

/// 外部文本注入端点：缺省 = 未挂载存根；打包树 = 入队 + 泵线程。
pub fn handle_external_chat(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
    auth_header: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    #[cfg(feature = "packed-mods")]
    {
        crate::web_api::packed_mods::external::handle_external_chat(
            ctx,
            method,
            path,
            body,
            origin,
            content_type,
            auth_header,
        )
    }
    #[cfg(not(feature = "packed-mods"))]
    {
        crate::web_api::external_routes::handle_external_chat(
            ctx,
            method,
            path,
            body,
            origin,
            content_type,
            auth_header,
        )
    }
}

/// 语音转写端点：缺省 = 未挂载存根；打包树 = 真实宿主门。
pub fn handle_voice_transcript(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
    auth_header: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    #[cfg(feature = "packed-mods")]
    {
        crate::web_api::packed_mods::voice::handle_voice_transcript(
            ctx,
            method,
            path,
            body,
            origin,
            content_type,
            auth_header,
        )
    }
    #[cfg(not(feature = "packed-mods"))]
    {
        crate::web_api::voice_routes::handle_voice_transcript(
            ctx,
            method,
            path,
            body,
            origin,
            content_type,
            auth_header,
        )
    }
}

/// A2A 前置路由：缺省 = 未挂载存根（直接 respond）；打包树 = 宿主门
/// （GET card 在循环内直答，POST 派生工作线程等上游）。
///
/// `respond` 由 `run_request_loop` 传入（内含 `respond_and_log`，保留请求级
/// 日志）；缺省分支同步调用，打包树 POST 分支把它移进工作线程。
#[allow(clippy::too_many_arguments)]
pub fn handle_a2a_request<F>(
    ctx: ServerContext,
    method: Method,
    path: String,
    body: String,
    origin: Option<String>,
    content_type: Option<String>,
    respond: F,
) where
    F: FnOnce(Response<Cursor<Vec<u8>>>) + Send + 'static,
{
    #[cfg(feature = "packed-mods")]
    {
        if method == Method::Get {
            let resp = crate::web_api::packed_mods::a2a::handle_a2a(
                &ctx,
                crate::web_api::packed_mods::a2a::A2aRequest {
                    method: &method,
                    path: &path,
                    body: &body,
                    origin: origin.as_deref(),
                    content_type: content_type.as_deref(),
                },
            );
            respond(resp);
        } else {
            crate::web_api::packed_mods::a2a::spawn_a2a(
                ctx,
                method,
                path,
                body,
                origin,
                content_type,
                respond,
            );
        }
    }
    #[cfg(not(feature = "packed-mods"))]
    {
        // 存根不读这些字段：显式点名，避免「未使用变量」掩盖真实分派。
        let _ = (method, path, body, origin, content_type);
        respond(crate::web_api::a2a_gate::handle_a2a(&ctx));
    }
}

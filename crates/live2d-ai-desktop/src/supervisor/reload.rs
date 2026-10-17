//! 热重载装配：从磁盘配置重建 LLM/TTS client + 表演层 + 二路清洗。
//!
//! 从 supervisor.rs 拆出（2026-10-16，D3「先只搬家」）。父模块再导出
//! build_sentence_cleaner（tests_performance.rs 直接点名它），其余函数只在
//! supervisor 子树内可见。**行为一行未改**。

use std::sync::Arc;

use live2d_ai_runtime::cleaning::SentenceCleaner;
use live2d_ai_runtime::performance::PerformanceRuntime;
use live2d_ai_runtime::{AppSettings, ConversationConfig, ConversationEngine, OpenAiClient};

use crate::app_event::AppEvent;

use super::dialogue_history;
use super::performance_wiring::{
    CleanEndpoint, DirectorTakeover, build_performance_runtime, clean_endpoint,
    load_director_takeover, motion_on,
};
use super::{Emit, PerformanceStatsSlot};

/// 从磁盘最新配置构造新的 [`OpenAiClient`] + [`ConversationConfig`]（热重载核心）。
///
/// **不**触碰 [`ConversationEngine::history`]（旧 engine 跨重建保留历史不现实——
/// `ConversationEngine` 是持 client 的；新 engine 从空历史开始，旧 engine 直接 drop，
/// 已 `commit_completed_turn` 写入的内容**不会**跨 reload 保留——这是公开的语义）。
///
/// 失败路径（文件 IO / TOML 解析 / `base_url` 非法 / env 解析 / reqwest 构造）
/// 全部以 [`String`] 形式回传，**不**修改调用方的旧 engine（保留旧配置继续运行）。
#[allow(clippy::type_complexity)]
fn rebuild_engine_from_config(
    config_path: &str,
    _emit: &Emit,
) -> Result<
    (
        OpenAiClient,
        ConversationConfig,
        Option<Arc<PerformanceRuntime>>,
        Option<Arc<SentenceCleaner>>,
        DirectorTakeover,
    ),
    String,
> {
    // 1) 读盘 + 解析。
    let settings =
        AppSettings::load_from_path(config_path).map_err(|e| format!("读取/解析配置失败: {e}"))?;
    // 2) 解析环境变量（用生产 lookup，与 chat 装配同语义）。
    let resolved = settings
        .resolve_with(live2d_ai_runtime::secrets::lookup)
        .map_err(|e| format!("配置解析失败（URL/环境变量名）: {e}"))?;
    // 3) 构造新 client。
    let client = OpenAiClient::new(resolved.llm.clone(), resolved.tts.clone())
        .map_err(|e| format!("构建 LLM/TTS 客户端失败: {e}"))?;
    // 表演层（T8；2026-10-08 起显式 opt-in）：开关 = `[performance].enabled`；
    // 导演启用时端点按 `staging_*` 两项共用 `takeover_of` 判定（两项都空 = 复用
    // 对话模型），导演未启用时用 `[performance]` 自己的 base_url/model。
    let director = load_director_takeover(config_path);
    let performance = build_performance_runtime(&settings.llm, &resolved.performance, &director);
    // 二路按句清洗（2026-10-08；2026-10-13 起动作门可改用 staging 端点）：
    // `resolved.llm.clean_tts` 由 `AppSettings::resolve` 置 true（产品默认）。
    let cleaner = build_sentence_cleaner(&client, &director);
    Ok((
        client,
        resolved.conversation,
        performance,
        cleaner,
        director,
    ))
}

/// 按句规则回退闭包（单一真源 = director 的 `rule_cues_for_sentence`）。
///
/// **只在动作门开着时**注入：实参是这一句一路原文 + 句号，绝不接用户句。
pub(super) fn sentence_rule_for(
    director: &DirectorTakeover,
) -> Option<live2d_ai_runtime::cleaning::SentenceRuleFallback> {
    motion_on(director).then(|| {
        live2d_ai_runtime::cleaning::SentenceRuleFallback::new(|text: &str, seq: u64| {
            live2d_ai_mod_director::presets::rule_cues_for_sentence(text, seq)
        })
    })
}

/// 二路清洗装配（唯一装配点；启动与热重载共用）。
///
/// 端点 / 模型 / 密钥**全部来自一路**（`client.llm()`）——模型定死同一份，
/// 不再给用户第二个下拉框。`clean_tts == false`（`LlmConfig::new` 的便捷构造）
/// → `None`：主链保持既有行为（上屏 == 送 TTS == `clean_for_tts(句)`）。
pub(crate) fn build_sentence_cleaner(
    client: &OpenAiClient,
    director: &DirectorTakeover,
) -> Option<Arc<SentenceCleaner>> {
    let llm = client.llm();
    let motion = motion_on(director);
    let allowed = || live2d_ai_mod_director::presets::accepted_preset_ids();
    // 表演倾向（2026-10-13 执行单 T3）：真源 = director 的 `motion_bias`，两条派生值。
    let motion_bias = director.motion_bias.to_runtime_bias(&allowed().join(", "));
    let cleaner = match clean_endpoint(director) {
        // 动作门 Own：这一次按句清洗改用 staging 端点。密钥变量名空则复用主 LLM
        // 的**已解析**密钥；否则经 `secrets::lookup`（永不落日志）。
        CleanEndpoint::Staging => {
            let key = if director.staging_api_key_env.is_empty() {
                llm.api_key.clone()
            } else {
                live2d_ai_runtime::secrets::lookup(director.staging_api_key_env.trim())
                    .map(live2d_ai_runtime::ApiSecret::from)
            };
            SentenceCleaner::from_parts(
                &director.staging_base_url,
                &director.staging_model,
                key,
                live2d_ai_runtime::cleaning::CLEAN_TIMEOUT_MS,
                motion,
                allowed(),
            )
            .with_bias(motion_bias)
        }
        // 复用对话模型（或动作门没开）：端点仍是主 LLM。
        CleanEndpoint::Conversation => {
            let base = SentenceCleaner::from_llm(llm);
            if motion {
                base.with_motion(allowed(), motion_bias)
            } else {
                base
            }
        }
    };
    if !cleaner.enabled() {
        return None;
    }
    tracing::info!(
        model = cleaner.model(),
        motion = cleaner.motion(),
        bias = director.motion_bias.as_str(),
        "二路按句清洗已装配（动作门；请求体显式关思考）"
    );
    Some(Arc::new(cleaner))
}

/// 应用 Reload：从最新配置重建 engine；失败保留旧 engine。
///
/// 抽成独立函数供两处复用：
/// 1. `run_forever` 空闲态 select 直接处理（无 in-flight turn 时）；
/// 2. `run_one_turn` 收口后兜底（处理在飞期间积压的 Reload）。
///
/// `config_path: None` 时静默 no-op（web 模式 / 测试装配）。
///
/// `reload_pending: Some(&AtomicBool)` 时**消费**标志：成功读置 false；
/// 读置 false 后**才**执行 rebuild（避免在 idle select 之外做无谓 IO）。
pub(super) fn apply_reload(
    engine: &mut ConversationEngine,
    config_path: Option<&str>,
    reload_pending: Option<&std::sync::atomic::AtomicBool>,
    performance_stats: &PerformanceStatsSlot,
    histories: &mut dialogue_history::DialogueHistories,
    emit: &Emit,
) -> Result<(), String> {
    // 消费标志：仅在需要应用时才清。失败路径不重置——保留以便下次再试。
    let should_apply = reload_pending
        .map(|f| f.swap(false, std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(true);
    if !should_apply {
        return Ok(());
    }
    let Some(path) = config_path else {
        tracing::debug!("Reload 收到但 config_path 未设置（web 模式 / 测试），no-op");
        return Ok(());
    };
    match rebuild_engine_from_config(path, emit) {
        Ok((new_client, new_conversation, performance, cleaner, director)) => {
            // 表演层 / 二路清洗与引擎同一份配置解析：热重载后说话人、cue 源、
            // 清洗模型一起换。
            let mut new_engine = ConversationEngine::new(new_client, new_conversation);
            new_engine.set_performance(performance.clone());
            new_engine.set_cleaner(cleaner);
            new_engine.set_motion(motion_on(&director));
            new_engine.set_sentence_rule(sentence_rule_for(&director));
            *engine = new_engine;
            // 新引擎是空历史。分桶必须一起丢掉，否则下一轮会把旧桶装回去。
            histories.clear();
            if let Ok(mut slot) = performance_stats.lock() {
                *slot = performance.map(|rt| rt.stats());
            }
            tracing::info!(path, "热重载成功：已重建 LLM/TTS client（含表演层）");
            Ok(())
        }
        Err(e) => {
            // 失败回退：旧 engine 不动（保留旧配置继续运行），但**不许静默**。
            // 2026-10-16（S-001 / S-007）：这条路是「文件已改、引擎没跟上」的
            // **唯一**失败点——PATCH 保存与「用户手改 live2d-ai.toml」都汇到这里。
            // 所以两条出口一起给：
            // ① 既有 error 帧（前端既有错误横幅）——复用，不新做通知系统；
            // ② 返回值交给等待中的调用方（PATCH 的 apply_status，见
            //    SupervisorHandle::reload_reported），界面不再写「已保存并生效」。
            tracing::error!(error = %e, path, "热重载失败：保留旧配置继续运行");
            // 五字段由 AppErrorEvent 的**唯一**构造点给出（与「无 supervisor 时
            // 手改坏 toml」那条出口共用），文案与行为不变。
            emit(AppEvent::Error(
                crate::app_event::AppErrorEvent::config_reload_failed(format!(
                    "配置已保存到磁盘，但引擎没能用上它：{e}"
                )),
            ));
            Err(e)
        }
    }
}

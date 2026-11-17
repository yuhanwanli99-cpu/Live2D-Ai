//! 「记」的网络半边：一次短 JSON 抽取（2026-11-16 热槽记忆 M4）。
//!
//! # 它为什么在宿主侧
//!
//! 抽取要发 HTTP（用主链 `[llm]`，**不新开端点、不新开密钥**），而
//! `live2d_ai_runtime::hotmem` 是纯函数层（不出网络）。所以网络在宿主，
//! 解析与折叠在 runtime——`hotmem::parse_recalled` / `fold_summary`。
//!
//! # 三条纪律
//!
//! 1. **失败只记日志**：抽不出来就记一条 episode 原文，不让这一轮失败；
//! 2. **不重试**：计划 §3「抽失败就只把这一轮原文记成 episode，不再调用第二次」；
//! 3. **不阻塞说话**：调用方在轮结束后把它丢进后台，HTTP 响应不等它。
//!
//! # 与「说」共用同一个 client
//!
//! `OpenAiClient` 的 clone 只是 `Arc` 化的连接池（见它的文档），所以这里
//! 直接拿主链那份，不新建 reqwest client、不读第二份配置。

use live2d_ai_runtime::OpenAiClient;
use live2d_ai_runtime::hotmem::{
    RECALL_SYSTEM_PROMPT, Recollection, parse_recalled, recall_user_message,
};
use live2d_ai_runtime::{ChatMessage, collect_chat_text};

/// 一次抽取的时间上限（毫秒）。超时 = 抽取失败 → 走 episode 兜底。
///
/// 取值理由：这是后台任务，慢一点不影响说话；但也不该让一个卡住的上游把
/// 后台线程无限期占着。10 秒够一次短 JSON 生成，又短到不会堆积。
pub const RECALL_TIMEOUT_MS: u64 = 10_000;

/// 抽取结果（成功 / 失败都不 panic，失败带一句可日志的原因）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recalled {
    /// 抽到了事实（可能为空表 = 模型认为没什么值得记的）。
    Items(Vec<Recollection>),
    /// 抽取失败（网络 / 解析 / 超时）；调用方走 episode 兜底。
    Failed(String),
}

/// 用主链 client 跑一次抽取。
///
/// **没有第二次调用**：任何失败都直接返回 [`Recalled::Failed`]。
pub async fn recall_once(client: &OpenAiClient, user: &str, assistant: &str) -> Recalled {
    let messages = vec![
        ChatMessage::system(RECALL_SYSTEM_PROMPT),
        ChatMessage::user(recall_user_message(user, assistant)),
    ];
    // **整个**抽取（建连 + 读流）都受同一个超时管：只包读流的话，
    // 「上游接受连接但永不回包头」会让这个后台任务无限期挂着。
    let budget = std::time::Duration::from_millis(RECALL_TIMEOUT_MS);
    let collected = tokio::time::timeout(budget, async {
        let stream = client
            .chat_stream(&messages)
            .await
            .map_err(|e| format!("抽取请求失败: {e}"))?;
        collect_chat_text(stream)
            .await
            .map_err(|e| format!("抽取流失败: {e}"))
    })
    .await;
    let text = match collected {
        Ok(Ok(t)) => t,
        Ok(Err(reason)) => return Recalled::Failed(reason),
        Err(_) => return Recalled::Failed("抽取超时".to_string()),
    };
    let items = parse_recalled(&text);
    if items.is_empty() && !text.trim().is_empty() {
        // 模型回了一段非空文本但不是合法数组 → 抽失败（走 episode 兜底）。
        return Recalled::Failed(format!(
            "抽取结果不是 JSON 数组: {}",
            first_chars(&text, 80)
        ));
    }
    Recalled::Items(items)
}

fn first_chars(text: &str, n: usize) -> String {
    text.chars().take(n).collect()
}

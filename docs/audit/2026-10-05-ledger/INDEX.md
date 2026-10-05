# INDEX.md — 文件 → 批次 → 一句结论
> 覆盖率看这里。每批关闭后追加。未审 = 空。
> 合计队列 483（crates 266 / flutter lib 115 / flutter test 102）

## crates/live2d-ai-desktop/src/web_api/** (49) —— 见下方各批次；另有 mod_registry.rs（属 desktop crate 根，本仓 Rust 最大单文件之一）

### BATCH-0001（7 文件 / 2083 行 · P0 0 P1 2 P2 3 P3 2）

### BATCH-0002（4 文件 / 1336 行 · P0 0 P1 2 P2 3 P3 2）

### BATCH-0003（4 文件 / 1175 行 · P0 0 P1 0 P2 1 P3 3）

### BATCH-0004（1 文件 / 1086 行 · P0 0 P1 0 P2 2 P3 1）

### BATCH-0005（2 文件 / 1560 行 · P0 0 P1 0 P2 1 P3 1）

### BATCH-0006（3 文件 / 1568 行 · P0 0 P1 0 P2 3 P3 1）

### BATCH-0007（3 文件 / 826 行 · P0 0 P1 0 P2 1 P3 1）

### BATCH-0008（2 文件 / 771 行 · P0 0 P1 0 P2 1 P3 1）

### BATCH-0009（2 文件 / 718 行 · P0 0 P1 0 P2 1 P3 1）

### BATCH-0010（3 文件清单 / 细读 400 行 · P0 0 P1 0 P2 0 P3 1）

### BATCH-0011（4 文件 / 2029 行 · P0 0 P1 0 P2 0 P3 2）

### BATCH-0012（1 主文件 + 2 测试名枚举 · P0 0 **P1 1（升级）** P2 0 P3 0）

### BATCH-0013（2 文件 · P0 0 **P1 1** P2 0 P3 1）

### BATCH-0014（3 文件 / 851 行 · P0 0 P1 0 P2 0 P3 1）
- `web_api/tests_dev_mode.rs` → B0014 · web_api 测试质量上界：三态矩阵全覆盖、双向翻 403↔200、同时断言内存态与磁盘、临时路径唯一化。全读，零发现
- `web_api/tests_mod.rs` → B0014 · 14 条零模式 B，但路由稳定性测试独独漏掉 RouteId::Env（rc.2 唯一新增路由·密钥端点）(F-0014-01)；:194-199 记录了一次主动消除机器状态依赖的修复
- `web_api/tests_ws_client.rs` → B0014 · helper 模块，2 条测试（握手残留字节 / 101 与首帧同批）名实相符
- `crates/live2d-ai-desktop/src/mod_registry.rs` → B0013 · persist_manifest 用内存注册表重建整份 mods.json，读侧也从不保留未知 id ⇒ 红线 R「不得丢未知 id」被违反，且 local-llm 移除有先例 (F-0013-01)；唯一的 manifest 往返测试起点是空文档 (F-0013-02)。读 190-509 + 688-712 + 938-987
- `web_api/cli_entry.rs` → B0013 · 读 604-643：mods_manifest_for_web 整份读入但只被逐 id 消费（未知 id 丢失的源头之一）；mods_path 与 toml 同目录；缺省启用 external-input 的口径与 AGENTS.md 一致
- `crates/live2d-ai-desktop/src/mod_registry.rs` → B0012 · start_one 对 api_version 不兼容的处理是「Failed+last_error+warn」不崩主链（登记的 S4 可能已部分修复）；但 disable/enable/shutdown 三处 .lock().unwrap() 在 web 请求路径上 → 毒化即打死 accept 循环 (F-0006-03 升级 P1)。读 306-509 + grep
- `web_api/tests_reload.rs` → B0012 · 4 条测试名无自相矛盾；两条直接测 refresh_from_disk **函数**（不是接线）
- `web_api/tests_p0c.rs` → B0012 · 10 条测试名无自相矛盾；含首次配置闭环的 e2e，但同样只测函数不测 file_watcher 装配
- `web_api/tests_security.rs` → B0011 · 30+ 条全为真测试（正负用例齐、无析取式恒真），模式 B 未命中。读 230-398
- `web_api/tests_security_e2e.rs` → B0011 · 13 条全走真实 dispatch 且断言 body 里的 code；两条真写固定 /tmp 路径不清理 (F-0011-01)、一条自述重复的零增量测试 (F-0011-02)
- `web_api/tests_ws.rs` → B0011 · 28 条测试名全部承诺可观察行为，抽读 3 条均为真测试；:776-777 证伪了 F-0003-04 的指控部分。读 700-796 + 全量测试名
- `web_api/settings_routes/test_endpoints.rs` → B0011 · 自检口径与 AGENTS.md 逐条对齐（TTS 不合成/404 算通过/超时文案明说不等于不能出声）、密钥走 secrets::lookup、响应体截 512 字符
- `web_api/settings_routes/mod.rs` → B0010 · to_toml_string_merging 保住注释（2026-09-10 缺陷未回退）、原子写+tmp 清理、全程无 std::env::var；稳定错误码由中文消息子串派生但有端到端测试兜住 (F-0010-01)。读 1-260
- `web_api/settings_routes/tests.rs` → B0010 · 三态 PATCH 端到端真测试，且主动规避「钉死策略常量」反模式；读 400-599，模式 B 复查未完成
- `web_api/dto.rs` → B0009 · 错误体三段式与 ApplyStatus 锚点都锁得住（code 用 &'static str）；app.status.audio 写死 none/false 与 DTO 注释矛盾且被 Dart 诊断面板直接显示 (F-0009-01)。读 1-240
- `web_api/models_routes/dto.rs` → B0009 · 请求体上用 deny_unknown_fields 是正确取舍（与 F-0007-01 的持久化侧形成对照）；ImportRequest 是唯一漏标的 (F-0009-02)。读 1-150
- `web_api/models_routes/mod.rs` → B0008 · active_model_id 是纯决策内核+穷举测试（模式 B 正面对照）；写锁纪律与 persist 回滚正确；registry 损坏告警只走 eprintln 不进日志 sink (F-0008-02)
- `web_api/models_routes/handlers.rs` → B0008 · resolve_safe_path 双方 canonicalize + 前缀校验写得扎实，三条写路径都有锁纪律与回滚；每请求对每个模型目录全递归 stat 求 size_bytes (F-0008-01)。:330-427 未读
- `web_api/env_routes.rs` → B0007 · 红线 R 全过：GET 只回 section/key/set、PUT 白名单与 GET 同一真源、日志只记 key、错误不含 value。本批未读 :183-380 的测试段
- `web_api/models_routes/registry.rs` → B0007 · 原子写回(tmp+sync_data+rename)与 id 严格白名单都写对了；6 处 deny_unknown_fields 使每次 schema 演进清空用户导入列表 (F-0007-01)
- `web_api/models_routes/util.rs` → B0007 · model3.json 查找确定且只下一层、compute_dir_size 不展开符号链接；unix→日历/ISO 格式化在本 crate 有三份实现 (F-0007-02)
- `web_api/mods_routes.rs` → B0006 · 五种 action 错误码齐、CT 只在带 body 时校验（取舍正确）；redacted_config 只删 spec 同名顶层键且 spec=None 时零脱敏，与头注「永不出现」不符 (F-0006-01)、每请求 Box::leak (F-0006-02)、锁中毒 .expect 会打死 accept 循环且与 external/voice 口径相反 (F-0006-03)
- `web_api/ws/audio.rs` → B0006 · 20ms 切片/引擎给句界/空末块边界帧/静音置零而 volume 真实，四条音频契约全部写对；AUDIO_SLICE_MS 的 allow(dead_code) 与在用事实不符 (F-0006-04)
- `web_api/ws/audio_tests.rs` → B0006 · 本仓测试质量上界：句界「恰好一个 start 一个 end」用真 broadcaster 逐帧钉死，静音三契约逐帧断言，零恒真断言
- `web_api/external_routes.rs` → B0005 · token 优先级/CT/Origin/三态齐备且都走 secrets::lookup；Mod 不在注册表时门禁与鉴权双双静默消失 (F-0005-01)
- `web_api/voice_routes.rs` → B0005 · 唤醒/手动/PTT 三闸判定顺序钉死且复用 Mod 纯函数；空 token 语义经 effective_token 正确处理（已核实非缺陷）；骨架与 external 重复 (F-0005-02)
- `web_api/chat_routes.rs` → B0004 · 会话表唯一、epoch 取真值、测试质量高；429 拒绝路径已改状态并发帧但响应体不含该信息 (F-0004-01)、1086 行越过 1000 豁免上限而头注仍写「约 560 行」 (F-0004-02)、CT/Origin 判定顺序与状态码两套口径 (F-0004-03)
- `web_api/ws.rs` → B0003 · 升级三态齐（426/403/503）且各有稳定 code；Origin 403 与慢订阅者丢弃只落 debug!，与 dispatch 的 warn! 口径相反 (F-0003-01)、426 头注描述的成因与调用点不符 (F-0003-03)
- `web_api/ws/events.rs` → B0003 · 帧投影逐字段核对通过（Dart 侧 text_delta 双可空已证伪风险）；action_cue 复用既有帧型符合红线 Q。error 帧无日志侧等价脱敏（候选 C-5）
- `web_api/ws/connection.rs` → B0003 · 三条退出路径都调 unsubscriber，无泄漏；subscribe_ack 死形参 + 未转义拼接 (F-0003-02)
- `web_api/ws/broadcaster.rs` → B0003 · unsubscribe 计数同步、teardown 及时（已核实无泄漏）；背压被当成死亡硬掐断连接 (F-0003-04)
- `web_api/app_routes.rs` → B0002 · 状态真源实现正确、测试扎实；performance 段 has_api_key 口径与 llm/tts 不一致 (F-0002-03)、last_fallback 缺省谎报 performance_ok (F-0002-04)
- `web_api/file_watcher.rs` → B0002 · 监听/防抖/Drop-join 本身干净；问题在装配侧 —— 首次配置路径永久不装 (F-0002-01)；日志文案中英混排 (F-0002-07)
- `web_api/supervisor_slot.rs` → B0002 · 动态装配逻辑正确且与启动路径同源；两条测试结构上不可能失败（假绿灯） (F-0002-02)
- `web_api/flutter_app.rs` → B0002 · 路径穿越双检 + 候选序列写得完整；静态产物全 no-cache 且无 ETag (F-0002-05)、build 目录每请求重算 (F-0002-06)
- `web_api/security.rs` → B0001 · Origin/CT 门禁逻辑正确；但 check_ws_origin 的「未装配」注释与事实相反且被 allow(dead_code) 掩盖 (F-0001-05)
- `web_api/dispatch.rs` → B0001 · 14 个 RouteId 与 is_mutating_route 逐一对齐；test/* 坏 JSON 静默降级 (F-0001-06)
- `web_api/responses.rs` → B0001 · 统一 404/403/501 JSON 错误体，无发现
- `web_api/model_root.rs` → B0001 · 唯一模型根实现正确，但两条测试结构上不可能失败（假绿灯） (F-0001-02)
- `web_api/log_routes.rs` → B0001 · dev_mode 门控 + 只读当前 log_dir，三态齐全，无发现
- `web_api/wasm_assets.rs` → B0001 · 路径穿越双检写得对；但 wasm_dist_dir 硬编码他树绝对路径 (F-0001-03)、/render 前缀判定 GET 与非 GET 不一致 (F-0001-07)
- `web_api/mod.rs` → B0001 · 路由表与前置路由顺序；四族前置端点绕过请求日志 (F-0001-01)、body 非 UTF-8/截断静默清空 (F-0001-04)、with_security 装配注释不实 (F-0001-05)

## crates/live2d-ai-runtime/**

### BATCH-0015（6 文件 / 1635 行 · P0 0 P1 0 P2 1 P3 1）

### BATCH-0016（5 文件 / 片段读 · P0 0 P1 0 P2 0 P3 0 —— **零发现批**）

### BATCH-0017（5 文件 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0018（7 文件 / 片段读 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0019（3 文件 / 片段读 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0020（2 文件 · P0 0 **P1 1** P2 0 P3 1）

### BATCH-0021（3 文件 / 片段读 + 3 Dart 文件核对 · P0 0 P1 0 **P2 1** P3 0）
- `runtime/src/settings/view.rs` → B0021 · 三档 has_api_key 口径文档化且 with_keys 路径三段统一走 key_is_ready（正确）；api_key_env 变量名永不出门。**但 performance 段零前端消费者** (F-0021-01)。Dart 侧 active_model_id 按顶层取（已核实对齐）
- `shell/flutter/lib/api/settings_models.dart` → B0021 · 931 行；LlmView/TtsView/PersonaView/ActionView 逐字段与 Rust 对齐（含 ActionModelView 三键恒出现不省略的契约）；**无 performance 段**
- `shell/flutter/lib/settings/sections/{llm_section,dev_tools_section}.dart` → B0021 · **F-0021-01 的消费侧证据**：设置面板用硬编码文案「配置只在 toml 段」；诊断面板读 app/status 的 7 个键但不含 performance
- `runtime/src/performance/plan.rs` → B0020 · **红线 P 核心硬约束成立**：`segments.concat() != source` 直接 Err（表演层不得改写正文）；字段级校验 + 逐条降级带 code；锚点有界/强度 TTL 轴全钳位；schema 与校验器同源。after_prev 退化时 at 标签与 sentence_seq 矛盾 (F-0020-02)
- `runtime/src/performance/client.rs` → B0020 · **MAX_TOKENS=1024 与 MAX_SEGMENT_CHARS=4000 矛盾**：plan 必须逐字复现全文，长回复必然截断→InvalidPlan 静默回退 (F-0020-01 P1)。请求体不含密钥、temperature=0
- `runtime/src/secrets.rs` → B0019 · 快照单例 + lookup 单入口 + 无 set_var；lookup 优先级 .env>进程环境>无；is_set/known_keys 只出布尔与键名。**红线 R 核心纪律全仓满足**（15 处 env 读取逐一归类，无一处直读 key）
- `runtime/src/performance/mod.rs` → B0019 · 日志只打 summary 不打全文、warning 带 code=；**但 InvalidPlan 回退路径 fallbacks 计数 +2**（其余四条只 +1） (F-0019-01)
- `runtime/src/audio/wav.rs` → B0019 · RIFF 解析 6 条不变量全过：非 PCM/非 16bit 明确 UnsupportedFormat、声明长度取交集、奇数字节填充对齐、EXTENSIBLE 子格式、截断 PCM 报错不补零
- `runtime/src/tts.rs` → B0018 · 请求体只含 input/model/voice/response_format，无密钥回显；逐块转发不做解码（职责边界清楚）。全读，零发现
- `runtime/src/audio/decoder.rs` → B0018 · 增量 PCM 解码任意字节切割安全、容量精确无 realloc、悬挂字节回 TruncatedPcm 不补零
- `runtime/src/conversation/worker.rs` → B0018 · 每句恰好一个 final_chunk、first_chunk 单一口径 !sent_any、三处收口都发 SentenceVoiced（先声音后文字）。读 176-344 补齐 B0015 未读段
- `runtime/src/audio/rms.rs + queue.rs` → B0018 · **416 行导出型音频代码生产零调用**，且被 crate 根 doctext 演示成标准流水线；实际音量走 ws/audio.rs 的另一份实现 (F-0018-01)
- `runtime/src/lib.rs` → B0018 · crate 根 doctext（:54-65）把 SampleQueue+RmsMeter 呈现为音频链路，与生产相反 —— F-0018-01 的载体
- `runtime/src/conversation/error_code.rs` → B0017 · code/stage/is_fatal/hint 四件套单一来源；hint_for 全变体覆盖无 `_` 兜底（新增变体编译器点名）；code_always_starts_with_stage 回归锁定
- `runtime/src/error.rs` → B0017 · code_suffix 把上游状态码编进码（upstream_401）；**Error::status 把上游响应体片段原样存进 Error** → 这是 F-0017-01 的源头
- `desktop/src/supervisor/handlers.rs` → B0017 · **第 6 条红线的强形式**：`from_kind` 算一次，tracing/println/WS 三处发同一个值；前端按 code 前缀分支不猜文案
- `desktop/src/logging.rs` → B0017 · SanitizingWriter 只挂文件层且 redact 只认「敏感键名+=/:」结构化形态，认不出散文里的密钥 → F-0017-01 的日志侧缺口
- `shell/flutter/lib/{ws_frame,ui/error_actions}.dart` → B0017 · 前端按 code 分支（:513 取码、:33 code.startsWith('llm_')），不猜显示文案 —— 红线后半句 ✔
- `runtime/src/llm.rs` → B0016 · max_tokens 端到端取值链完整（唯一默认 4096 → effective → ResolvedSettings → wire，0 才省略）；SSE 关流三分支顺序正确。**零发现**。读 1-325 + 15 条测试名
- `runtime/src/sse.rs` → B0016 · 任意字节切割安全（BOM/CRLF/UTF-8 多字节都有专测），finish 语义在结构上不丢事件。零 Pattern B。读 1-200 + 14 条测试名
- `runtime/src/config.rs` → B0016 · LlmConfig.max_tokens 字段注明确「已由 effective_max_tokens 解析过默认值」；::new 设 0 但仅测试用（已 grep 证伪生产绕过）
- `runtime/src/settings.rs` → B0016 · DEFAULT_MAX_TOKENS 唯一定义在 :187，守卫测试 :438 断言 >= 2048（改回 512 会红）
- `runtime/src/dialogue/sentence.rs` → B0015 · 状态机设计正确（终止符 run 折叠/小数与缩写保守规则/任意 delta 切割一致）；200 字安全阀的注释与 AGENTS.md 的绝对红线措辞不符 (F-0015-02)
- `runtime/src/dialogue/orchestrator.rs` → B0015 · **思考隔离的执行点**（:59 吞掉 ReasoningDelta，注释写明后果）；ReasoningDelta/Done 枚举穷尽匹配
- `runtime/src/dialogue/clean.rs` → B0015 · 确定性清洗的 7 条规则表与幂等回归都在；清洗后为空走静音句而非 TTS（红线）。读 1-175
- `runtime/src/conversation/worker.rs` → B0015 · **空句不进 TTS 的真实执行点**（:105 两种情形不发 HTTP，只发空 final_chunk + SentenceVoiced）；全文件 tracing=0
- `runtime/src/conversation/engine.rs` → B0015 · 思考在进 assembler 前被拦一道（:292-307）；两条路径的 sentence_seq 均无条件自增（D24）；worker panic 消息被 unwrap_or 丢弃 (F-0015-01)
- `runtime/src/dialogue/mod.rs` → B0015 · 模块边界与可编译示例（doctest 演示 SentenceReady+Done），职责声明与实现一致
## crates/l2d/** + crates/l2d-wasm-demo/**

### BATCH-0045（2 文件 / 片段读 · **0 新发现** · 渲染面首触）

### BATCH-0046（3 文件 · P0 0 **P1 1** P2 0 P3 0）—— 45 批以来第一条安全发现

### BATCH-0047（2 文件 · **0 新缺陷** · 但改写了 F-0046-01）
- `l2d-wasm-demo/src/web/net.rs:20-21` → `fetch_bytes` 是**裸 `window.fetch_with_str(url)`**：无 scheme 检查 / 无 host 白名单 / 无同源判定
- `main.rs:163-181` → `load_model_from_url` **先抓后校验**（:163 抓 manifest → :166 解析 refs → :171-175 再逐个抓）⇒ 一次输入触发 **1+N 次**任意 URL 抓取
- `main.rs:267` + `net.rs:38-42` → ⭐ **`?model=` 查询串是入口 V1，零 JS 门槛**（`iframe src=".../render?model=http://attacker/…"`），**不需要 postMessage、也不需要 origin 缺口**
- ⇒ F-0046-01 影响段改写：V1（查询串）与 V2（postMessage 无 origin 校验）是**两条独立**入口，共同使能条件是全仓无 `X-Frame-Options`/CSP。**定级 P1 维持**（抓取是盲的），但**可达面比原文宽**
- 反证补记：「解析失败会挡住」不成立 —— 抓取在解析之前，失败只意味着攻击者没拿到模型，请求已发出
- `l2d-wasm-demo/src/main.rs:380-386` → **接收侧 postMessage 零 origin 校验**（`ev.data()` 直接吃，不碰 `origin`/`source`）
- `l2d-wasm-demo/src/main.rs:706` → **发送侧 ACK 用 `"*"`** 通配 origin
- `flutter/lib/live2d/live2d_host_web.dart:48 / :78` → **协议另一侧做对了**：接收 `if (event.origin != location.origin) return;`、发送钉 `location.origin`；且 :41-43 把该契约写成文档 ⇒ **缺的是属于 wasm 的那一半**
- 全仓 `X-Frame-Options` / `frame-ancestors` / CSP **零命中** ⇒ 任何页面可 iframe `/render`；处理函数可 `load_model_from_url` 抓任意 URL（**先抓后校验**）⇒ 盲抓 + 舞台劫持
- ⇒ **F-0046-01（P1）**：不判 P0 的理由——抓取是盲的（服务端不回 CORS），拿不到响应体，也没把密钥交给跨源方
- `l2d-wasm-demo/src/main.rs:491-501` → `audio-volume` 分支**只写 `st.bridge.volume` 一个字段**：零渲染、零分配、零纹理
- `l2d-wasm-demo/src/web/surface/input.rs:151-196` → `apply_bridge_effects` 由 `tick` 每帧调，`set_parameter("ParamMouthOpenY")` 参数写（非纹理重建）；**衰减与帧率无关**（60/240fps 推导写在 :164-174）
- ⇒ **红线 O 双端成立**：Dart 侧 30ms 轮询 + 33ms 节流 + GlobalKey→postMessage（B0023）＋ wasm 侧「消息入状态 / 帧循环出参数」
- 另：`:176` `lip_sync.then(...)` ⇒ 关口型真的写 0；与「静音时 PCM 置零但 volume 真实」（B0006）叠加 ⇒ **静音 / 音量 / 口型三者正交**
- `:187-192` 记录了一次**实测驱动的修正**（旧实现门限过滤使真实语音 RMS 中位数 ~0.03 下嘴几乎不动 → 改 dB 映射）
- `:391-405` v1 信封 + 扁平回退 + 逐字段钳位**不整条拒收** ⇒ 合红线 Q「只增不改」
## crates/live2d-ai-core/**

### BATCH-0039（3 文件 · **0 新发现** · Rust 未审区首批）

### BATCH-0040（4 文件 · P0 0 P1 0 P2 0 **P3 1**）

### BATCH-0041（1 文件定点深读 · **0 新发现** · 模式 B/H 第 4 次复查）

### BATCH-0042（1 文件 / 片段读 · P0 0 P1 0 P2 0 **P3 1**）

### BATCH-0043（1 文件 / 片段读 · **0 新发现** · live2d-ai-core 收尾）
- `core/src/performance/mod.rs:15-16` → 休眠层**永不驱动嘴开合**（「`MouthOpenY` 归 TTS RMS 通道」）⇒ 与红线 O 不冲突，唤醒也不会抢通道
- `core/src/performance/mod.rs:95-101` → `ParameterMask` 语义是「**释放所有权**」而非「写零压住 idle/物理层」（更正确的模型）
- `:89-91 envelope` 的两处除法：6 个调用点全用 (0,1) 内字面量，且函数**私有** ⇒ 不变量由**结构**保护（假设证伪）
- **候选模式 J**：休眠台账只覆盖「谁不调用」，不覆盖「留下的代码自身是否仍正确」；B0042/F-0042-01 与本批三条都是台账视角看不到的
- `core/src/action/mod.rs:112-118` → `ActionSource::LlmTool`（优先级 90）是**活的值**（host 把 Mod 动作请求映射进这一档，`mod_registry.rs:288`），但名字与字段注仍写「主 LLM 的 tool action 输出」——**该产出路径 rc.2 已整体拆除** ⇒ F-0042-01
- ⇒ 三重后果：误以为工具层还在 / 若恢复工具层会误以为只剩接线 / 但**删不得**（Mod API 契约带这个 source）⇒ 正确修法是补注或下个大版本改名
- **与 F-0040-01 同根因**（台账与代码演进不同步）的第三处症状
- 核验：`apply_action` 的优先级仲裁与模块头注逐字相符（低优先级显式 Dropped、同级后来者胜/高级抢占 → 单一 `Transition`）
- `core/src/core_tests/mod.rs:196-275` → **本审计读到的最硬测试**：制造 turn#7 ID 跨代次复用、让 id 判定**完全命中**只有 epoch 闸门能拒、断言**丢弃不留状态痕迹**、再断言闸门**未过度拦截**新代次、最后把双闩锁最难那句（Completed+Playing ⇒ turn 存活）直接断言
- ⇒ 删闸门 / 挪闸门位置 / 丢弃留副作用 / 全拦，**四类改法每一种都会让它变红**
- **模式 B/H 累计：连续 4 次「难形态」复查零命中**（含本条）；4 处历史命中集中在 2 个文件 + 2 种形态 ⇒ 校准：假绿灯非系统性，只盯新写测试与未审目录
- `core/src/reducer.rs:115-272` → 不变量 3/4/5 逐行核实通过；**「恰一次」靠 `active_turn = None` 实现而非布尔标志** ⇒ 单一真源、无第二份状态（值得记的范式）
- `core/src/core_tests/` → 20 个测试名与四条不变量**一一对应**（含 `completed_generation_before_drain_keeps_turn_alive`、`stop_clears_everything_and_advances_epoch_once`）；断言体未逐行读
- `desktop/src/main.rs` → 工厂数护栏真名是 **`mod_count_is_five`**（恰好 5），`mod_count_is_three`/`_four` **全仓零命中**；而 AGENTS.md 休眠台账仍点名 `mod_count_is_three`，且说 director「已删除」而它**在册**（Wave 3 零投递骨架）→ F-0040-01
- **注册表变更日志写法可借鉴**：main.rs:60-82 逐次记录每次数字变化及其理由（4→3→5→6→7→5），是本仓最完整的一份变更溯源
- `core/src/lib.rs` → **红线 R 休眠台账落地**：六条不变量 + 「谁休眠/为什么/谁能唤醒/为何保留不删（不变量 4、6 以之为前提）」逐项可答；:53-54 有「删动作时绝不要连带删 IdleState」的防误删警告
- `core/src/reducer.rs:24-34` → epoch 闸门**在任何规则之前**，返回 `Dropped(StaleEpoch)` 并记两个代次
- `core/src/events.rs:91-102` → 除 `StopRequested` 外**每个**变体都带 epoch，**含 `Event::Action` / `ActionPlaybackFinished`** ⇒ 休眠子系统的事件也在闸内，lib.rs 的宣称**为真**
- **休眠由三层独立挡住**（通道已删 / Mod API 返 false / 闸门兜底），且**第三层不依赖休眠本身** —— 本审计见到的对休眠资产最稳的处置
## crates/live2d-ai-mod-system/** + Mod crates
## crates/live2d-ai-mod-*/ （8 crate / 61 文件 / 24,665 行 —— 换根后的新区域）

### BATCH-0060（4 目标 · P0 0 P1 0 **P2 1** P3 0）—— ⭐ 换根后的第一个发现
### BATCH-0061（7 目标 / 定点读 · **0 新发现** · Mod 根第 2 批）
### BATCH-0062（6 目标 / 定点读 · P0 0 P1 0 **P2 1** P3 0）
### BATCH-0063（1 文件 + 5 Mod grep · **0 新发现** · F-0062-01 扩散核）
### BATCH-0064（1 文件 / 定点读 · **0 新发现** · 会话分桶实现面）
### BATCH-0065（1 文件全读 · **0 新发现** · Mod 根收尾）
### BATCH-0066（3 目标 / 定点读 · **0 新缺陷** · ⭐ 自我更正 F-0062-01）
### BATCH-0067（2 文件 / 定点读 · P0 0 P1 0 P2 0 **P3 1**）
### BATCH-0068（1 文件 / 定点读 · P0 0 P1 0 P2 0 **P3 1** · 结 B0067 未核实点）
### BATCH-0069（3 文件 · **0 新发现** · ⚠ **撤回 F-0030-01**）
### BATCH-0070（3 目标 · **0 新发现** · F-0034-01 证伪）
### BATCH-0071（1 文件 / 定点读 · **0 新发现** · 结 B0070 残留）
### BATCH-0072（1 文件 / 定点读 · **0 新发现** · ⚠⚠ 撤回 B0070 的证伪）
### BATCH-0073（1 文件 / 定点读 · **0 新发现** · 核自己写下的前提）
### BATCH-0074（2 文件 / 定点读 · P0 0 P1 0 **P2 1** P3 0）
### BATCH-0075（1 文件 · **0 新发现** · 三处清单机械对账）
### BATCH-0076（1 文件 · **0 新发现** · 第 4 处清单对账）
### BATCH-0077（1 文件 / 定点读 · **0 新发现** · 离开 display_prefs 区）
### BATCH-0078（2 文件 / 定点读 · **0 新发现** · 唯一漏斗核实）
### BATCH-0079（4 文件 / 定点读 · **0 新发现** · 落盘侧三项核验）
### BATCH-0080（2 文件 / 定点读 · **0 新发现** · 职责澄清 + 红线 6 前端侧）
### BATCH-0081（2 文件 · **0 新发现** · 模式 H 第 6 次复查零命中）
### BATCH-0082（3 文件 · **0 新发现** · 三处拦截逐段核实）
### BATCH-0083（1 文件 / 3 处定点 · **0 新发现** · 关闭 B0082 边界）
### BATCH-0084（1 文件 / 读 140-255 · **0 新发现** · 最细的错误处理推理之一）
### BATCH-0085（1 文件 / 读 140-195 · **0 新发现** · 接缝关闭）
### BATCH-0086（3 文件全读 · **0 新发现** · 本区接缝完全关闭）
### BATCH-0087（1 文件 · 0 新缺陷 · ⭐ **F-0034-01 升级 P3→P2、潜在→在线**）
### BATCH-0088（2 文件 · **0 新发现** · ⭐ 拦下高危假发现）
### BATCH-0089（1 文件 / 定点读 · **0 新发现** · 第二处同形排除）
- `appearance_section.dart:211-227` → `_BackgroundBlock` 的 **6 个可空回调 + 4 个非空回调在生产构造点逐个接上**（`onAddImage`/`onClearLibrary`/`onPickStageImage`/`onClearStageImage`/`onRemoveItem`/`onAddPattern`/`onReorderItem`/`onRemoveMany`/`onPreviewItem`）
- ⇒ **可空签名是给「测试 / 复用」留的**（与 `appearance_background.dart:12` 的 `part` 理由一致），**不是**功能静默不可达的迹象 ⇒ **B0088 留的「第二处同形」排除**
- ⇒ 与 B0087 的 F-0034-01 **明确对照**：那一条回调**也接上了**、缺陷在**下游判等**；本批这条**也接上了**、下游也正常 ⇒ **「回调未接上」这一族在本区只有一个实例**
- 顺带正面：`:213-215` 两条消息合并（「两个来源轮流出现会让用户以为刚才那条是上一张图的」）· `:231-239` 两套轮播按来源互斥并引用 **rc.5 §9.2** 已记录事故 + 专门改名分清
- ⚠ **差一步就记「1551 行背景 UI 从未被构造」**：① `grep "_BackgroundBlock("` 只命中自己的声明(:276) ② 类名带 `_` ⇒ 我判「文件外无法构造」⇒ 两者相加判「整块背景 UI 是死代码」（若成立是本审计最重的一条）
- **真相**：`appearance_background.dart:44` 是 **`part of 'appearance_section.dart';`** ⇒ **共享 library 的私有命名空间**，`appearance_section.dart:211` 正在构造它 ⇒ **我犯的错是「把文件私有当成库私有」**（规则 3 第 4 次同族、**最险的一次**）
- ⇒ **规则 3 变体 4**：断言「某符号从未被使用」前**必须先确认文件是 `part` 还是独立库**；同一文件 grep 不到构造点**不构成**死代码证据
- ⭐ 正面：`appearance_background.dart:22-44`「# 行数（**如实标注：超 1000，是既有债**）」——给出规模来源（1489 行）、为何不现在拆（「只搬不改……搬运+重构同时做是经典事故面」）、**Stage C3 拆分表**（四类 + 实测行数 + 建议去处）；`ls` 实测两个目标文件**都不存在** ⇒ **没把「计划」写成「已完成」** ✔
- 正面：`:927-929` 选 `ReorderableListView` 的理由**点名换来的是「键盘可达」**（「自己用 LongPressDraggable 拼一个既不完整又没有无障碍」）
- 六跳接线全核到行：`onItemChanged`(`appearance_background.dart:393-398`，**生产代码**) → `onStyleChanged` 派生(:957-961) → 按钮条件(:1263/:1301) → `onChanged` → `shell_prefs.dart:146` 总闸 → `display_prefs.dart:970` **id-only `sameAs`**
- ⇒ **只改逐图样式 ⇒ id 不变 ⇒ `==` 判等 ⇒ 总闸早退 ⇒ 什么都不发生**；且 `DisplayPrefs` **无独立样式存储**（三候选名零命中）而样式**住在 `BackgroundImage` 内** ⇒ 没有第二个字段能判出差异
- ⇒ **与 F-0036-01（口型/缩放静默不刷新）同一族**；修法仍是一行（`:970` 改用 `!=`），测试与 F-0074-1 同处
- ⭐ **方法论第三次同族（最贵）**：我打「潜在」时核的是**按钮的渲染条件**、没核**回调值在生产是否非空** ⇒ 规则 3 补：**「条件依赖某值」必须追到赋值点**；**低估比高估更危险**（让在线缺陷以「潜在」名义长期留在账本）
- `background_store.dart:84-102` → 契约**默认值即安全侧**：`readChecked` 默认有损但标注「应当覆写」（:82-83）· `probe()` **默认 `false`**（:100-101「无法证明健康 → 不剪枝」）
- `background_store_web.dart:180/197-200` → **两个方法都覆写**；`probe` 用**专用哨兵键** `'__health_probe__'`（:45）做真实可用性探测 ⇒ **剪枝在 web 上确实启用**
- `background_store_stub.dart:24/33/50` → 也**都覆写**（`probe => true`）
- ⭐ **已排除的高隐蔽性假设**：「web 实现可能忘覆写 `probe` ⇒ 剪枝永久静默关闭 ⇒ 字节库无界增长」——**证伪**（确有覆写）；该假设的症状会极其隐蔽（无报错无日志），故**记为「已排除」而非「没查」**
- 备忘：`read`（`String?`，分不清两种 `null`）**被保留**、另加 `readChecked` 三态 ⇒ **不静默改语义、另给精确变体**
- `background_hydration.dart:179` → **`keep.add(item.id)` 在任何 I/O 之前** ⇒ 读失败的项**不可能**被 `retainOnly` 删掉字节 ⇒ 保护**结构性**，**不依赖** B0084 核到的那道闸（双保险）
- `:145-149` → `liveKeptIds` 解的是**快照 vs 实时 UI 的竞态**（水合窗口里刚导入的图会被当孤儿删掉，**F-0013-1 的另一半**）；解法是「传一个读**此刻**偏好的函数」，且 `null` 时**退化成旧语义**
- `:155-157` → `healthy` 先探，且**区分「空清单」与「库空」**（「空清单可能只是『这一次读不到』」）
- `:181-192` → 内联字节搬不进库时**不动该项**，「绝不改写成 id-only：那等于把字节从两处一起抹掉」（**P0-2**）⇒ 该区至少经 **P0-1 / P0-2 / F-0013-1 两半** 四次真实事故，推理全部保留在原地
- `background_hydration.dart:200-211` → **`missing` 与 `failed` 是两件事**：`missing`（存储明确答「没有」）⇒ **剪枝**；`failed`（读失败 ≠ 没有，P0-1）⇒ **原样保留、等下次启动**
- `:214-218` → 破坏性剪枝**双重门控** `if (healthy && !storeFailed)` ⇒ 存储抖动不可能造成批量删除
- `:230-233` → 写入侧 bool 契约「**绝不能先更新偏好再假装成功**——那会得到『界面上有、刷新后消失』」⇒ 与读侧构成**读写两端**
- `:244-255` → `_safe` 吞异常**与返回 `Future<T?>` 成对** ⇒ 「吞掉并上报」而非「吞掉并说谎」
- ⭐ **正面模式第 3 样本（已成风格）**：每个非平凡决策都配一句「**不这么做用户会看到什么**」—— 与模式 D 恰为**镜像**（D 是注释与事实脱节；这条是注释**主动**指向可核的用户可见症状）
- 三处「选留下 ⇒ 中止」**全部正确**：`closeSettings:450-452`（`!leave || !mounted ⇒ return`，await 后**再查一次** mounted）· `_closeSheetGuarded:549-552`（**`sheetContext.mounted`**，因为 `Navigator.of` 需那个 context）· `_selectGuarded:671-673`（`if (await ask()) _select(next)`，且**同分区/无注入时走快路径不问**）
- 三处结构**不同**（setState / Navigator.pop / 纯通知）但**语义一致** ⇒ B0082 的诚实边界**关闭**
- ⭐ 顺带正面（:660-665）：「设置页的说明还写着『聊天面板会跟着透』，实际面板已经退回不透明——**界面在骗人**。那份条件连同它造成的断层一起删掉了」⇒ 不只修行为，还**删掉会骗人的说明文字**
- 完整链路四段全核到行：`confirmLeave`(settings_controller:335) ← `_confirmDiscard`(shell_settings:182-198) ← `main.dart:1301` 注入 → `AppShell.confirmDiscard`(app_shell:138/314) → **三处使用 :449 / :545 / :664**
- ⇒ `shell_settings.dart:177` 点名的三处（换分区/关设置/关浮层）**与代码里的三处使用对得上**
- 三处细节：① 字段**可空**且降级有记录（:445「为空时保持原来的同步行为」）② `shell_settings:184-186` **防重入**用布尔 + `try/finally` ③ ⭐ `app_shell:443`（**P0 / 2026-09-20**）记载「脏草稿要走同一套 `confirmDiscard`」⇒ **这「三处」曾经就是不三处**，是修过的真实缺陷
- ⇒ 模式 H 变体「入口存在 ≠ 三处都接上」**不成立**；**未核**每处「选留下」是否正确中止导航（已记 STATE）
- `settings_controller.dart:332-340` → `confirmLeave` 是「三处拦截」的**唯一入口**（三处调用共用一个实现，与 `_applyPrefs` 同一形状）；`:357-361 edit` 写完立刻 prune，理由写明「否则『点开又改回去』会**永久显示「未保存』**」；`:363-371 pruneAgainstRemote` **公开**且理由是「让测试能直接构造草稿后调用」⇒ **可测性是设计输入**
- `settings_controller_test.dart` 36 条用例 → 头注三条 + 实现四条**逐条有对应用例**；含 `只改一个字段，补丁里**只有**那个字段（不误伤同段其它字段）` 与 `dirty 与键序无关`
- ⭐ 两条**诚实性**用例值得单记：`persisted:false → noChange（不谎报「已保存」）` · `未识别的 apply_status → 保守按需重启` ⇒ 守的是**「不说谎」**而非功能正确，与 AGENTS.md「自检说谎比没有自检更坏」同源
- ⇒ **模式 H 累计 6 次复查零命中** ⇒「难形态未被钉住」**不是本仓的普遍形态**
- **职责澄清**（解答 B0079 疑问）：`settings_controller.dart` 是**服务端 settings 的草稿控制器**（PATCH 链），**不是**本地 `DisplayPrefs` 落盘链 ⇒ B0079 零命中**不是缺口**
- `api/settings_models.dart:35/49/56/63` → `Tri` 三态用 **`sealed`** ⇒ 编译器穷尽；`:72-78`「`null` 与 `TriKeep` 都表示『不修改』→ **不写键**」⇒ 与 Rust 侧 `Option<Option<T>>`（B0010）**同一契约两端** ✔
- ⭐ **红线 6 前端侧成立**（`settings_controller.dart:207-212`）：`messageWith` 在失败时把后端带来的 `code`+`message` **原样上屏**，不塌缩成「保存失败」；注释用**用户原话**「后端出错无具体错误代码」作缺陷证据
- `SaveOutcome` **5 结局**（applied/restartRequired/queued/noSupervisor/failed）+ `isSuccess` 只认 4 绿 ⇒ 诚实区分「需重启/排队中/下次启动生效」，**不把非即时的成功说成即时**
- `browser_io.dart:53-63` → `saveDisplayPrefs` 的 bool 契约**被兑现**：`catch (_)` 兜住全部异常（无痕/配额/存储被禁）⇒ 只在真成功时返 `true`
- `kDisplayPrefsKey` 全仓**只有一个写入点**（`:56`）⇒ 落盘路径**单一无旁路**，这是「落盘失败可被观测」的前提，**成立**
- 两个同名 `onPrefsChanged` 是**合法的两层设计**：`main.dart:289` `bool Function(...)`（外层，返回落盘结果）· `appearance_section.dart:88` `ValueChanged<...>`（内层 void，9 个调用点不需要知道结果，因失败横幅由外壳层展示）⇒ 同名不同签名是潜在误读点，**但类型系统会拦** ⇒ 记备忘不记发现
- `_applyPrefs` 调用点**恰好三处**：定义 `shell_prefs.dart:76` · `_updatePrefs`→`:158` · **`main.dart:1207` `onReady: _applyPrefs`** ⇒ 「唯一漏斗」**为真**
- 声明**自己划了作用域**（`main.dart:496`「那四个字段**只有** `_applyPrefs` 会发」）—— 舞台组件**确实也直写桥**（`live2d_stage.dart` 9 处）⇒ 唯一性成立于**那四个字段**，且代码**明确列出是哪四个** ⇒ 精确边界，非虚假全局唯一
- 两个测试记这条链，`stage_color_motion_test.dart:6` 是**真实缺陷回归**（「过去 `_applyPrefs` 一按就下发终色」会杀掉过渡）⇒ 声明/实现/回归三处对齐
- ⚠ **第 5 次同类失误（新变体）**：`grep "_applyPrefs("` 漏掉 tear-off `onReady: _applyPrefs` ⇒ 规则 3 再补：**查调用必须覆盖「调用/tear-off/回调字段」三形态**，只匹配 `name(` 会**系统性**漏掉接线
- `display_prefs.dart:644-659` → `_readBackgroundSource`：未知枚举值**回落不生效**；迁移走「**读旧键**」而非「改默认值」，理由是**改默认值会让「显式设过」与「从没设过」不可区分**（根因级设计约束）；旧键读取带长度上限
- `:661-675` → `effectiveBackground`「判据只有这一处」，并**点名它防的类**：「判据散到两处就会出现『设置说用背景库、画的不是背景库』这类不一致」；`:668-669` 记 **每帧路径用固定 id 而非内容哈希**「免得每帧重算一张几 MB 的哈希」⇒ 与 B0071 的 djb2 指纹构成互补一对
- ⇒ **换根（停止规则）**：本区连续 5 批零发现 + 2 次自我更正 ⇒ 判为「已验透」；教训固化为模式 L 执行规则 3
- 四路机械差集（每路带非空断言）⇒ **`DisplayPrefs` 四处清单全部一致（24 字段）**：`==`(22 标量+2 列表) / `toJson`(24) / `fromJson`(24) / `copyWith`(25 含合成标志)
- ⚠ **第 4 次「没看到 ≠ 没有」**：差集先报「`fromJson` 少读 `backgroundSource`/`backgrounds`」⇒ 查解析发现二者**经辅助函数**读取（`readBackgrounds(json)` / `_readBackgroundSource(json)`，:578-579）⇒ **解析漏了，不是代码漏了**；直接收下就会记下一条看似 P1 的**假发现**（「背景图重载后丢失」）
- 顺带正面：`fromJson` **读时钳位**（`clampBackgroundOpacity` / `clampBackgroundBlur` / `_clampInt`，:580-587）⇒ 落盘值在读取时消毒；与 F-0067-01「写入侧无钳」是**不同面**
- 机械集合差（各自读到结尾）：`==`(22 标量) ⊆ `copyWith`(25 形参) ⇒ **差集 ∅**；多出的 `backgrounds`/`stagePlaylist` 确实在 `==` 里（`sameAs` 循环 + `_sameList`）
- ⇒ **`==` / `copyWith` / `hashCode` 三处手维护清单当前完全一致**（F-0074-01 的「靠人守」风险**未实现**，故不记发现）
- ⚠ 过程中犯「**解析失败当成结论**」：第一次正则用旧 Dart 风格 ⇒ 解析出 0 个 copyWith 参数、差集显示「缺 22 个」——未警觉就会留下一条**假发现** ⇒ **规则 3 补充**：「我的工具解析出空/异常」也按「没看到 ≠ 没有」处理
- 风险陈述（非缺陷）+ F-0074-01：清单今天是对的，**没有任何测试会在它们开始漂的那天告诉你** ⇒ 建议**参数化**测试（枚举字段名）而非手写 22 条断言
- ⭐ **F-0074-01（P2）**：`DisplayPrefs.==` 是**全应用的变更检测总闸**（`shell_prefs.dart:146` `if (next == widget.prefs) return;`，所有偏好变更都过它）⇒ **零测试覆盖**；它委托的 `sameAs` **也零测试**；唯一相邻的 `background_store_test.dart:58` 两侧都用**默认样式** ⇒ `==` 丢掉三样式字段时它照样绿 ⇒ **对本轴结构上无法失败**
- 两处自我更正：① B0070 备忘「`hashCode` 不含 `backgrounds`」**错**（`:1003` 有 `...backgrounds`，当时只读到 :994）② 「`==` 与 `hashCode` 两份手写清单已漂」**不成立**（两者覆盖同样 24 字段）
- ⭐ **模式 L 执行规则 3**：「**没看到 ≠ 没有**」—— 对字段清单/分支/函数的**完整性**断言必须读到结尾标记；三处同类错误全在同一区域，与「必须核发送前还有没有一层」同族
- `background_item.dart:377-385` → `BackgroundImage.==` = id + **opacity + fit + align**，`hashCode` 覆盖**同样五字段** ⇒ **教科书正确的值类型**；`BackgroundPattern.==`（:430）只比 id 且 hashCode 对称 ⇒ 同样正确。**`design/` 层无错**
- ⇒ F-0034-01 升级为**完全指定的接缝**：**正确的判据已写在同一文件里**，而 `display_prefs.dart:970` 调了更弱的 `sameAs` ⇒ 修法 = **一行替换**，无新增代码/API 变更
- 「潜在」定级不变（`appearance_background.dart:1264` 样式控件无回调时不渲染），但**两个各自很小的改动叠加**就得到在线的静默丢弃 ⇒ 模式 C 典型形状
- ⭐ **B0072 教训当场兑现**：建议里引用的 `==` 本批**先读后用**，未再犯 B0070 的错
- `background_hydration.dart:134-136` → 「**必须走 copyWith**：直接 `new BackgroundImage(id: …)` 会把用户给这张图设的 **opacity / fit / align** 一起清空」⇒ **逐图样式住在 `BackgroundImage` 内部**，不是独立字段
- ⇒ **B0070 的证伪前提是我推的、不是读到的，且它是承重墙** ⇒ 撤回该证伪；**F-0034-01 恢复并升级**
- 重写后的机制：`sameAs` 只比 id（对**去重正确**）→ 但 `DisplayPrefs.==`（`display_prefs.dart:968-971`）**复用它做变更检测** → 对象含逐项可变子状态 ⇒ 纯样式变更被 `==` 判成「没变」⇒ `shell_prefs.dart:146` 整条丢弃
- ⇒ 准确说法：**不是「id-only 判等错了」，而是「为去重设计的身份比较被复用为变更检测判据」**（接缝，模式 C）。仍属潜在（`appearance_background.dart:1264` 样式按钮无回调时不渲染）
- ⭐ **本批最重要的教训**：做证伪时**必须逐句核对自己引用的每个前提** —— B0070 的错不是结论错，而是**用了未读的前提**；无人复核就会变成一条**假证伪**留在账本里
- `background_item.dart:478-484` → 指纹是 **djb2 mod 2^31-1 的完整 31 位**（:483 `return h`），id 用 8 位十六进制 ⇒ **无截断**
- ⇒ 「不同图因碰撞判等」不成立（n=100 概率 ≈1.2e-6，且**后果良性**：被当重复项提示，不丢数据/不损坏/不影响渲染）
- 三处微决策**都写了理由且正确**：模数取奇素数（33 与之互质 ⇒ 哈希不退化）· id 带 `bg` 前缀 · 整个 dataURL 含 MIME 头参与（明确承认是取舍）
- 与 B0070 的「`hashCode` 与 `==` 非对称」**不矛盾**：djb2 是非密码学哈希，用途是本地身份标识，不承担安全职责
- `background_item.dart:494-495` → `backgroundIdOf(dataUrl) = 'bg' + hex(背景指纹(dataUrl))` ⇒ **id 是内容指纹**，同 id 必然同内容 ⇒ **id-only 判等是正确的**
- `:40` 注释说明为何不用 `Object.hash`：「**按平台而变**」⇒ 同图算出不同 id ⇒「图不见了」——这解释了自定义指纹的必要性
- 三处担心逐一不成立：① 同 id 不同内容不可能；② 逐图样式在**独立字段**（B0034 已核 `appearance_background.dart:1264`）改样式走另一个字段；③ `sameAs` 唯一调用点 `shell_prefs.dart:475` 正是**去重**，与文档注一致
- 备忘（不构成发现）：两处 `hashCode` 与 `==` 非对称配对（`:385` 含 opacity/fit/align 而 `sameAs` 只比 id；`display_prefs` 的 `hashCode` 完全不含 `backgrounds`）—— Dart 允许且 `DisplayPrefs` 不作 map key
- ⚠ **撤回 F-0030-01（P2）** —— 本审计最重的一次自我证伪。三条子主张：「滑杆每像素一次」**假**（`action_scales_sync.dart:53/136` 有 **150ms 防抖**，连续拖动 ≤7 帧/秒，回归 `preview_test.dart:142` 钉住）·「第三条通道无去重是遗漏」**不成立**（`:186` `test('force 绕过去重：iframe 重建后必须重发')` 把该行为命名并钉住，`:162` 另证非 force 路径会去重）·「文档用途与实际用途不符」**HEAD 已不成立**（`shell_prefs.dart:129-131` 与 `action_scales_sync.dart:33-35` 两处都写明「改主题/调音量等任意变更都会走」并给理由）
- ⇒ **模式 I 样本 2 → 1**（只剩 F-0028-01 解码缓存容量）
- ⭐ **模式 L 执行补充规则**：追链路必须核「**发送前还有没有一层**」；任何「每 X 一次」的性能主张都须先确认 X 与发送之间无防抖/节流/合帧。**知识资产会随时间失效，必须每轮重核，不能靠「我上轮核过」**（B0023 记得 33ms 二次节流 ⇒ 没写错；本批忘了 150ms 防抖 ⇒ 写错了）
- `field_row.dart:432-433` → `NumberField` 越界即 `return`（在 `widget.onChanged` 之前）⇒ **「不回调」为真**，B0067 判读成立、注释准确
- ⭐ **F-0068-01（P3）**：越界输入被**静默拒绝** —— `TextField` 照常显示用户输入，`_values` 仍是旧值 ⇒ **框里显示的不一定是将保存的值**，且零反馈（helperText 的区间常驻，不构成「被拒了」的信号）
- 与 F-0067-01 叠加成可达链：宿主零校验 ⇒ 直接 POST 可把越界值写上盘 ⇒ 面板回显越界值且**不可见其为无效** ⇒ 再点「保存」把越界值**原样再存一遍**
- 修法：越界给明确反馈（错误态 / 回弹到最后一个合法值），或钳位后回调 + 显示钳位提示
- `dev_tools_section.dart:1011-1019` → `min`/`max` **前端强制**：`NumberField` 对越界输入**不回调** ⇒ `_values` 进不了越界值 ⇒ 不发。我原先怀疑「未钳位」**证伪**
- 但**宿主侧零强制**：`clamp` 在 `mod-system/src/settings.rs` / `mod-external-input` 零命中；`validate`（:74-80）只查 key 重复；`reload_config` 整份替换不校验 ⇒ 不变式只在「所有写入都经该 UI」时成立
- ⇒ **F-0067-01（P3）**：定 P3 因**无契约声明说宿主会校验**（与 F-0060-01「声明拒绝而实际忽略」不同类）；但它是第三条「校验只在一条边上」的边界样本
- 独立复核：团队 **F-0012-1（P1）放大器 HEAD 仍成立** —— `field_row.dart:617` `error: resultIsError ? result : null` + `result` 全文件只此一处 ⇒ 误判成功 ⇒ **界面什么都不显示**，与「通过」「没测过」不可区分
- `dev_tools_section.dart:697-716` → 前端**做的是合并**（从 `widget.mod.config` 出发，:698-700 注释**完全正确**，确实保护 spec 外既有键）—— 我 B0062 把它说成「只发自己那几个字段」是**错的**
- `mods_routes.rs:483-499` → `redacted_config` 对 secret 字段用 **`obj.remove(key)`**（:494，**整键删除**）⇒ ① 把值取走，② 合并基线已空，③ `reload_config` 整份替换 ⇒ 盘上也没了
- ⇒ **结论不变且更硬**：② 在结构上**不可能**保留它（手里没值）⇒ **修法只能是宿主侧**，并**否掉「前端修法」**
- ⇒ **模式 C 升级**：三段各自局部正确，而「把值带过第二段」这个职责**从未被指派给任何人** ⇒ 不用改任何一端的意图，**只需给接缝一个责任人**
- 正面：`_selectValue`（:690-693）校验值必须属于 `options`；`_intValue` 坏值回落 0 不崩
- `mod-system/src/settings.rs:20-33` → `secret: bool` 是「这个键是密钥类」的**单一数据源** ⇒ **F-0062-01 建议① 的修法有现成地基**（`reload_config` 已在同一结构体上，`self.settings_spec(id)` 即 secret 键清单，改动约 5 行）
- `:73-80 validate` → 拒绝**重复字段 key** ⇒ 堵死「同一键两种身份」（让某键在 GET 明文、在面板当普通框）——**第三处「结构上堵死一类攻击面」的设计**（前两处：`compose_slots` 类型级隔离 / `sanitize_session_id` 字符白名单）
- `:3` → 「禁止 Mod 注入 HTML/JS/DOM」写在**数据契约文件头部**而非散落在 UI 实现里
- **Mod 根小结**：6 批 → **2 条 P2**（都落在同一条 Mod↔host 信任边界：写通道失败开放 / secret 被抹）；**读通道、会话隔离、路径归一、spec 硬ening 四处核验通过**
- `session_scope.rs:111-117` → `compose_slots(&Slots)` **只接受某会话的槽引用**、拿不到 `Inner`/id ⇒ 隔离是**类型级**的（结构上够不着别的会话），不是运行时检查
- `:162-166 / :246-250` → `prompt_for` / `SessionPromptSink::get` 用 `get(&id)?` **无跨会话回落**
- `:163/:205/:211/:247/:254` 五处各自调 `sanitize_session_id`；:202-203「与 `prompt_for` **逐字同构**」；:168-172 显式禁止「另起一套归一化或容量判定——**那正是『第二套会话表』的开端**」
- ⭐ **对照 F-0062-01**：同批代码两种姿态 —— 会话表按「多写者会出事」完整设计，config 往返按「只有一条路」设计而漏 ⇒ **模式 L 补充判据**：审多写者系统还要问「同一个值会不会经两条路往返」
- 5 个在册 Mod 的 `secret:true` 字段核：**`external-input`(token, lib.rs:93) 与 `voice-input`(token, lib.rs:192)** 两个有；`persona`/`memory`/`director` 无
- `voice-input/src/lib.rs:190/237` → 它的**字段标签**（「访问令牌（空 = 不鉴权）」）与**函数注释**（「空值语义是『不鉴权』」）**都自己写明了后果**
- ⇒ **F-0062-01 范围确定**：宿主侧从未实现 secret 保留 ⇒ **每个带 secret 字段的 Mod 都会丢**，当前 **2/5**；而那个「空」正是本产品 UI 保存流程会造出来的
- 修法不变（B0062 建议①），但**验收要求**新增：回归要**覆盖全部在册 Mod**（逐个 Mod 各自测会漏掉「新增 Mod 忘了遵守」）
- `mod_registry.rs:463-468` → `reload_config` = `e.config = config` **整份替换**；路由层 :258 直接透传 ⇒ **全链路无任何合并**
- `dev_tools_section.dart:705-708` → 前端**有** secret 守卫（空则省略），注释还写对了后果（「发空串会把已存的密钥清掉」）—— 但它防的是「发空串」，而宿主是**整份替换** ⇒ **「不发」同样等于删除**
- `mods_api.dart:20` + B0006 的 `redacted_config` → 前端**手里没有 token 值**，回传也回传不出 ⇒ 抹除在 UI 路径上不可避免
- `shell_admin.dart:235` → 界面回 `'$id 配置已保存'`（**成功**文案），零信号
- ⭐ **升级已登记的 S3**：后果不止「抹掉 secret」——token 若只存于 config，抹掉后 B0005 已核的 `check_token` 对任何请求返 `true` ⇒ **注入端点变成无鉴权**
- ⇒ **接缝缺陷**（模式 C 第 2 例），两侧各自看起来都对；`external-input/src/lib.rs:156-161` 的「只看存在性、绝不回显明文」这半边是**对的**
- `cli_entry.rs:273-287` → `ModSettingsReader` 生产实现返回 `settings_to_view_with_keys`（B0010 已核的 `SettingsView`）⇒ `LlmView` 5 字段**无 `api_key_env`**、`has_api_key` 是布尔 ⇒ **契约为真，密钥与变量名都不出门**
- ⭐ **对照结论**：同一条 Mod↔host 边界，**读侧严格（F-0060 的对面）/ 写侧失败开放（F-0060-01）** —— 两半严格程度不一致 ⇒ 不是「整体松」，是写侧那次实现漏了考虑
- `mod-memory/src/lib.rs:711` + `strategy.rs:571` → 会话 id **三重归一**（宿主 / `on_scoped_event` / `resolve_session_store_path`），非法 id 退回全局桶 + warn（「不拿它拼路径」）
- **路径穿越假设被自己推翻**：`sanitize_session_id` 白名单含 `.` 故 `".."` 能过，但 id 的用法是 `format!("{id}.memory.jsonl")` —— **永远是文件名的一部分**，`/` `\` 被排除 ⇒ 分段不可能
- `lib.rs:718-721` → `AssistantReplied` 直接用本轮 id 选桶，**不依赖 `current_session`** ⇒ 避开切会话瞬间错位
- `mod-system/src/services.rs:158-160` → 契约声明「写不存在的键由 host 的 `SettingsPatch` 反序列化**拒绝**」
- `desktop/src/web_api/cli_entry.rs:188-200` → 生产实现只对**类型错**返回 `false`；`SettingsPatch` 系**无 `deny_unknown_fields`**（grep 零命中）⇒ 未知键被 serde **静默丢弃** ⇒ Mod 拿到 `true` 却什么也没改成
- `mod_registry.rs:371-372` → applier 把**整个 patch** 原样打进 info 日志（secret 若在 patch 里会进日志；文件层有 SanitizingWriter 兜、stdout 没有）
- 边界**零测试覆盖**：`mod_registry.rs:1311` 的回归只断言「合法 patch 被接受」，无非法键用例
- ⇒ **F-0060-01（P2）**：信任边界失败开放 + 契约声明与实现相反。这同时回答了 CONSOLIDATION-07 的待检验问题：「全新区域第一次被审就出 P1/P2」——**有**
## shell/flutter/lib/**

### BATCH-0023（7 文件 / 片段读 + 1 测试文件全读 · P0 0 P1 0 **P2 1** P3 0）—— **前端首批**

### BATCH-0024（6 文件 / 片段读 + 3 测试文件定点 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0025（3 文件 / 2 测试文件全读 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0026（4 文件 / 定点读 + 全目录扫描 · **零发现批**）

### BATCH-0027（跨审计去重批 · 0 新发现）

### BATCH-0028（3 文件 / 定点读 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0029（3 文件 / 片段读 · **0 新发现** · 核验批）

### BATCH-0030（3 文件 / 片段读 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0031（2 文件 / 片段读 · **0 新发现** · 对抗性核验批）

### BATCH-0032（4 文件 / 定点读 · **0 新缺陷**）

### BATCH-0033（3 文件 / 片段读 · **0 新发现**）

### BATCH-0034（5 文件 / 定点读 · P0 0 P1 0 P2 0 **P3 1**）

### BATCH-0036（2 文件 / 全读 + 跨语言对照 · P0 0 P1 0 P2 0 **P3 1**）

### BATCH-0037（3 文件 / 片段读 · **0 新发现** · 双快照核验）

### BATCH-0038（2 文件全读 + 跨语言对照 · **0 新发现**）
- `flutter/lib/audio/epoch_gate.dart` → **「停止即静音」红线的 Dart 端**：判据抽成纯函数的**理由被记录**（原实现埋在 import package:web 的 chat_controller 里 ⇒ VM 测试加载不了 ⇒ 回归抓不住）；实测依据「调度提前量峰值 3.03 s」；顺序陷阱写明
- `desktop/src/supervisor/turn.rs:674-677` → 「**NewEpoch 只由 stop 路径（`do_stop!`）在真正推进 epoch 时发送**」，且 `tests_stall.rs:232-239` 有回归钉子「stop 后根必须推进到 epoch=1」
- ⇒ **该红线跨 Dart/Rust 两侧各有一个钉子**；同时澄清：停止信号走 `NewEpoch`，**不**走 `TurnAborted`（空分支，B0035 已核）——两条并存不矛盾，F-0036-01 的记录准确
- `flutter/lib/chat/chat_session.dart:346-363` → 裁剪明确避开 `firstWhere(orElse:)` 反模式（「兜底反而把当前会话删了」），且宁可超限不删用户在看的那条；Dart 存储与 LLM 历史**完全解耦**（`ChatBody` 不带历史）
- **可测性纪律跨三文件一致**：`relativeTime`/`breakpoints`/`display_prefs` 都「显式传 now，不依赖系统时钟」，理由同为「在 VM 上才测得了」
- `flutter/lib/chat/chat_controller.dart` → 轮末才落盘（:497-500 理由写明）；会话增删改**每条**都先 `_abandonTurn()`（:507-510 头注说明「迟到 delta 落进新会话」陷阱已显式堵住）
- `flutter/lib/state/ui_state_tracker.dart` → 胶囊只认 `turn_state`/`text_delta{completed}` 收口、**不**因裸 error 帧收口（:181-182 写明分工）；与 ChatController 错误文案共用 `formatWsError`
- **核验结论**：`UiStateTracker` 与 `ChatController` 的两份快照是**刻意分工**——聊天链路走带兜底的 `settleTurn`，胶囊走权威帧。输入框的锁在聊天链路上，**不会被锁死**；两者不一致的失效半径只有显示层且下一轮自愈
- 跨语言投影链复核：致命错误 → `TurnStatus::Failed` → `RootEffect::TurnCompleted{outcome}` → `turn_state{status:"failed"}` ⇒ 胶囊**一定**会被收口（前三跳已在早前批次读原文）
- `flutter/lib/chat/turn_liveness.dart` → 全部收口判据（fatal 才收口 / null-epoch 判当前轮 / isProblem 而非 !isUsable）**与 Rust 侧逐条对得上**；它对 Rust 的断言「TurnAborted 不投影任何 WS 帧」**经核实为真**
- `desktop/src/supervisor/handlers.rs:324` → `RootEffect::TurnAborted { .. } => {}` **空分支承载跨语言契约**，而契约只写在 Dart 侧 (F-0036-01)
- **核验手法（可复用）**：Dart 注释里对 Rust 行为的断言是**可验证的** —— 拿它去被引用的那行读一遍，本批 5 条判据全部对上
- `flutter/lib/settings/display_prefs.dart` → `toJson` ↔ 构造参数 **24/24 对齐**、`:942` 的 `operator ==` **24/24 覆盖**（`main.dart:238` 的落盘门依赖它）；`:970` 用 id-only 的 `sameAs` ⇒ 潜伏陷阱 F-0034-01
- `flutter/lib/design/background_item.dart` → `sameAs`（只看 id）vs `==`（id+opacity+fit+align）的差别**写在头注里**（:368-375），并明说漏掉样式的后果是「静默失效」
- `flutter/lib/settings/sections/appearance_background.dart` → `:1264` 回调为 null 时「样式」按钮**不渲染** ⇒ 无回调时优雅降级，不是死 UI（假设证伪）
- `flutter/lib/settings/sections/appearance_section.dart` → `:212-227` 提供了 9 个回调，**独缺 `onItemChanged`**（全仓零提供方）
- **本批 4 个假设全部证伪**；净产出 1 条「装好枪没扣扳机」的潜伏陷阱
- `flutter/lib/app/browser_io.dart` → localStorage 一条记录；`saveDisplayPrefs` 返回 bool（丢设置必须报告，rc.3 §9.1）/ `saveChatSessions` 返回 void（丢追加日志可接受）——**不对称是刻意的**，两侧理由都写明；`pickImageDataUrl` 三态已修
- `flutter/lib/settings/display_prefs.dart` → `fromJson` 逐字段「类型化读取+钳位」，非显然默认值全记理由（:573/:592/:626/:608）；`clampSlideInterval:865` **哨兵 0 先行返回**，证伪了「关掉的轮播重启后自己活过来」
- **取舍判据（可复用）**：持久化函数的返回类型按「**丢了用户会不会心疼**」选 —— 表达过的意图必须报告，追加日志可以静默
- `flutter/test/app_shell_background_test.dart` → 模式 B **第 6 次复查未命中**：探针刻意用嵌套 `Builder` 从「被缓存子树之内」读 `BackgroundRuntimeScope`，`app_shell.dart:638-640` 声称的那条回归**真实存在且能失败**
- `flutter/lib/ui/field_row.dart` → `:618 error: resultIsError ? result : null`，`result` 全文件只出现 5 次、**唯一消费点**就是这一行 ⇒ 成功文案被整条丢弃。与 llm_section 的子串扫描复合后：**假成功 = 无任何输出**（为团队 F-0012-1 补机制）
- `flutter/lib/state/live_region.dart` → 1500ms 节流 + 可注入时钟 + dispose，AGENTS.md 那条读屏纪律已落实
- `flutter/lib/main.dart` → `_live` 接线正确（:555/:1019/:1172/:1321）
- `flutter/lib/app/app_shell.dart` → F-0005-2 **已修且两头都对**：缓存键 `(revision, tick, section)`；`tick` 由面板**自订阅** `_settingsTick`（:591-597），补上「浮层那条路根本没有外壳重建」；:638-640 预先警告 InheritedWidget 作用域陷阱并留回归
- `flutter/lib/main.dart` → 音频侧 `setState` 带 `event.muted != wasMuted` 守卫（:729-731）⇒ 50 次/秒的音频事件**不**重建外壳；`stageClock` 30ms 路径也不走 setState
- 观察（不记入 FINDINGS）：`main.dart:1170` `_settingsRevision++` 写在 `build()` 内，是「发生过一次重建」的代理量，注释把它说成了事实；方向保守（只多失效），不构成缺陷
- `flutter/lib/app/shell_prefs.dart` → F-0002-1 的两条通道已真去重（sync 桥身份+7 字段指纹 / stage-bg 按值）；**第三条 `actionScales` 被 `force: true` 绕过** ⇒ 滑杆仍每像素一帧 (F-0030-01)
- `flutter/lib/main.dart` → F-0001-2 轮播双索引**两形态都修**（`jumpTo` 同步 + `setLibrary` 后回写夹持值）；:436-439 是 actionScales 的 postMessage 下游
- `flutter/lib/live2d/action_scales_sync.dart` → `syncNow(force:)` 的 `!force && key == _sent` 短路（:161）就是被绕过的那道；`force` 的头注用途只写「iframe 重建后补发」
- **可借鉴判据（第二次出现）**：「修了一半」的形态 = 同一函数里前两行去了重、第三行用 `force: true` 立刻绕回去
- `flutter/lib/main.dart` → 核验：F-0013-1/F-0001-3（P1 水合竞态）**已修**（合并基座换成实时 `_prefs`）、F-0002-2 **已修**（`final` 单实例真库）、F-0002-3 **已修**（删掉永不触发的假兜底）。读 141-260 / 1225 行未读
- `flutter/lib/data/background_hydration.dart` → `applyBackgroundHydration(latest, …)` 只动背景域、补字节必须 `copyWith`、剪枝先 `probe()`；`source` 派生视图被显式封「线上勿用」——**误用点在类型层被堵死**
- `flutter/lib/ui/audio_bar.dart` → F-0002-1 的「每像素」半边**已修**（`_onDrag` 只改本地态，`_commit` 只在 `onChangeEnd`），且逐条枚举了键盘路径不漏
- **可借鉴判据**：水合那组「结构性修好」，解码缓存那组「机制对但容量口径与真实工作集矛盾」——差别在于**有没有把误用点本身堵死**
- `flutter/lib/data/background_decode_cache.dart` → 团队 F-0006-1/F-0016-1 的修复机制**正确**（LRU 保实例身份）；但**容量 3 < 实际工作集（壳壁纸 + 可见缩略图 8–11）** ⇒ 打开设置面板时壳壁纸每个流式 delta 重新解码 (F-0028-01)
- `flutter/lib/ui/shell_backdrop.dart` → :257 每次 build 调 `decodeDataUrlBytes(dataUrl)`；与缩略图共用同一缓存
- `flutter/lib/settings/sections/appearance_background.dart` → :930 `ReorderableListView.builder`（懒构建但含 cacheExtent，约 8–11 项）；:1472 与壳共用缓存函数
- **团队 rc.6 抽查**：F-0006-1 已修（带残留）/ F-0016-1 已修（带残留）/ F-0003-1、F-0002-1 未核
- `docs/plans/ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md` → 通读 250 行，提取其 45 条登记；**与我的 50 条逐条语义比对，重叠仅 1 条**（其 F-0017-1 ≡ 我的 F-0021-01，跨端事实的两个方向）
- 两轮口径对照：他们 Dart 117 文件 / 45 条 / P0 0；我 Rust ~70 文件 + 前端片段 / 50 条 / P0 0。**两套编号命名空间独立**（`F-0001-1` vs `F-0001-01`），勿混引
- 抽查三条已登记项的 HEAD 现状：F-0005-2（放大器 P1）**已修**（`settingsRevision` 机制在位）· F-0012-1（自检扫 `contains('ok')` P1）**仍存在** · rc.6 背景域未抽查
- `docs/audit/2026-09-28-frontend-nightly/` → 团队原账本已按 §2.1 入库，证据链完整；其 README 自陈计数口径与历史计数错误
- `flutter/test/no_backdrop_filter_test.dart` → B0026 · 自带反假通过钉子（isNotEmpty + ≥30 文件 + main.dart 存在性）；并**记录了本仓自己的一条第五种假绿灯**（contains('RepaintBoundary') 命中「不要直接铺」的注释⇒恒真），已于 rc.7 修复
- `flutter/test/visual_language_test.dart` → B0026 · `expect(findBareBold(bad), isNotEmpty, '扫描器连合成违规都抓不到')` —— **扫描器自身的判别力被测试**，比 isNotEmpty 更强
- `shell/flutter/lib/（全目录）` → B0026 · **红线 K 验证通过**：12 处 https?:// 逐条归类，全是 localhost / 注释里的文档链接 / 「为什么不能依赖 gstatic」的说明；web/index.html 零外链
- `flutter/test/design_tokens_lint_test.dart` → B0025 · **反假通过的范本**：自带「扫描确实覆盖到源码」钉子（含 main.dart 存在性与 ≥10 文件）、豁免面被审计（恰好 2 个且存在、不重复实现别处规则、不过度覆盖）、豁免表历史有记账。5 条规则**能失败**
- `flutter/test/font_subset_test.dart` → B0025 · 数据层防伪（重算字体 FNV-1a 比对，「换字体不重生成覆盖表比没门禁更危险」）+ 词法器自测（group ①）+ 校准位断言（两个历史违规字符仍不在子集）。**能失败**；但只扫编译期字面量 ⇒ F-0025-01
- `flutter/lib/chat/chat_session.dart` → B0025 · :410-414 明确按 rune 截断且注释写「中文与 emoji 都在」——**代码预期聊天文本含 emoji**，而全仓无运行态字符过滤（F-0025-01 的证据 1）
- `flutter/lib/app/app_shell.dart` → B0024 · **红线 N 实现正确且不变量就写在旁边**：三断点共用同一 Flex（:702-711「子树形状不变 ⇒ 舞台 Element 不被反激活 ⇒ iframe 不重建」），stage 带 GlobalKey。但该不变量**无测试** (F-0024-01)
- `flutter/lib/app/page_cross_fade.dart` → B0024 · 隐藏页用 `Offstage` 保留 Element/RenderObject（不 paint），并给出两条偏离参考实现的理由（保活 / 避免切换帧卡顿）
- `flutter/test/app_shell_layout_test.dart` → B0024 · 约 20 用例覆盖断点→导航形态与三种设置宿主，**无**「跨断点舞台身份未变」断言
- `flutter/test/settings_panel_keepalive_test.dart` → B0024 · 用 `initState` 计数把「有没有被重建」变成可断言事实 —— 正是 F-0024-01 建议照抄的手法
- `flutter/lib/live2d/stage_pointer_interceptor.dart` → B0023 · 红线 M 的实现与头注是全仓文档质量上界（根因画成 DOM 树、诚实记录垫层管不到的两处）；**但 `enabled` 这条更难的不变量零测试** (F-0023-01)
- `flutter/test/stage_pointer_interceptor_test.dart` → B0023 · 172 行 8 用例，用 `find.ancestor` 验祖先关系（能失败）；覆盖三种宿主 × 四类覆盖层 + VM 透明性。无 enabled 断言
- `flutter/lib/app/app_shell.dart` → B0023 · 9 处垫层使用点逐一读过：两处常驻（CollapsiblePanel / PageCrossFade）**正确**传了 enabled；其余条件挂载无风险
- `flutter/lib/{audio/audio_player,live2d/live2d_bridge,ui/stage_host,live2d/live2d_stage}.dart` → B0023 · 红线 O 成立：30ms 轮询 + 33ms 二次节流 + GlobalKey→Bridge→postMessage，**不进 Widget 树**
## shell/flutter/test/**
## xtask / scripts / tests / shared / verification / .github/workflows
- **P1/P2 复核**：F-0046-01（渲染面 `ev.origin()`=0 vs Dart 侧=1，不对称未变）· F-0048-01 · F-0049-01 · F-0050-01 全部仍成立
- ⭐ **零发现长跑归因**：B0054–B0058 连续 5 批零发现，每批只读 1–2 文件且**都是追前面显式留下的「未读」条目** ⇒ 线索追踪的边际产出低
- ⇒ **模式 L（新）**：同根链上「已验过的节点」，其邻接代码预期产出低；新发现来自**从未被追过的根**
- ⇒ **停止规则**：同一根因链上**连续 2 批零发现 ⇒ 强制换根**（本次连做 5 批，是规则缺失的代价）
- ⇒ **队列重排**：切 **8 个 Mod crate**（61 文件 / 24,665 行，**全 0 审**）——红线 R（Mod 边界与秘密）的实现侧

### BATCH-0048（4 文件全读 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0049（1 文件全读 · P0 0 **P1 1** P2 0 P3 0）

### BATCH-0050（3 目标 · P0 0 P1 0 **P2 1** P3 0）

### BATCH-0051（3 目标 · 0 新发现 · 两处既有发现的修订）

### BATCH-0053（1 文件全读 · **0 新发现** · 门禁轴结项）

### BATCH-0054（2 文件 / 片段读 · **0 新发现** · 渲染面续）

### BATCH-0055（1 文件定点 · **0 新发现** · ⭐ 红线 N 结清）

### BATCH-0056（1 文件 / 定点读 · **0 新发现** · 红线 P 渲染侧落点）

### BATCH-0057（1 文件 / 枚举 + 精读 · **0 新发现** · 模式 H 复查）

### BATCH-0058（3 文件 / 定点读 · **0 新发现** · 帧循环 + 资源侧）
- `web/surface/render.rs:256-270` → `tick` 的 dt 钳 **[1/240, 1/15]s**「防巨步（**切后台回来**）与零步」；`core.update` 失败 ⇒ 先 `drop(st)` 再 `status()` 再 `return`（顺序错会 RefCell panic，此处正确）
- `:246-254` → 帧循环**记录了被推翻的方案**（R4 60Hz accumulator：修好 240Hz 四倍速、引入「姿态每 4 帧跳一次」视觉步进）⇒ **第三处「记录推翻过的方案 + 实测理由」样本**
- `l2d/src/renderer/model_core.rs:91-92` → 换模型时 `self.stack = PoseStack::from_loaded_model(loaded)` **赋值替换** ⇒ 旧 PoseStack（含**有状态**的 `physics_work`/`PhysicsEngine`）被 drop
- `l2d/src/model.rs:69-71` → `ModelHandle` 是纯 `Arc<ParsedModel>`、**无自定义 Drop**（正确设计，非遗漏）
- **边界标注**：纹理/wgpu 资源在**上游 crate `ayagami`**内，不在本仓范围，**不作断言**；红线 N 的时序侧（B0055）+ 本仓资源侧（本批）均已核
- `preset/tests/fields.rs` → 11 条测试名**直接就是协议语义 ID**：C3 先加后钳 / D26 跨批 replace / V5 hold·ttl / 缺参数不静默 / wire 名冻结（红线 Q）/ 时钟域切换 / 恰好一次 / 通道白名单
- 精读 `same_field_cues_add_before_clamping`(:157-189)：断言**四元组** `raw==36`（钳前和）· `value==30`（钳后）· `clamped==true` · `[Applied, Applied]`（两条各自回执不合并）⇒ **能失败**
- 覆盖面的**细微空隙**（非假绿灯，记备忘）：对称钳位下「逐条钳位再相加」与「先加后钳」数值相同，本测试不区分该排列
- ⇒ **模式 H 在 preset 侧不成立**：三条核心语义都有可失败的定点测试（累计复查 5 次，命中仍为 0）
- `preset/field_runtime.rs:526-575` → `accept_cue`：跨 epoch **清空全部字段**（D26，与 Dart `audioEpochChanged` / Rust `epoch_gate` **三层同口径**）· `none` 撤销哨兵仍写 `prev`（后续 `after_prev` 仍可解析）· 未知/缺失 id **不静默**而发 `dropped`（与 desktop 侧同纪律）
- `:788-804` → 时钟 `ClockDomain` 切换时 `dt` **归零**，域切换不会把 `elapsed_ms` 推成垃圾值
- `:3-8` → 行数豁免头注写「当前 953 行 / 上限 1000」——**声明与事实相符**；**对照** F-0044-02（`chat_routes.rs` 实测 1086 却写「约 560」）
- ⭐ **记账卫生判据**：写下的那一刻核对过数字的注释至今都真；跨多轮改动没人回头核的都已漂。**腐烂机制不是「写错」，是「没人在改动的同一次里重新核对」**
- `:10-14` → 纯逻辑纪律带**踩坑记录与受害者文件名**（`mouth.rs` / `stage_bg.rs`）
- `l2d-wasm-demo/src/main.rs:338-343` → 启动时序**刻意排序**：`install_stage_bridge` → `install_stage_interaction` → `emit_event("ready")` → `emit_event("loaded")` → `start_render_loop`（rAF 最后开）
- ⇒ **`ready` 发在 listener 装好之后**，宿主见 `ready` 即可下发，**无「消息早于监听器」竞态**
- 宿主侧重发（B0030 已核）：`_attach` 每次挂桥**无条件重发** `stageImage`；`onReady → _applyPrefs` —— 两侧口径一致
- 换模：成功/失败分别发 `loaded`/`error`（:670/:674），失败**不**伪装成功
- ⇒ **13 条红线全部结清**（这是唯一一条此前未做正向验证的红线）
- `l2d-wasm-demo/src/main.rs:919-961` → 帧循环**无每帧分配**：单一常驻 Closure + rAF 调度；fps **复用 rAF 时间戳每秒发一次**「不额外起定时器」⇒ 没有第二个 60Hz 源
- `main.rs:877/902/…` → wheel / dblclick / 拖拽三处都查 `bridge.click_enabled` ⇒ 与 B0030 核的 Flutter 侧「发送 clickEnabled」**首尾对上**
- `l2d/src/pose_stack.rs:1-21` → 五层各持独立 `Pose`、**每帧重新合成** ⇒ 「未被任何层写入的通道不出现在合成结果里」= B0043 那条「释放所有权而非写零」的**结构落点**；头注还记了它替换的旧实现两条病症（idle 被用户值永久压制 / `clear_parameter` 撤销不传播）
- `pose_stack.rs:178-182` → 非法 dt（NaN/Inf/≤0）在 `elapsed += dt` **之前**被拒为 `InvalidDt`，与 :175「任何状态都不变」的承诺一致
- `scripts/verify_core_chain.py` → **「绝不伪造绿灯」在脚本侧成立**：`:443` 退出码由失败集决定（无部分成功路径）；超时判**失败**不判跳过（:434）；采样率/口型电平/句界三条必查项**显式禁止静默跳过**（:385「跳过等于假装验过了」）
- ⭐ `:437-440` 把休眠台账写成**可执行断言**：`action_state`（rc.2 已删）出现即 FAIL「动作系统又接线了」——与 F-0040-01（AGENTS.md 点名 `_three` 而真名 `_five`）构成**直接对照**：同一件事，一个做成了可执行断言，一个只写在文档里且已漂
- ⭐ **门禁轴最终结论**：「门禁不弱，弱的是门禁自己的诚实性」——可靠三处（xtask 排除表 / nightly opt-in / 本探针）· 覆盖洞三处（F-0048-01 / F-0049-01 / F-0050-01）· 诚实性失真一处（F-0049-01 的 docstring）
- `scripts/ignition-precheck.sh:134` → **逐字复制**同一份错误探针清单，且比 `ignite.sh` 更弱（只 grep `main.dart.js`）⇒ **F-0050-01 升级为「被复制的错误假设」**（同句在仓内 2 次，均漏 `flutter_bootstrap.js`）
- `AGENTS.md:105 / :608` → 另两处**已**更新为 `mod_count_is_three` → `mod_count_is_five` ⇒ **F-0040-01 指控范围收窄**为「单份文档内部不一致（休眠台账那一节没跟上）」，非「通篇未更新」；已显式改写
- 仓内三处对照：`main.rs:471` 测试名 `_five`（断言 5）· `ignition-precheck.sh:156` 文案「五个已注册」· AGENTS.md 休眠台账仍 `_three`
- `scripts/ignite.sh:89/94` → gstatic 体检只探 `index.html` + `main.dart.js`；用**门禁那条一模一样的模式**实测真实产物：这两个文件命中 **0**，而 `flutter_bootstrap.js` / `flutter.js` 命中 **1**（门禁从不探）
- 活的 CDN 决策就在未探的 `flutter_bootstrap.js`：`canvasKitBaseUrl ? … : e.engineRevision && !e.useLocalCanvasKit ? W("https://www.gstatic.com/flutter-canvaskit", …)` ⇒ **探针集 ∩ 字符串所在文件 = ∅**
- 门禁的**模式对、层次对**（先查 HTTP 200 再 grep，避开「404 空 body 假绿」），**唯独文件清单选错**；当前构建是安全的（`--no-web-resources-cdn` 置 `useLocalCanvasKit` ⇒ CDN 分支为死代码）
- ⇒ **F-0050-01（P2）**：指控是「门禁抓不住它被写出来要抓的那个故障」，**不是**「产品现在坏了」；边界如实标注（只读约束下无法构造坏构建实证假绿，禁跑 `flutter build`）
- ⇒ **升级 F-0048-01 的影响段**：红线 K **没有任何可用的自动化防护**（CI 无 + 本地不可靠）。修法有先后依赖：先修本条，再把 `ignite.sh --check` 接进 CI
- `xtask/src/main.rs:30-89` → rust-ratio 排除表**窄且有据**（7 目录名逐条理由；`dart` 豁免但**仍单独打印行数**「防止前端悄悄膨胀成为审计盲区」）；我「宽松排除表让门禁自己及格」的假设**证伪**
- `scripts/check_public_secrets.py` → 三条可复跑事实叠加：① **CI 从不调它**（B0048 已穷举全部 `run:`；AGENTS.md 门禁表里也没有）② `:5` docstring 声称的 **Gitleaks 兜底在仓库里不存在**（`grep -rni gitleaks` 唯一命中就是这句话自己）③ `INCLUDED_PREFIXES` **不含 `shell/`** ⇒ 217 个 Dart 文件 + docs 不在扫描面
- ⇒ **F-0049-01（P1）**：误粘进 `.dart`/文档的 `sk-…` 可进仓库而无任何自动化会响。整套密钥纪律的**最后一环没有门禁**，且**门禁的自我说明与事实相反**
- 附带：行级豁免 `_is_test_line` 大小写不敏感地匹配 `INJECTED` ⇒ 同行真密钥被跳过（与 F-0006-01 的 `redact` 只认键名同形，是「键名/标记驱动 ⇒ 会漏」的第三例）
- 反证留档：AGENTS.md 的门禁表**没有**这一行 ⇒ 骗人的是**脚本自己的 docstring**，不是项目宪法；措辞已据此限定
- `.github/workflows/*.yml` → 穷举全部 `run:` 步骤：Rust 六道 + Dart 两道 + 根 pytest **在位**；E2E 探针 `if: vars.LIVE2D_AI_VERIFY_BASE_URL != ''` ⇒ 未配置时 **skipped 而非 passed**，「绝不伪造绿灯」**实现正确**
- 但 **零** `flutter build` / **零** `trunk build` / **零** `ignite.sh --check` / **零** gstatic 检查 ⇒ **红线 K 无构建期门禁**；`--no-web-resources-cdn` 在 CI 里从未被执行 (F-0048-01)
- AGENTS.md 为 **wasm** 半边声明了「已知缺口」，**前端**半边未声明 ⇒ 本条是「同一类缺口的另一半，漏记」
- 假设证伪两条：`requirements-test.txt` **在树里**（root-py-tests 可运行）；`build-upload.yml` 的 4 个多行 `run:` 块逐块读过，无前端构建
- `pr-checks.yml` 的 `paths-ignore: *.md / docs/** / .pi/**` 合理；AGENTS.md「本地必跑 vs CI 必跑」表逐条核对**属实**

### 第十次对账（CONSOLIDATION-10）· 账本口径更正 + 规则 3 五变体提炼
- **15 条 P1/P2 逐条复核：15/15 仍成立**（含 F-0034-01 升级后的 P2）
- ⚠ **口径更正**：STATE 手记的「67 条」**是错的** ⇒ 机械去重后 **64 条**（P1 9 / P2 29 / P3 26）。`F-0002-01` 两个标题（补记回填）、`F-0006-03` 两个且**跨级**（原 P2 → B0012 升 P1，两处皆为有意历史）
- **教训**：计数必须**机械算**，不能每批手改 +1 —— 升级/撤回/回填都会打破「+1」假设；错数在几十批里会慢慢漂成**看起来可信的错数**
- ⭐ **模式 L 规则 3 的五个变体正式提炼**（共同根：**我的默认假设是「我看到的就是全部」**）：① 完整性断言要读到结尾标记 ② 工具解析出空 ≠ 不存在（含辅助函数里的读取）③ 「条件依赖某值」要追到赋值点 ④ 先确认文件是 `part` 还是独立库 ⑤ grep 查调用要覆盖三形态
- ⇒ **执行要求**：任何**否定式断言**（「没有 X」「从未 Y」「默认是 Z」）必须附**穷尽性证据**；**没有的一律降级为「未核实」写进 STATE，不进 FINDINGS**
- 覆盖率（机械，195 条去重路径）：Rust `web_api`≈40 · `runtime` 22/43 · `core` 9/15 · `l2d-wasm-demo` 7/22 · `l2d` **0/16** · Mod crates **0/8**；前端 **35/217**；CI 4/5 · scripts 4/12

### BATCH-0091（3 文件 · 0 新缺陷 · ⭐ F-0046-01 机制削尖）
- `l2d/src/lib.rs:19-20` + `asset/mod.rs:14-16/73` → crate **确有**路径加固且很严（拒绝 `..`/绝对路径/`\`，错误携带原始资源名）—— 正面
- ⭐ **但它在抓取之后**：`main.rs:170-175` 先解析**刚抓回的**字节 → `net.rs:52-63 resolve_relative`（**纯文本拼接、零校验**）→ **立刻 fetch** ⇒ `asset/mod.rs` 的加固排在最后，请求早已发出
- ⇒ 攻击者 model3.json 的 `"Textures": ["../../../../api/v1/settings"]` 可让浏览器**抓本机服务任意路径**；**修法增补 ③：抓取前校验 ref**（只加 origin 校验不够 —— V1 `?model=` 不走 postMessage）
- **危害严格限定**：`..` **不能换 host**（`dir` 前缀恒在），绝对 URL 与 `//host` 都被拼成畸形同源路径 ⇒ 仍是**盲抓 + 同 host 路径上跳**，不夸大成任意主机 SSRF

### BATCH-0092（1 文件 · **0 新发现** · 模式 H 第 7 次复查零命中）
- `asset/mod.rs:302-321` → `normalize_resource_ref` **比「归一后放行」更严**：任何一段是 `..` ⇒ **直接拒绝**（不是折叠），不依赖归一逻辑正确
- 四路读点（:211/:215/:219/:223）**全部**经过它，落盘读是 `base_dir.join(规范化名)` ⇒ **磁盘入口无穿越面**
- 测试**恰好钉住难形态**：`for bad in ["", "/abs/moc.bin", r"a\b.png", "../up.moc3", "a/../b", "."]` ⇒ **`"a/../b"`（嵌入式上跳）在列**（朴素的 `starts_with("..")` 会漏），且断言错误**携带原始资源名**
- ⭐ **F-0046-01 修法 ③ 强化**：要加的校验**已存在且已有测试** ⇒ 修法变成「把 `normalize_resource_ref` **提前一个层次调用**（在 `net.rs` 的 `fetch_bytes` 之前）」，**规则无需新设计**；与 F-0034-01 同形（正确判据在同一 workspace，调用方用了更弱的）

### BATCH-0093（3 文件 · **0 新发现** · 接缝证伪 + 档位 + GPU 获取）
- ⭐ B0092 标的「潜在第二处接缝」**结构性证伪**：`from_memory_map` → `from_memory` → **唯一 `assemble`**（:199-206），四路读全经 `normalize_resource_ref`；头注写明「两条入口在此汇合，**行为必然一致**」
- ⇒ **本审计第 4 个「汇合/漏斗」样本**（`_applyPrefs` / `confirmLeave` 三处 / `secrets::lookup` / `assemble`）⇒ **可提炼**：本仓表达「多入口一致性」统一用「**私有汇合 + 策略闭包**」，而非「两条路各自实现 + 测试保证相同」
- `tier.rs:38-45` → `RenderTier` **闭枚举**（4096/8192/16384）+ `from_side` 穷尽 `match`，其余 `None`；调用方 `if let Some(…)` ⇒ **忽略非法值不钳位**；头注**如实标注代价**（16384:1GB，「需真机确认」）
- `gpu.rs:95/129-135` → **逐个适配器尝试** + 每次失败记 note（adapter 名 + 错误）+ `Err(NoGpuBackend(notes.join("; ")))` **聚合全部原因**；用户要 16384 而 GPU 不支持时**不静默降级、如实失败** ⇒ 与「不谎报」纪律同族

### BATCH-0094（1 文件 · **0 新发现** · 热路径 + oracle 核验）
- `renderer/mod.rs` → **维度 D 干净**：全文件分配只在 `save_png`(:187) 与测试(:401/:406)，**生产热路径无 `vec!`/`collect`/`to_vec`/`String`**
- `:144-156 alpha_coverage` / `:160-167 visible_bbox` → **冒烟 oracle 不是恒真的**：全透明帧 ⇒ `0.0` / `None`，断言 `> 0` / `is_some()` **会真红**；空/零尺寸**显式返哨兵**不 panic；用 `as_chunks::<4>()`（`chunks_exact`）无越界面 ⇒ **模式 B 的反例**
- 有界观察（备忘）：`as_chunks::<4>().0` 静默丢弃残余、分母用**声明的** `width*height` ⇒ 若尺寸不符会**偏小**而非报错；但不变量由构造保证（:138），且「尺寸严重不符」会被 `coverage > 0.5` 类断言抓住 ⇒ **不会静默通过**
- `crates/l2d` 16 文件**已读 7** ⇒ 覆盖率约 **45%**

### BATCH-0095（1 文件 · **0 新发现** · 模式 H 第 8 次零命中）
- `format.rs:126` → **先查长度再索引**（`len() < 64 || header[..4] != MAGIC` 同一 if ⇒ 短路）⇒ 空/6 字节输入都不越界；字节序与版本均走**闭集白名单**，未命中 ⇒ `Unsupported(原值)` 不推断
- ⭐ 测试**钉住会 panic 的形态**：`guess_format(b"MOC3\x04\x00")` = **6 字节但魔数与版本字节都合法**（长度检查若后移就 **panic** 而非 fail）· `&BAI_HEADER[..63]` 把边界钉在**差一** · `b""` 空输入
- 另有 `illegal_endian_flag_is_unknown` + `other_known_versions_are_not_v0_guaranteed`（逐版本断言「识别为 Moc3 但**不被 v0 背书**」）⇒ 白名单与 v0 保证被**分开钉住**
- **`crates/l2d` 覆盖小结**：5 批 · 按文件 **8/16 = 50%** · 按行约 **45%**；**0 条缺陷**、多个正面样本、1 次既有发现修法削尖（F-0046-01）

### 第十一次对账（CONSOLIDATION-11）· ⭐ 立模式 M 与正面模式 P2
- **15 条 P1/P2 复核：15/15 仍成立**（十一批无一被推翻）
- ⭐ **新立模式 M「防御存在 ≠ 防御在正确的层」**（3 实例，跨三轮互不相关的文件）：F-0034-01（`DisplayPrefs.==` 绕过更强的 `BackgroundImage.==`）· B0092（`normalize_resource_ref` 正确但排在**抓取之后**）· F-0062-01（三段各自正确、**职责无人认领**）
- ⇒ **与模式 D 的区别**：D 的修法是「把注释改对」；**M 的修法是「把已有的正确组件接到该接的地方」——不需要新写任何规则** ⇒ 修复成本极低、争议极小
- ⇒ **审计推论**：看到「某处**没有**校验」时**先全 workspace 搜这个校验是不是已存在**；存在 ⇒ 报 **M** 而非「缺规则」
- ⭐ **正面模式 P2（新）**「私有汇合 + 策略闭包」（4 实例：`secrets::lookup` / `_applyPrefs` / `confirmLeave` / `asset::assemble`）⇒ 在**类型层面**排除漂移，本仓**最值得学**的工程习惯
- 「`crates/l2d` 五批零发现」是**有信息量的结果**（该区是**从未被任何一轮追过的根**）：5 批 45% 覆盖 ⇒ 0 缺陷，产出 4 正面样本 + 1 次修法削尖 + 2 次假设证伪
- 覆盖率（机械，203 条去重路径）：Rust `.rs` **69** · 前端 `.dart` **35/217** · CI/脚本 4 · Mod crates **0/8**

### BATCH-0097（2 文件 · **0 新发现** · 结掉 7 批未核实项）
- ⭐ **`ttl` 执行点找到了**：`FieldRuntime::frame(wall_ms, sink)`（`field_runtime.rs:596`），到期条件 `:628 !c.hold && c.elapsed_ms >= c.ttl_ms`；`remove(i)` 后 **`i` 不自增** ⇒ 连续到点不跳元素
- 逻辑正确：`!c.hold` ⇒ **hold 的贡献永不到点也不发 `preset-expired`**（合头注冻结语义）；索引删除的经典错没犯
- ⭐ **规则 3 第 6 变体（正面查找方向）**：B0056/0057 我依次试过 `expire`/`tick`/`advance`/`evaluate` 四个**猜的**名字，真名是 **`frame`** ⇒ **「我猜的名字没找到」不能推出「它不存在」**；必须先枚举全部方法再比对。与变体 1–5 同根
- 「两套 runtime 是同一职责的两个实现」**证伪**：`mod.rs:658`「与旧的 `preset_id` 双槽**并存**：旧路径**一字不删**（V11）」，且旧路径仍可达（`main.rs:507-533` 按 `payload.field` 是否存在分流）⇒ 是**两条协议路径**而非重复实现

### BATCH-0098（3 文件 · P0 0 P1 0 P2 0 **P3 1**）
- ⭐ **F-0098-01（P3）**：`preset/mod.rs:58` 与 `:696` 说缺参数「**静默降级**／不报错」，而 `field_runtime.rs:28-29` 说「**缺参数不得静默**」→ 发 `preset-dropped` / `applied+degraded`。**实现是「不静默」的一方 ⇒ 文档低估了保证**（安全方向）
- ⇒ **修正我自己的模式表**：模式 D 只跟踪「注释**高估**」；实际有**高估（危险）**与**低估（安全）**两个方向，两者都会让读者误判行为
- 核验通过：`ALLOWED_PARAMS` **13 条**、**不含 `ParamMouthOpenY`**（机械确认）；白名单**单一真源**（定义在 `table.rs`，`mod.rs:692` 再导出，`field_map.rs:22` 消费）⇒ **无第二份清单**
- ⇒ **红线 O 第三层保险**（协议侧结构禁止动作写口型），与 core 侧 `ParameterMask`（B0043）**同构**

### BATCH-0099（1 文件 · **0 新发现** · 白名单无法被突破）
- `field_map.rs::parse_targets:340/345` → 外置 `field_map.json` 路径上**两道独立闸**（`ALLOWED_PARAMS` + `field_allows`），每道拒绝**都带 warning** ⇒ **无法**引入 13 条外通道、**无法**让 `body`/`head` 写五官通道
- ⇒ **红线 O 第三层保险在内建 + 外置两条路径上都成立**（本批否掉了我上一批提出的「保险可能失效」的可能）
- ⭐ **第 4 个「显而易见的实现会踩红线、他们选了更严的并写下为什么」样本**：`Field::Expression` 用**显式小词表**而非 `scale_class == Expression`，注释点名松写法会放进 **`ParamMouthOpenY`（口型）/ `ParamBreath`（呼吸）** ⇒ **正面模式 P3 待立**：「更松的写法在哪里」要写进注释
- **补强 F-0098-01**：拒绝时**明确 push warning** ⇒ `mod.rs:58/:696` 的「静默降级」是**第三处**不成立；正确描述需区分两条路径（`field_map` 丢弃+warning vs `field_runtime` 发 `preset-dropped`）

### BATCH-0100（1 文件 · **0 新发现** · 覆盖表可达性 + F-0098-01 第四次补强）
- `git ls-files | grep actions/` ⇒ 只有 `preset_labels.json` + `presets.json` ⇒ **`field_map.json` 不在树里**，而 `with_override_json` 在**生产**被调（`main.rs:236`）⇒ 覆盖路径是**给用户的扩展点**，**出厂缺它是常态**（头注已声明「本轮不要求每皮套一份」）
- `main.rs:230/234/244-247` → 降级**三层、每层带原因**；告警**条数与内容**上屏
- **边界澄清**：此处 `fetch_bytes` 用**编译期常量** `FIELD_MAP_URL` ⇒ **F-0046-01/B0091 的顾虑不适用**于这条调用
- ⇒ **F-0098-01 第四次补强**：实际降级**没有一条是静默的**（四处反例），`mod.rs:58/:696` 是**唯一**声称静默处 ⇒ 措辞不是「过时」而是**与实现相反**

### BATCH-0101（1 文件 · **0 新发现** · 红线 K 第三层核验）
- `l2d-wasm-demo/src/main.rs:102/107/111` → 渲染面 URL 常量**恰好三个**，**全部同源绝对路径**（`/models/…` · `/actions/presets.json` · `/actions/field_map.json`）；`https://` 在该 crate 非注释代码里**零命中**
- ⇒ **红线 K 三层独立证据齐**：① 源码（宿主+前端，12 URL 全注释/localhost）② **源码（渲染面，本批）** ③ 构建产物（`ignite.sh --check`，但**探针选错**，F-0050-01）
- 闭合 B0100 的边界澄清 + B0091：`FIELD_MAP_URL` 是同源路径，`resolve_relative` **无法换 host** ⇒ **连动态构造的 URL 也在同源内** ⇒ 渲染面**没有出网路径**

### BATCH-0102（1 文件 · **0 新发现** · preset 外部表逐字段有闸）
- `table.rs:248-275 parse_param_list` → `presets.json`（**在树里**、用户可编辑）每条参数四道闸：非数字丢弃 · **白名单外丢弃** · 非有限丢弃 · **超幅值钳位**；**每道都带 warning**
- `from_json:131-200` 逐项防御：空 id 跳过 · **重复 id 保留先出现** · 非法 kind 跳过 · `duration_ms` 校验并 `.min(MAX_TTL_MS)` · 未知 wave 宽容回落 · morph 极不全则失效回落
- ⇒ **红线 O 协议侧保险三条路径齐**：内建表（编译期）· `field_map.json`（B0099 两道闸）· **`presets.json`（本批）**
- 结构差异备忘（不记发现）：字段路径有 `field_allows` 做**字段↔通道**闸，预设路径只有 `pack_limit` 管**幅值**；后果限于「用户自改本地 JSON + Motion 包声明五官通道」，无安全/数据影响，两道闸仍在

### BATCH-0103（1 文件 / 读 1-142 · P0 0 P1 0 P2 0 **P3 1** · ⭐ 门禁轴结项）
- **F-0103-01（P3）**：`:134` 只有 `cargo build --release`、`:136` 只有 `ls -lh`（**无** test/fmt/clippy/rust-ratio），而触发器是 `push: tags: v*` ⇒ **直接推 tag 产出从未跑门禁的发布工件且 workflow 不红**；正常流程下 tag 来自已过 PR 的提交 ⇒ **范围如实标注**，问题只在于「tag 是发布入口」无任何文字说明它不受门禁保护
- ⭐ 正面：**本审计最高的事故记录密度** —— Linux apt 步带**两个 run ID** + 「**根因至今未确定——不要再写未经证实的结论**」+ 「**不用 apt 的退出码当成功判据**」；`+crt-static` 移除记录日期 + 两个独立问题叠加的诊断 + 正解（musl）为何不在本 RC；`shell: bash` 带 2026-09-11 windows 失败诊断
- ⭐ **门禁轴结项**：设计**可靠**（门禁齐全、opt-in 显式 skipped 不伪造绿、不拿工具退出码当判据、事故带 run ID）· 覆盖**三处洞**（F-0048-01/0049-01/0050-01）· 诚实性**两处失真**（不存在的 Gitleaks / 「静默降级」被四处实现反证）· 发布入口**不受保护**（F-0103-01）
- ⇒ **一句话**：代码纪律强、门禁设计也强，但**覆盖面**与**自述的诚实性**各有缺口；而**最重的一条 P1 不在门禁里，在跨源边界上**（F-0046-01）

### BATCH-0104（边界扫描 + 1 定点 · **0 新发现** · Mod 根首审）
- **写侧**：`fs::write`/`File::create` 命中**全部**在 `memory` Mod **自己的 store**（JSONL/摘要 JSON，原子写）；`read_to_string` 命中在 400+ 行处**都是测试** ⇒ **没有任何 Mod 触碰 `live2d-ai.toml` 或 `.env`** ⇒ 红线 R 写侧成立
- ⭐ **密钥侧**：唯一带**真实 HTTP 客户端**的 `director`，生产构造 `staging_http.rs:100-112` **只收变量名**、值经 **`live2d_ai_runtime::secrets::lookup(env_name)`** 解析；头注 :9 写明不读 `[llm]`、「二路断了不影响主链」、该路「默认关」
- ⇒ 「密钥真源 = `.env`」「Mod 不得持有密钥」**在实现层成立**；且 Mod **复用**宿主 lookup 而非自己 `std::env::var`（后者会绕过 `.env` 快照 ⇒ AGENTS 记载过的坑）
- ⭐ **正面模式 P2 的第 5 个消费者**：「私有汇合 + 策略闭包」**被 Mod 复用** ⇒ 这是它最值得学的原因

### BATCH-0105（2 文件 + 目录清单 · **0 新发现** · director 可观察面）
- `director/src/lib.rs:804` → `"api_key_set": self.staging.has_api_key()`（**方法返回值**）+ `tests_staging.rs:676` 断言缺省态为 `false` ⇒ 头注 :111「密钥**只回布尔**、永不回值」**声明与实现一致**（与宿主 `SettingsView.has_api_key` 是同一纪律的两个实现）
- `lib.rs:130-133` → 日志只写**推导结果与正文长度**、不写原文，并**显式交叉引用**宿主「不记录请求体」；`:123-128` 命令路径不触碰下行通道、**断言带名字**（`command_paths_never_touch_action_or_settings`）
- ⚠ **规则 3 第 7 变体**：`grep -B 3 -A 3 … | head -16` **被 head 截断**、恰好没露出 `lib.rs:804` ⇒ 我起了「该字段只存在于注释」的假设（**正是模式 D 的形状**）；**穷举版 grep 立刻给出 4 处命中** ⇒ 假设当场被自己推翻
- ⇒ **本审计第 7 次同族失误**，但**被最快抓住的一次**（同批内就做了穷举复核）

### BATCH-0106（1 文件 **全读** · **0 新发现** · 「默认关」是结构性的）
- ⭐ `staging.rs:48-59` `DisabledStaging` 是**空对象（null object）**：**没有 HTTP 字段**、`complete` 恒 `None` ⇒ **结构上不可能发请求**；`disabled()`(:75) 与 `degraded()`(:83) **两个构造器都注入它** ⇒ 头注「与『默认关』逐字一致」是**类型保证**而非纪律
- `:119-150 assemble` **三道闸**才可能出现请求能力；即使闸开、端点非法（`new` 返 `Err`）⇒ 注入的**仍是 `DisabledStaging`** + 可见降级原因 ⇒ 「开了闸但没接上」也安全
- `:130-142` 「没有密钥」的**两种情形被分开描述**（「没配变量名」vs「配了但查不到值」）且 note 里**只有变量名、绝无值**
- ⭐ **正面模式 P3 第 5 样本、且最强一档**：前四个都是运行时的更严分支，本例**把「关」建模为「不存在该能力」** ⇒ **建模优于加判断**（前者由类型保证、后者由纪律保证）

### BATCH-0107（3 文件 · **0 新发现** · ⚠ 更正 B0104 的「唯一」）
- ⚠ **我 B0104 写「唯一带真实 HTTP 客户端的 Mod（director）」是错的** —— 穷举 `grep -rln reqwest` 得**四个文件**：`director/staging_http.rs` · **`memory/summary_http.rs`** · **`memory/summary.rs`**（共用客户端工厂）· `local-llm/lib.rs`（**已废止未注册**）
- ⇒ **规则 3 变体 7 的第二次代价、最贵的一次**：`grep … | head -6` 恰只露出 director 三行 ⇒ **基于不完整枚举对全仓下「唯一性」结论**。补执行要求：**支撑「唯一/只有/没有别的」的枚举必须 `-l`/`-c`/无 `head`**
- **更正后红线 R 判定更强**：`director` 与 `memory` 两 Mod 密钥纪律**逐字同构**（变量名 → `secrets::lookup` → `with_api_key`）；`with_api_key` 在**三个** crate（director/memory/runtime-performance）都**只被测试直接调用** ⇒ 正面模式 P2 消费者 5 → **6**

### BATCH-0108（1 文件 · **0 新发现** · persona 的 PNG 卡解析）
- 假设一「`read_itxt_chunk` 切片越界」**证伪**：索引全部由 `position` 得到的 NUL 位置推导（恒 `< len`），`?` 传播 `None`，切片至多为空（`&v[len..]` 合法）
- 假设二「`read_u32` 裸索引越界」**证伪**（结论更值得记）：它**只有一个调用者**（:408），而循环条件 `while offset + 8 <= bytes.len()`（:407）**同时覆盖** `read_u32` 的 4 字节与其后的 `&bytes[offset+4..offset+8]`
- ⇒ **结构事实**：私有无检查原语 + 恰好一个带守卫的调用者。本仓可接受，但**对未来改动脆弱** ⇒ 备忘级建议：给 `read_u32` 加 `debug_assert!` 或返 `Option`（**让约束跟着函数而不是调用点**）
- 正面：`:404` 头注「**先量再取**」；`:412` 先量后取且错误带块长度与剩余字节；压缩 iTXt **显式拒绝 + 可执行建议**（:426-429）；`parse_chara_payload` base64 双模式 + 只接受像 JSON 的（否则明确报错）

### BATCH-0109（2 文件 + 目录清单 · **0 新发现** · ⭐ 跨会话泄漏这条路关着）
- ⭐ `persona/src/command.rs:31` **逐字**：「（`import_card_for_session`），**绝不** `apply_settings`」；两条写路径**分开**：导入到会话 → `:114 set_owned(SESSION_PROMPT_OWNER_PERSONA, session, …)`（**只写该会话槽**）· 还原基线 → `:238 apply_settings.apply({persona:{system_prompt: base}})`（**全局写但写的是启用前基线**，失败不静默 :242）
- ⇒ **A 会话的卡不会被写进全局 prompt**；还原失败有明确文案；`lib.rs:41` 记「没接管就不记基线」
- ⇒ **三段闭环**：写入只落会话槽（本批）· 存储按会话键**类型级**隔离（B0083）· 组合顺序**单一真源**（B0081/B0093）⇒ **本审计唯一一条从 Mod 写入侧一路核到宿主合成侧的完整红线链**
- ⚠ **第二次修正 B0104 的扫描口径**（规则 3 变体 2/7 第三形态）：我的模式列表**漏了 `fs::read`**，而 `persona/src/lib.rs:606` 确实 `std::fs::read(&path)`（读自己的导入卡，有 `MAX_CARD_FILE_BYTES` 上限）⇒ 结论**仍成立**但**「全量扫描」的覆盖不完整**。补：**grep 做全域扫描前，模式列表必须按 API 族枚举**

### BATCH-0110（1 文件定点 + 4 族全域扫描 · **0 新发现** · 能力台账）
- **env 4 族零命中**（唯一一条是 `external-input:632` 的注释「**不**用 `std::env::set_var`」）⇒ 无 Mod 读进程环境 ⇒ 密钥只能经 `secrets::lookup`（**这次有穷举证据**）
- ⭐ **5 个在册 Mod 中只有 `voice-input` 有进程执行能力**（`commands.rs:292 Command::new`）；`local-llm` 的同款**已废止未注册**，另两个 `TcpListener` 是测试 mock
- `voice-input` 的 spawn 安全选择**正确**：`:291`「**逐参数**，绝不拼 shell 字符串」（无 shell 注入面）· `:266` `is_file()` 校验 · `:294 stdout(null)/:295 stderr(piped)` · `:300-306` wait 放后台（「HTTP 线程绝不等 sidecar」）
- 输入来源**只有本机用户**（`sidecar_script` 经 loopback+Origin 校验的 Mod config 路由；无 Mod 能写 `mods.json`）⇒ **非远程提权面**，但属**应记账的能力不对称** ⇒ 已在账本立**能力台账**
- ⚠ **扫描口径第三种失效形态确认**：B0104 的模式列表**既无 `Command::new` 也无 `std::net`/`fs::read`** ⇒ 当时「进程执行」维度**整个空白**。结论不变但该维度当时无证据 ⇒ 再次验证 B0109 那条规则

### BATCH-0111（3 文件 · P0 0 P1 0 **P2 1** P3 0）
- ⭐ **F-0111-01（P2）**：`mod-voice-input/src/sidecar.rs:132-135` 把**明文 token 作为 argv** 传给 sidecar；`commands.rs:293` 逐参数 spawn ⇒ token 出现在 **`/proc/<pid>/cmdline`**，`hidepid` **默认 0** ⇒ **同机任何用户/进程可读**（窗口数秒）
- ⭐ **关键**：接收侧**已支持**更安全通道 —— `docs/examples/voice-sidecar/voice_sidecar.py:492` 有 `default=os.environ.get("VOICE_INPUT_TOKEN", "")`，而 Rust 侧**从不设该环境变量** ⇒ 修法**一行**（`cmd.env("VOICE_INPUT_TOKEN", …)` + 去掉 argv 的 `--token`）
- **分量在与本仓纪律不一致**：密钥不出现在 `GET /api/v1/env`(B0009)·日志 sink(B0017)·Mod `state_json`(B0105)·只经 `secrets::lookup`(B0004)——**argv 是唯一被漏掉的通道**；`grep "ps |进程表|可见|泄露"` **零命中** ⇒ 没有注释说明它被考虑过
- **非远程面**（B0110 已核：token 只能由本机用户经 loopback+Origin 的 Mod config 路由写入 `mods.json`）⇒ 定 P2（暴露窗口短、需同机其他进程、但修法一行且与既有纪律明显不一致）

### BATCH-0112（1 文件定点 · **0 新缺陷** · 扩写 F-0111-01）
- `sidecar.rs:36-39` → `sidecar_url_from_config` **原样取配置**，**无 `user:pass@` 剥离 / 无 URL 解析 / 无 scheme 校验** ⇒ 带凭据的 URL 落进**同一条 argv 通道**
- ⇒ **F-0111-01 扩写**（同根因合并、不新开条目）：修法扩成两处 env —— `cmd.env("VOICE_INPUT_TOKEN", …)` + `cmd.env("VOICE_INPUT_URL", …)`（接收侧 `:490` **已支持** `VOICE_INPUT_URL`）
- 顺带：`sidecar_python`（:52-59）是**第二条**用户可控可执行路径（=`argv[0]`），与 B0110 的 `sidecar_script`（=`argv[1]`）并列，均只做 `is_file()`、**无白名单** ⇒ 计入能力台账，**不记发现**（设计用途 + 仅本机可配 + 无远程面），但使 F-0111-01 后果**加重**：argv 里可能同时出现「可执行路径 + 带凭据 URL + 明文 token」

### BATCH-0113（2 文件定点 · **0 新发现** · memory 的注入预算）
- `memory/src/config.rs:61-68` → 摘要（调 LLM 的那条路）**缺省 false、显式开启**，base/model **空即不摘要**，`summary_api_key_env` 只存**变量名**；`:60` 注明**独立于主链 `[llm]`** ⇒ **「默认关 + 键名」纪律的第 4 个一致样本**
- `:47-59` 注入预算是**三层**：`injection_budget_chars` 上限 + `trim_ratio`(0.60) 按分数裁 top-k + `summary_ratio`(0.75) 触发真摘要；另有 `summary_cooldown_turns` 防抖 · `summary_keep_recent_turns` 保最近原文 · `max_records` **写入时物理淘汰** · `enabled_injection` 显式开关
- `assistant.rs:22` **逐字**：「无论哪种，**都不注入**、不写全局 persona.system_prompt」⇒ **注入面只有用户侧**（助手回复只入库）⇒ 提示注入的**来源面小得多**；`:47-51` 存储缺失 ⇒ warn + 跳过，不崩
- ⚠ **grep 精度教训**：搜 `INJECTED|注入` 时命中的其实是「**不**注入」的子串 ⇒ **中文注释里搜关键词要加上下文或改搜代码标识符**（本次因读了原文而未致错）

### BATCH-0114（1 文件定点 · P0 0 P1 0 **P2 1** P3 0）
- ⭐ **F-0114-01（P2）** `memory/src/strategy.rs`：**伪造 marker ⇒ 记忆块污染基线并逐轮累积**。四步全核：`:478` 拼装 `base+BEGIN\n{body}\nEND` · `:384` 空白折叠**不中和** marker（marker 只含单空格）· `:370` 剥离取「BEGIN 后**第一个** END」故提前收口 · 残留里已无 BEGIN ⇒ `while` 退出 ⇒ **残片永久留在基线**，下一轮再拼一块 ⇒ 两个 END + 旧残片常驻
- ⇒ 用户侧表现「模型复读我以前说过的话，**关掉记忆 Mod 也不掉**」（`lib.rs:491/744` 同按 marker 剥）
- **定 P2**：需用户/角色卡文本**逐字含该 marker**（低概率、无远程路径），后果**持久**；非 P1（无安全影响、无外泄）
- **与 B0109 同源**：`:364-366` 已正确处理「有 BEGIN 无 END 的半截块」，**没考虑「多一个 END」** ⇒ 两半都对、接缝无人管。**修法**：`sanitize_memory_line` 中和 marker（一行）/ `strip` 取最后一个 END 或计数配平 / 补回归

### BATCH-0115（1 文件定点 · **0 新发现** · 证伪「第二条 marker 注入面」）
- 假设「角色卡不经 `sanitize_memory_line` ⇒ 是 F-0114-01 的第二条注入面」**证伪**：`compose_system_prompt`(:285-306) 确实**原样**拼接（不过清洗），但它位于**记忆块之前** ⇒ 伪造 END 在 `BEGIN` **之前** ⇒ `strip_memory_block` 取「BEGIN 后第一个 END」**不受影响** ⇒ F-0114-01 范围维持「记忆记录」一条
- ⭐ 顺带**分清两种 Mod 内容的形态差异（这是对的）**：`persona` 角色卡**原样无标记**（它**按设计就是指令**）· `memory` 召回记录**有 BEGIN…END**（它是**数据**，需与指令区分）⇒ 形态不同不是遗漏；且**只有「数据」那条路径才有「必须被标记」的约束** ⇒ 这正是 F-0114-01 只可能发生在记忆侧的原因

### BATCH-0116（1 文件定点 · **0 新发现** · Mod 根收尾）
- `persona/src/lib.rs:175-178` → `DISCIPLINE_TEMPLATE` **不含任何 marker** ⇒ 合成提示词的三个来源（角色卡原文 / 记忆标记块 / 纪律模板）中**只有记忆那条需防 marker 伪造**，与 F-0114-01 的范围判断一致 ⇒ **B0115 的未核实点关闭**
- ⭐ 记一处**跨层承重设计**：`DISCIPLINE_TEMPLATE:177`「**每句话以真实句读结尾（。！？）**，不要用逗号把多层意思串成长句」= AGENTS「**一句一单元**」红线在**提示词侧**的落点；而装配器侧（B0005/0006）**只按真实句读切分、不许硬切** ⇒ **两层必须同时成立**：模型若不遵守而用逗号长句，用户听到的是**无法断开的长句**（而这不是装配器的 bug）

### BATCH-0117（2 文件定点 · **0 新发现** · ⭐ Mod 根结项）
- `strategy.rs:52-53` `MIN/MAX_MAX_RECORDS = 1 / 10_000` ⇒ `append_capped:103` 的 `max_records==0`（不限）分支**从配置路径不可达** ⇒ **「文件无限增长」被钳位挡在门外**（正面）
- 代价备忘：`append_capped` 是 **append → load(整份) → rewrite(整份)** ⇒ 上限值**同时是每轮 I/O 预算**（10k × 200B ≈ 2MB ⇒ 单轮约 4MB 读写）。**不记发现**（有界 + 可调 + 原子写必需 + 有回归钉住「保留最新」），但记此备忘以便日后调 `MAX_MAX_RECORDS` 时知道它的第二重含义
- ⭐ **Mod 根结项小结（B0100–B0117，18 批）**：**2 条 P2，且两条都在「Mod ↔ 宿主」的边界上**（进程边界 argv / 标记边界 marker），**Mod 内部逻辑 0 缺陷**；另有 2 次自我更正与密集的正面样本

### 第十二次对账（CONSOLIDATION-12）· ⚠ 账本自身四处分叉 + 元结论形式化
- **11 条 P1/P2 复核：11/11 仍成立**（含新增的 F-0111-01 / F-0114-01）
- ⚠ **本轮最重要的产出是审计自己账本的四处分叉，已全修**：① **F-0062-01/F-0111-01/F-0114-01 只有散文、没有 `### F-` 规范头** ⇒ **三条真实发现对机械计数不可见** ② F-0034-01 头仍写 P3（B0087 已升 P2）③ F-0030-01 已撤回但头未标 ④ 我在 B0090 立了「计数必须机械算」却随后又手改两次
- ⇒ **权威口径**：标题 69 − 已撤回 1 = **有效 68 条**（P1 9 / P2 32 / P3 27）
- ⭐ **新立模式 Q**：「**追加式记录会让规范头与实际状态分叉**」⇒ **规则：改定级/存废必须同时改规范头**，勘误正文只写「为什么变」
- ⭐ **元结论形式化（须成对读）**：**新代码**（Mod 根 18 批 / l2d / core / preset）**内部零缺陷、缺陷在接缝**；**老代码**（`web_api` 7 条 P1）**全在内部、无一涉及跨组件边界** ⇒ **审计手法要按层年代选**：新代码查接缝，老代码查内部，反用两边都低产出
- ⭐ **正面模式 P3 正式立档**：5 样本，最强一档 `DisabledStaging` **没有 HTTP 字段** ⇒ 「默认关」不需任何分支保证（**建模优于加判断**）
- 覆盖率（220 条去重路径）：Rust `.rs` 69 · 前端 `.dart` **35/217** · Mod crates **8/61**（本 18 批是**边界扫描 + 定点**，**不是逐文件通读** —— 不把「结项」说成「审完」）

### BATCH-0119（1 文件定点 · **0 新发现** · ⭐ 换根 + 换手法）
- **换根到 `live2d-ai-runtime`（老代码区）**，按 CONSOLIDATION-12 §③ 判据改用「**查内部**」的手法；`settings.rs`(826) 是该区最大且内部未审文件，且 AGENTS.md 记着一条真实事故（保存设置抹掉全部注释 → 改用 `toml_edit`）
- ① 写盘点**收进一处** `to_toml_string_merging`(:751)，头注点名系统性风险：「避免三个写盘点各写一遍（**漏一个就又是一次注释清空**）」⇒ 结构性收口，非顺手改
- ② 那个「重生成整份文档」的 `to_toml_string` 仍在，但**穷举 3 个生产调用点**后确认：:734 空文本时合理 · :740 仅作 `DocumentMut` **中间表示不落盘** · :754 是唯一可能写盘的兜底
- ③ ⭐ **兜底不可达** ⇒ 不会毁掉用户文件：`load_from_path`(:571-578) **读失败与解析失败都返 Err、不回退缺省** ⇒ 从磁盘出发的写路径**必然先成功解析过** ⇒ `merge_into_toml` 的解析错误分支到不了。**不记发现**，但**记备忘**：这条安全性**依赖 `load_from_path` 的严格性** —— 若将来给它加「解析失败回落缺省」，`:754` **立刻变成静默覆盖用户配置的路径**
- ④ `sync_item`(:760-762) 对 `toml_edit` 语义理解准确：「键自身的前缀注释挂在 key 上，**换键会把它们一起换掉**；就地改值只动值」

### BATCH-0120（3 文件 · **0 新发现** · B0119 开放点关闭）
- ⭐ **「三个写盘点收在一处」核实为真且恰好三个**（穷举 `to_toml_string_merging`）：`settings_routes/mod.rs:203`（HTTP 设置）· `cli_entry.rs:225`（**Mod apply_settings**）· `app/settings_ui.rs:379`（egui **休眠壳**）⇒ 与 `settings.rs:747-750` 头注逐一对应
- 另穷举 desktop+runtime 全部 `fs::write`/`File::create`：命中项是 benchmark JSON、**测试**、models registry 自有文件 ⇒ **无一处直接写 `live2d-ai.toml`**；写盘为 tmp+rename 原子
- `secrets.rs:167-175` → `.env` 侧**同一条纪律**并点名同一失败模式（「整份重写会把这些抹掉——**用户不会立刻发现**，等发现时已经找不到原来的注释了」）；`merge_env_line` 为**纯函数**
- ⭐ **本审计见过最细的一处安全细节**（正面）：`secrets.rs:211-214`「先收紧 **tmp** 的权限再 rename：否则会有一段『**文件已是新密钥、权限还是 umask 默认（通常 0644）』的窗口**。rename 之后收紧就晚了」⇒ **亚秒级世界可读的密钥窗口已被关闭**
- ⇒ 该细节同时**佐证 F-0111-01 的分量**：「同机可读」这个威胁模型本仓**是认识的**（这里想到了并修掉），**只是没覆盖到进程命令行那条通道** ⇒ 不是没人想到，是**覆盖不全**

### BATCH-0121（3 文件 · **P3 1**）
- ⭐ **F-0121-01（P3）**：`plan_atomic_write`（patch.rs:738-771，三处 toml 写盘点 + `.env` **共用**）用 `File::create(&tmp)`（tmp 权限 `0666 & ~umask` 通常 0644）、文档只讲内容与崩溃语义、**不负责 rename**（:770 只返回 tmp 路径）⇒ **「谁该 chmod tmp」成了调用方各自的知识**
- **两个调用方做法不同而各自都对**：`.env`（含密钥明文）**必须**先 chmod（`secrets.rs:211-214`）· `live2d-ai.toml`（**只持变量名**）**不需要**（`settings_routes/mod.rs:217-218`）⇒ **不对称按内容正确划分，不记为缺陷**；缺的是**契约表述**
- 建议：文档补「本函数不设权限；写敏感内容时调用方须先 chmod tmp 再 rename」，或加 `mode: Option<u32>` 让「要不要 0600」成为**调用点必须回答的问题**
- 未核实（如实标注）：是否有代码在别处 chmod 过 toml（若用户手动收紧 0600，首次保存会被放宽回 0644 —— **长期状态**问题而非窗口问题）

### BATCH-0122（1 文件定点 · **0 新发现** · 老代码区查内部）
- `patch.rs:693-700` → 校验在**合并之后**、对**最终状态**做 ⇒ 即使当前值本身非法而 patch 没碰它，**也会被拦下**（增量补丁最易写反的顺序，此处正确）；`validate_base_url_strict`(:710-717) 要求绝对 http/https，错误带段名+原值
- `:414-438` → 三态**逐字段穷举**（整段清空时 5 个字段逐个判、逐个可能置 `changed`）⇒ `:702-708` 的 `Updated`/`NoChange` **准确**（不是「收到请求就算变更」）
- ⭐ **对 F-0121-01 的自洽校验**：`plan_atomic_write` 的文档(:719-737)其实是**五步完整契约** + 崩溃语义 + PID 并发 + 「与旧实现的差异」清单 ⇒ 我的指控**精确**：不是「文档缺失」，而是**在高质量契约里恰好缺「文件权限」一项**，而那一项**恰好被一个调用方需要**
- ⇒ 再次印证 CONSOLIDATION-12 §③：**老代码的缺陷多是「局部具体疏漏」而非「整体缺失」**，与 Mod 根的「接缝问题」是两种形态

### BATCH-0123（3 目标 · **0 新发现** · ⭐ 红线 R 序列化面闭环）
- **机械差集**（settings 各段 `pub` 字段 vs view 各 View 段）：Llm `api_key_env`→`has_api_key` · Tts `api_key_env`+`response_format`→`has_api_key` · Performance `api_key_env`→`has_api_key`+`wired` · Persona/Action **完全对等**
- ⇒ **四处 `api_key_env` 全部被 `has_api_key: bool` 取代，一处没漏**；且这比 B0009「不回值」**更进一步**：**连环境变量名都不出门**
- `response_format` 唯一需追问 ⇒ **刻意取舍**且两侧都写明（`tts_section.dart:7`「不在 view 也不在 patch 里 → UI 上不存在这个控件」；背景：本机 TTS 实测只有 `pcm` 是 200）
- ⇒ **红线 R 序列化面闭环**：`GET /api/v1/settings` 的脱敏视图与真实设置的字段差集**全是「敏感 → 派生布尔」的替换**，**无任何字段未经脱敏出门**

### BATCH-0124（2 文件 · **0 新发现** · F-0002-01 影响面确认）
- `secrets.rs:118-125` → 快照刷新语义正确：**缺文件 ⇒ 空表**（不是错）· 真错 ⇒ 上报并带路径 · 整体替换；`snapshot()` 首用惰性落盘读；`lookup` 契约写在定义处（`.env` 快照 > 进程环境 > `None`）⇒ **红线 R 读取面成立**
- ⭐ `cli_entry.rs:334-345` → **`.env` 确实在 watcher 的反应里**（:345 `secrets::refresh_from_disk()`）⇒ AGENTS.md 的宣称为真；而 **F-0002-01 的缺口正落在这条 `if let Some(sup) = &supervisor_opt` 上**（首次运行为 `None` ⇒ watcher 不存在）⇒ **该 P1 的影响面从「一个文件」确认为「两个文件」**（`live2d-ai.toml` + `.env`，两条都是外部热修的关键入口）
- 备忘（不可达）：`:123` `if let Ok(mut g) = SNAPSHOT.write()` 忽略写锁结果；中毒在本处不可达（临界区只有一次 move）⇒ 不记发现，但本仓已因锁中毒付过代价（F-0006-03）

### BATCH-0125（1 文件定点 · **P3 1**）
- ⭐ **F-0125-01（P3）** `secrets.rs:74-76 / :87-89`：**键写错被显式挡住**（头注 :58-59「宁可少一个变量，也不要让一行手写错的内容变成**半截 key**」⇒ 哲学正确），**但值写错没有同等对待** —— `unquote` 只在**首尾同种引号**时成对去掉 ⇒ 手写 `KEY="sk-abc`（漏收尾引号）得到值 `"sk-abc`，**引号成为密钥的一部分**
- ⇒ 后果链：密钥**实际是错的** ⇒ 全程 401；而 UI 只显示 `has_api_key` **布尔**（只是「是否查到值」）⇒ **用户看到「已设置」却全程 401**，无任何提示。**冲突点精确：同一条注释里的哲学只应用到键，没应用到值**
- 定 P3：手写漏引号（低概率、无远程面），后果是持续 401 + 可诊断性差，不丢数据不外泄。**与 F-0098-01 相反**：那条是「文档说静默而实现不静默」（安全方向），这条是「**文档已说清规则、而规则本身漏了一种形态**」
- 顺带正面：**CRLF 正确处理**（`str::lines()` 去掉 `\r` ⇒ Windows 保存的 `.env` 正常，否则全线 401；**头注未写**但属安全方向的未文档化）· 值的注释语义与头注一致（只跳整行注释、不做行尾 `#` 截断）

### BATCH-0126（1 文件 · **0 新发现** · 模式 H 第 9 次复查）
- `secrets.rs` 10 条测试**逐条对应一个行为**：成对引号+注释+`export` · 后覆盖前 · 键名合法性 · **就地改值保留注释**（= AGENTS 那次事故的回归）· 保留 `export` 前缀与缩进 · 空值只删该键 · **引号内含空格或 `#`** · **原子+属主 0600** · 拒坏键名与多行值
- ⭐ `write_key_to_disk_is_atomic_and_owner_only` **把 B0120 那处最细的安全细节钉住了** ⇒ 「先 chmod tmp 再 rename」不只写进注释，**还有测试在盯属主权限**（本审计见到的「安全细节 + 回归」配套最完整一处）
- ⇒ **唯一的测试空缺恰好就是 F-0125-01 那一种形态**（成对引号有测、**不配对引号无测**）⇒ **实现的空缺与测试的空缺是同一个形态** ⇒ 补测试时顺带补实现，成本一条断言
- 另一处未钉住：**CRLF**（行为正确但无测试 ⇒ Windows 用户存 CRLF 时的「全线 401」无回归保护）

### BATCH-0127（1 文件 · **P2 1**）
- ⭐ **F-0127-01（P2）** `performance/plan.rs`：**同一个解析函数里三种上限、两种处置** —— `speak` > 4,000 字符 ⇒ `chars().take()` **静默截断（从中间切）**（`:514`）· `cues` > 16 ⇒ `Err(TooManyCues)`（`:521-522`）· `segments` > 64 ⇒ 报错+消息（`:592-594`）⇒ **该文件自己的约定是「超限就告诉调用方」，`speak` 是唯一例外**
- 且 `parse_plan` 返回 `Result<Plan, PlanError>` **没有告警通道**（对比 memory Mod 的 `warnings`，B0102）⇒ 这种截断**结构上不可能**被上报
- **与 F-0020-01（P1）的关系**：那是 `MAX_TOKENS` 从请求侧截断**整个 plan**，这里是**字段级**的第二次截断，**独立存在**，两者叠加症状相同（半句 / 一个字都不上屏）
- 定 P2：默认配置下不易触发（需很大 `max_tokens` 或手写 plan），但一旦触发就是**静默的半句话**。建议：与邻居一致地 `Err`；或至少给调用方「被截断」信号，且不要从中间切

### BATCH-0128（1 文件 · **0 新发现** · 收窄 F-0127-01 + 模式 Q 复发）
- ⭐ **F-0127-01 由 P2 收窄为 P3**：`json_schema_strict`(plan.rs:803-806) 写明 `speak` 是「**已弃用（V11 保留）：与 segments 同现时被忽略**」，而 `required` 只有 `segments`+`cues`(:793) ⇒ **模型按 schema 产出时根本走不到那个截断**；且同现时是 **`SpeakIgnored` + warn**（:307/:318），**有提示不静默** ⇒ **真实范围只是弃用兼容路径**（`speak` 存在而 `segments` 缺席）
- 正面/附带：schema 给 `segments` 写了机器可校验的 `maxItems`，而**字符上限只写在 `description` 里**（:799「总字符不超过 {MAX_SEGMENT_CHARS}」）⇒ 字符预算在 schema 层是「提示」而非「约束」；但解析器的 `segments.concat() == source` 不变式（:610-613）会**整条拒掉**不合法的分段
- ⚠ **模式 Q 隔一批就复发**：`grep "^### F-0127-01"` **零命中** —— B0127 又只写在散文里。而 INDEX/STATE 摘要已声称「F-0127-01（P2）」⇒ **账本对外有、账本内部找不到**。已补规范头（P3 + 复发说明写在头下）
- ⇒ **教训比规则更重要：立了规则不等于会执行**。模式 Q 需要**机械检查**：每批收尾跑「INDEX/STATE 提到的每个 `F-xxxx-xx` 都能在 FINDINGS 里 grep 到 `^### F-xxxx-xx`」

### BATCH-0129（账本完整性 · ⭐⭐ 模式 Q 检查**首次运行**即抓出两条真实丢失）
- ⭐⭐ 检查口径：**INDEX/STATE/NEXT 里出现的每个 `F-id`，都必须在 FINDINGS 有 `^### F-id` 规范头**（双向核对，能抓「正文丢失」）
- 抓出 **F-0044-01/02**：被 INDEX **对外引用**、而 FINDINGS 里**根本没有 BATCH-0044 段**（那批只写了 BATCH/INDEX/STATE）⇒ **约 85 批**的账本缺口
- 抓出 **F-0125-01**：B0125 段存在，但**段标题里的 F-id 前缀在写入时丢失** ⇒ 正文在、标识不在
- ⇒ 两者都比 CONSOLIDATION-12 §② 那类**更严重**（那类至少有正文、只是没头）。已**回填**并在段顶显式标注「这是回填」
- 检查本身两处改进：① 加词边界（初版把 `HANDOFF-2026-09-28` 的 `OF-2026-09` 误判成一条发现）② 从「查缺头」升级为「**对外引用 ↔ 规范头**」双向核对
- 修正后权威口径：**规范头 73 − 已撤回 1 = 有效 72 条**（P1 9 / P2 32 / P3 31）· 模式 Q 检查 ✔ 通过
- ⭐ **教训**：「我给账本立的每条规则，都必须配一个**会失败的机械检查**，否则它只是一句我下一批就会忘的注释」——证据就是这条检查**第一次运行**就抓出两条真实丢失、其中一条已存在 85 批

### BATCH-0130（1 文件 / 定点读 · **0 新发现** · 模式 Q 检查通过）
- `llm.rs:400-421` → 「请求体不含 tools」以**最难形式**钉住：断言**键不存在**（非值为空）· **覆盖两条序列化分支**（`max_tokens` 0/512）· **顺带锁 `tool_choice` 别名** ⇒ rc.1 工具层拆除**不可静默回潮**
- `:126-138` → `max_tokens: 0` 实现为**整个字段省略**（`skip_serializing_if`）而非发 0 ⇒ **跨服务端互操作风险被想到**
- `:128-130` → **跨层设计的第三条腿**：协议层硬上限 + 我们按 `。！？` 主动分句「两者配合才是完整的长度控制」⇒ 与 B0116 的 `DISCIPLINE_TEMPLATE`（提示侧）、装配器侧「不许硬切」**三腿互相引用**
- ⭐ **对 F-0020-01 措辞的收紧依据**（不改定级）：**机制本身是对的**（字段省略语义、互操作、与分句器配合都考虑到了）⇒ 缺陷是「**一个常量的值不对**」，修法是**改一个常量**、**不应改机制**
- 【模式 Q 双向检查】**✔ 通过**（B0130 收尾跑）

### B0130 补记 · 模式 Q 检查的第二次改进
- 检查须扣除 **`KNOWN-LOSSES.md`** 里显式记录的**已知丢失** ⇒ **已知的丢失要显式记录，不能靠正则藏掉**
- 首次按此口径运行时抓到的 `F-0044-01` 是**假阳性**：它只出现在 B0129 的**缺陷描述文字**里；该条**正文在任何文件都不存在**（`BATCH-0044.md` 里也没有）
- ⇒ 已建 `AUDIT-REPO/KNOWN-LOSSES.md`（KL-1），**F-0044-01 不计入有效发现数**；若日后有人能提供 B0044 原始上下文，应恢复正文并从该表删除

### BATCH-0131（1 文件 / 结构+测试名枚举 · **0 新发现** · 模式 H 第 10 次零命中）
- `sse.rs:28-77` → 增量解码器把**难形态当设计输入**：字节任意切分（`push` 只追加、`next_event` 只在空行边界返回）· 尾部无空行（`finish` 兜底）· **悬挂 CR 显式 pop** · BOM 状态位 · 多行 `data` 以 `\n` 连接
- 8 条测试**全部按「线上字节形态」命名**：`decode_byte_by_byte`（逐字节）· `crlf_and_cr_terminators_mix_freely` · `crlf_split_across_pushes_is_not_two_lines`（**CRLF 切在两次 push 之间**）· `field_value_strips_single_leading_space_only`（只脱一个空格）· `empty_events_are_skipped_without_reseting_pending_state`
- ⇒ **测试名就是它在线上要抵御的那种字节形态**；与 B0095（`guess_format` 钉住「6 字节但魔数合法」）同属本审计**最硬的两个解析器回归**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0132（1 文件 / 定点读 · **0 新发现** · 第三处跨语言对齐）
- `worker.rs:96-128` → 空句分支是**教科书级修复**：注释把**机制**（`is_terminator` 含 `\n` ⇒ 切出纯空白句，它 `is_empty()==false` **躲过非空守卫**）· **后果**（上游 `400 input 为空` + TTS 错误 **fatal** ⇒ **整轮**判失败、用户看到「说了半句就没了」）· **取舍**（不发 doomed 请求，也**不让分句器丢字符**）全写齐
- ⇒ 实现为**发一个空 `AudioChunk` 且 `first_chunk`+`final_chunk` 都为 true**（:115-117）⇒ 边界信号照发、前端分句框不卡住。**守住了 `segments.concat() == source` 那条硬不变量，绕开了问题**
- 另两处：`:130-135` 格式门禁失败 ⇒ `ErrorKind::Decode` **明确报错**（不静默降级）· `:137-143` `select!` **取消优先**（停止不被在途 TTS 拖住）
- ⭐ **第三处跨语言对齐**：「空末块也要发边界帧（`audio: ""` + `end: true`）」这条规则在**线上帧侧**（B0006）与**引擎侧**（`worker.rs:113-117`）是同一条规则的两种实现 ⇒ 连同 B0116 的提示侧与协议侧，「一句一单元」现有**四处对齐**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0133（2 文件 / 定点读 · **0 新发现** · 「去掉修复即红」经核实为真）
- `supervisor/turn.rs:316-344` → 残余事件排空在**正确位置**、**有界**（`for _ in 0..256`）· **epoch 过滤**（:329，与 B0040 核过的闸一致）· **只有一条处理路径**（:330 与循环内**同一个** `handle_engine_event`，:326「顺序不变、语义不变」）⇒ 正是**模式 C/接缝缺陷的正解**
- `:353-354` **停止分支**改调 `drain_residual_events`（丢弃而非处理）⇒ 策略按场景分：正常结束补处理 · 用户停止丢弃（否则会继续上屏整轮正文）——**分叉是对的**
- ⭐ 回归 `tests_loop.rs:770 failed_turn_delivers_generated_text_via_text_fallback` **复现的正是那个竞态**：LLM mock 返回**真实正文** + TTS 指向 `http://127.0.0.1:9/v1`（discard 端口）⇒ **传输层**失败（注释注明「与上游 5xx 是不同路径」）；断言落在**可观察后果**（正文必须经 `TextFallback` 到 UI）
- ⇒ **去掉排空循环 ⇒ 该测试变红** ⇒ AGENTS.md 的「去掉修复即红」**为真**。**第 3 个「真机抓到 → 结构性修法 → 复现竞态的回归」完整闭环**（前两个：WASM `stage_css` 长手属性、Mod `secrets` chmod-before-rename）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0134（2 文件 / 定点读 · **0 新发现** · 一处「危险用法被函数自己挡住」）
- ⭐ `supervisor/handlers.rs:351-361` `drain_residual_events` 把「**只用于 stop/cancel**；**正常收尾路径上不能丢**」写在**函数自己身上**，并指向 `turn.rs` 的排空循环 ⇒ 一个「A 上下文对、B 上下文危险」的函数**危险用法被函数自己挡住**（命名 `drain_`=丢弃 + 体内警告）⇒ **本审计最干净的一处 API 约束写法**
- 与 B0133 核到的**两个调用点**正好对上：`turn.rs:327-344`（正常收尾 ⇒ 不用它）· `turn.rs:353-354`（停止 ⇒ 正是用它）
- `engine.rs:170-171` → `cancel.child_token()`：**取消令牌按作用域派生**（致命错误只中止自己的 TTS worker，「**而不触碰调用方的令牌语义**」）⇒ 作用域最小化且理由写明
- `:178-179` → `tts_queue_capacity.max(1)` / `audio_chunk_samples.max(1)` ⇒ **容量在使用点夹紧**（配置写 0 也不会造出零容量通道）⇒ 与 B0122「合并后再校验」、`plan.rs` 钳位**同形状：不指望上游给合法值**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0135（2 文件 / 机械枚举 · **0 新发现** · 9 变体 ↔ 9 臂、零兜底）
- `EngineEvent` **9 个变体** ↔ `handle_engine_event` **9 个处理臂**（各恰好一次）⇒ **零缺口、零 `_ =>` 兜底**
- ⇒ 两个推论，第二个更重要：① 没有任何变体被静默丢弃；② **新增变体会编译失败** ⇒ 「加了事件类型、supervisor 忘了处理」**在结构上不可能发生**（不靠测试、不靠纪律，靠类型系统）
- ⇒ **比 `DisabledStaging`(B0106) 与 `FieldRuntime::frame`(B0097) 更强一层**：那两处保护**当前状态**，这一处保护**未来的改动** ⇒ 正面模式 P3「建模优于加判断」的最强样本
- 9 变体各自对应的红线已连成表：`TextDelta`/`ReasoningDelta` 思考隔离(B0009) · `AudioChunk` 空末块边界帧(B0006/0132) · `SentenceReady`/`Voiced` 一句一单元(B0006/0116) · `TextFallback` 残余不许丢(B0133) · `ActionCue`(B0015) · `Error`/`Terminal` 致命分类(B0017)
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0136（3 文件 / 机械枚举 · **0 新发现** · 事件链端到端画完）
- `ConversationUiEvent`（app_event.rs:156）**7 变体** ↔ `ws/events.rs` **7 条投影**（各恰好 1 条）⇒ **零缺口、无死变体**
- 9→7 的差额**不是丢失**：`AudioChunk` 走 `ws/audio.rs` + `broadcaster.rs` **独立通道**，契约写在 `audio.rs:40-49`（`first/final_chunk` **由引擎给出**，「一句音频切成十几块，只有引擎知道哪块」）
- ⭐ 顺带核到**句界红线的历史缺陷记录**（`broadcaster.rs:169-173`，2026-09-11 修）：曾用 epoch 记账把「一轮语音的首片」当句界 ⇒「**永远推不出句界**」⇒「**本方法不再做任何推断**」⇒ 该规则是**事故驱动**的
- ⇒ **完整事件链地图**：`EngineEvent`(9) → `handle_engine_event`(9 臂，**无兜底**) → `ConversationUiEvent`(7) → `events.rs`(7 投影) ／ `AudioChunk` → `audio.rs`+`broadcaster.rs`（独立通道、不推断）⇒ **四个环节全部有机械完整性证据（9/9、9/9、7/7、7/7）**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0137（1 文件 / 定点读 · **0 新发现** · 「薄」有据）
- `TurnReport` **只有 2 个字段**，而 :392 写明「**轻量汇总；全部细节都经事件通道传递**」⇒ 「薄」是**写明的设计**，与 B0015 核过的 `ErrorKind::code()` → `AppEvent::Error` → WS `error` 帧是**同一套两通道设计**（结果在返回值、细节在事件）
- `TurnStatus` 是 **3 变体枚举**（非 bool），每个变体都写明**细节去哪**：`Completed`（途中可恢复错误已由事件上报）· `Failed`（细节经**先行** `Error` 事件上报）· `Cancelled`（终态 `Terminal{Cancelled}`，**历史不提交**）
- ⭐ 记一条**设计演进**（`Failed` 文档逐字）：「**不再用 Done 隐示成败**」⇒ 过去「失败」是用「**不发 Done**」隐示的，现改为**显式变体** ⇒ 与本仓反复出现的纪律一致：**用显式信号，不用「本该出现却没出现」来推断**（句界/dirty/结局三处同形）

### BATCH-0138（1 文件 / 定点读 · **0 新发现** · D8 语义逐字对齐）
- `engine.rs:256-257` → 流中途出错：**逐字**「已完整切句并入队的内容**允许播完**；**未封口残余不再 TTS**。**不取消 worker**」⇒ 发 `Error` 事件（WS 带码，B0015 核过同源）⇒ 与 D8 裁决逐字一致
- `:246` → 「**流耗尽」与「流出错」被两个独立标志区分**（`llm_finished` / `llm_failed`）⇒ 结局判定不互相污染；注释还记了**为什么 `None` 意味着耗尽**（`try_unfold` 的易误解点）
- 所有发送都走 **`send_event(&event_tx, &cancel, …)`**（取消感知）⇒ 通道满 + 无接收者时**不会把 turn 卡死** ⇒ 与 B0134 的 `tts_queue_capacity.max(1)` **同族防御**（不让「容量/阻塞」变成隐性死锁）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0139（2 文件 / 定点读 · **0 新发现** · 一条跨层疑问如实标为未核实）
- `engine.rs:406-412` → 表演层入口是 **6 路门控**（`perf` / `!cancelled` / `!tx_closed` / `!llm_failed` / `!enqueue_failed` / `!queue_dead`），注释**点名它与阶段 4 的 `TextFallback` 互补**（「失败轮那一轮不发 TTS」）⇒ **无「失败轮两头都不管」的洞**
- `:545` → 结局归类第三臂 **`debug_assert!(llm_finished, "非取消/非致命路径必须来自正常流耗尽")`** ⇒ 「**Completed ⇒ 流确实耗尽**」在 debug 构建被断言、release 里写进消息 ⇒ 三臂里唯一可能悄悄错的那一臂**不再沉默**
- ⚠ **跨层优先级未核实**：Rust **先判 `cancelled`**、Dart `settleTurn` **先判 `failed`** ⇒ 表面相反；但 ① `settleTurn` 第一条「有文字 ⇒ 一律 `keep`」使多数情形不落到那两个分支 ② **未能定位 Dart 侧 `failed` 的赋值处** ⇒ **按纪律不记发现**（B0140 第一件事）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0140（1 文件 / 定点读 · **0 新发现** · ⭐ B0139 疑问关闭）
- ⭐ **B0139 的跨层优先级疑问关闭**：`chat_controller.dart:429` `_finishTurn({bool failed = false, bool stopped = false})` 的 **4 个调用点各只设其中一个标志**（`:215` stopped · `:245` `completed == false` · `:260` `status == 'failed'` · `:327` 本地 true）
- 且 `:260` 的 `failed` **直接取自后端 `turn_state.status`** ⇒ 后端报 `cancelled` 时 `failed` 必为 `false` ⇒ **两个标志从不同时为真**
- ⇒ Rust「先判 `cancelled`」与 Dart「先判 `failed`」的**表面相反不可观察**；`settleTurn` 的顺序**从未被行使** ⇒ **不是分歧，是 tie 被设计成不可能**
- ⭐ 归入正面模式：**把 tie 设计成不可能，而不是靠测试保证**。分界线：两侧是否可能被**同一批输入同时触发** —— 不可能 ⇒ 设计赢，可能 ⇒ 接缝风险（对照 F-0034-01 / F-0062-01）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0141（1 文件 / 定点读 · **0 新发现** · 阶段 2 三要点）
- `engine.rs:523-534` → 「关队列 → 等排空 → 退出原因参与终态」三个要点全对：`drop(job_tx)` 后 worker 的 `for job in rx` **在缓冲排空后才结束** ⇒ **已入队句子照样播完**（与 B0138 的 D8 同一条决策、**两处都写明**）
- 「**无条件 join，杜绝泄漏**」⇒ `worker.await` 一定执行 ⇒ 不存在「轮次已收口、后台还在出声」的游离任务
- `:542 || worker_exit == WorkerExit::Failed` ⇒ **排空途中的 TTS/解码错误不被吞掉**，该轮仍判 `Failed` ⇒ 与「等完就当成功」这个坑恰好相反
- ⭐ 顺带澄清「排空」有**两个不同层**：本处 = **TTS 队列排空**（PCM 都交出去了）；`engine.rs:141` 提到的 = **真实播放排空**（即 B0040 核过的 core **双闩锁**「生成结束 ≠ turn 完成」）⇒ **两层都要有**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0142（1 文件 / 定点读 · **0 新发现** · 「权威是报告，不是事件」）
- `engine.rs:553-557` → `TextFallback` 只在 `Failed` **且**清洗后非空时发；`Cancelled` 的排除**写了理由**（「用户按了停止**不该再补一段文字**」）；兜底正文走**与健康轮相同的清洗器**（:554-555）⇒ 上屏口径跨成败路径**统一**
- ⭐ `:569-570` → 终态事件投递**有上限**（`TERMINAL_SEND_TIMEOUT`），而「**supervisor 的权威终态是 `TurnReport`，不依赖本事件送达**」⇒ **消费端卡死不会丢掉本轮结局**（B0137 两通道设计的**韧性侧**）
- 与 `:229-230` 的 `let _ = send_event` **同一纪律**：**事件发不出去是常态，不是灾难；权威在别处**
- ⇒ **`run_turn` 主体读完**，仅剩「表演层 snapshot 注入点」（:406-416）未读
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0143（1 文件 / 定点读 · **0 新发现** · ⭐ 最强的单处工程）
- `turn.rs:439-441` → 权威规则拆成**四条互斥规则**（声卡 fault/stall ⇒ Failed 无论引擎怎么说 · 仅用户取消 ⇒ Cancelled · 仅 LLM 中途失败 ⇒ 沿用引擎 · 真成功才 Completed）⇒ **没有「引擎说什么就是什么」的默认路径**
- ⭐ `:443-447` → 一个**带对抗性评审编号**（「复审 P0-2 场景 B」）的竞态：声卡 fault「**恰好卡在 `gen_fut` 完成的瞬间**，20ms tick 观察**尚未置位**就到 Stage C」⇒ 直接沿用引擎 status 会**错误定格为 Completed**，且「**Stage D 补发 Cleared 也无法纠正** root 的权威 outcome」（因为 Stage C 正是**落闩点**）⇒ 修法：**判定前同步健康检查一次**（「原子读，零成本」）
- ⇒ **与 B0040 核过的 core 双闩锁「恰一次」直接对上**（闩过就改不了）⇒ 归入**正面模式 P4：竞态要用对抗性评审编号记录，并在权威落闩前关闭**
- 顺带：`turn.rs:8` 记着一次**被放弃的重构**（「曾尝试把 Stage B `pump_pending` 与 …」）⇒ 与 B0039–B0042 核过的「记录推翻过的方案」同形
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0144（1 文件 / 定点读 · **0 新发现** · 同纪律第 4 例）
- ⭐ `turn.rs:377-384` → **`WouldBlock` 的剩余样本必须放回下一轮继续泵**，注释写明防的是「**尾段被静默截断**」⇒ **正面模式 P5「部分投递必须放回/补齐，不得静默截断」的第 4 例**（① B0132 空音频块带双边界 ② B0133 残余事件补处理 ③ B0136 句界由引擎给 ④ 本批 PCM 尾段放回）
- `:381-383` → 停滞判定粒度是「**是否有真实进展**」：`accepted_now > 0` ⇒ `WouldBlock` 但接受了若干样本**算有进展**（重置 `stall_ticks`）；真零进展 250×20ms=5s ⇒ `AudioStalled` 致命 ⇒ **部分成功不算卡死**
- `:389-391` `select! { biased; control_rx.recv() … }` ⇒ **停止/退出/重载都第一顺位**（与 B0138「`select!` 里 `cancel` 先判」同纪律）；`:400-401` 重载**丢弃、留在 idle 重建** ⇒ **不拽在飞的 turn**
- ⇒ turn 管线**仅剩阶段 A（:200-360）**未读
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0145（1 文件 / 定点读 · **0 新发现** · ⭐ turn 管线 100% 读完）
- ⭐ `turn.rs:222-223` → 控制通道断开时**不丢弃生成 future**，而是「**摘臂后继续轮询 `gen_fut` 至自然返回，绝不 abandon（D2 契约）**」⇒ 引擎自己的收尾（`worker.await` 无条件 join）**才真的会执行**
- ⇒ **资源生命周期三处一致**（正面模式 **P6**）：① B0141 `worker.await` 杜绝泄漏 ② 本处 `gen_fut` 绝不 abandon ③ B0133 残余事件排空而非丢弃 ⇒ **每个 spawn 的 future / 每条打开的通道都被驱动到确定终态**
- `:208-213` → 两处 `#[allow(unused_assignments)]` **都写了「静态分析看不见什么」**（「跨 select 静态不可见」「跨 select 迭代，静态分析不可见」）⇒ 压制 lint 时说明**为什么分析器看不到**
- `:240-244` → `biased` 的**饿死风险被显式处理**（`finish_closed = true, // 摘臂防 biased 饿死`）
- ⇒ ⭐ **里程碑：turn 管线 100% 读过**（B0132–B0145，**14 批 0 条缺陷**）⇒ 质量**高于本审计任何一区**；而它的**四次事故**（rc.3 N0 / B3-P0-1 / B3-P0-2 / B4-P0）**全部有编号、有修复、有复审**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十二次对账（CONSOLIDATION-12）· ⭐ turn 管线质量评估 + 模式 P6
- **16 条 P1/P2 复核：16/16 仍成立**；【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**
- 本轮（B0119–B0145，27 批）产出结构：**3 条 P3、0 条 P1/P2、1 次范围收窄、1 次模式 Q 复发**；三条 P3 **全部是「局部具体疏漏」**，与 `web_api` 的判据一致
- ⭐ **turn 管线质量评估**：B0132–B0145 **14 批 0 缺陷**，而它历史上出过**四次事故**（rc.3 N0 / B3-P0-1 / B3-P0-2 / B4-P0），**每次都有编号 + 修复 + 复审记录** ⇒ 本审计读过**质量最高**的一区
- ⭐ **正面模式 P6 正式立档**：**每个 spawn 的 future / 每条打开的通道都必须被驱动到确定的终态**（3 例：① `worker.await`「无条件 join，杜绝泄漏」② `gen_fut`「**绝不 abandon（D2 契约）**」③ 残余事件**排空**而非丢弃）⇒ 与 **P5「部分投递不得静默截断」**是同族两半：**P5 管「投了一半怎么办」，P6 管「没人接手怎么办」**
- ⭐ **本轮最重要的一句**：**零发现是修复的累积结果，不是「本来就对」的证明** —— 判断一个区的质量要问「**它出过几次事故、每次有没有留下编号与复审**」，而不是「我读了几批、找到了几条」
- 覆盖率（251 条去重路径）：Rust `.rs` **70** · 前端 `.dart` **35/217** · CI/脚本 4；`live2d-ai-runtime` 17 文件已读 13

### BATCH-0147（8 机械 + 1 定点读 · **0 新发现** · 覆盖率口径修正）
- ⚠ **我自己的机械错误（今天第三次）**：算 `web_api` 覆盖率时把 INDEX 的 `web_api/<文件>` 与**裸文件名**比 ⇒ 得出「未审 **44**/48」，**比真实值（8）差 5 倍** ⇒ 规则：**脚本算覆盖率/差集前要先用已知答案自检；输出与既有认知冲突时先怀疑脚本**
- 修正后：**`web_api` 48 个 .rs 中 40 个生产文件已审**（**多为定点读，不等于逐行** —— 如实标注）· 余 8 个**全是测试**（2472 行）⇒ 测试面正是 B0001-02 / B0002-02 两条假绿所在的层
- `models_routes/tests_models_p1.rs`(534) 13 用例 **0 缺陷**，且是**成组三向对照**：`拒绝坏值`+`接受合法值` · `坏档备份`+`缺档不备份` · `round-trip`（写入再读回）
- ⇒ ⭐ **正面模式 P7：每个「会做 X」的断言都配一个「不会做 X」的对照**（否则「永远做 X」也能过）—— **这正是 B0001/B0002 两处假绿所缺的对照面**（那两处不是「没有测试」，是「测试**缺对照面**」⇒ 断言可恒真）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0148（3 文件 / 枚举用例名 · **0 新发现** · 判据修正）
- `voice_routes_tests_gate.rs` 9 用例：**每条规则配反向对照**（`显式空→403` vs `缺失→回落缺省` · `ptt 绕过` vs `无 ptt 仍需唤醒词`）· **三向结局**（403 / 400 / 归一化）· **`manual_off_403_takes_priority`**（优先级）
- ⭐ **`token_check_precedes_gate`** —— 把「**鉴权先于功能门**」这条**顺序**约束**按名钉住**。顺序反了 ⇒ 未鉴权调用方可探测门的响应差异（403/400 各自暴露）；而**这条顺序在代码里看不出来**，只能从路由 `if` 次序推断
- `voice_routes_tests_token.rs` → `dotenv_token_is_read_and_still_wins_over_config` = **红线 R 的优先级表述本身**（`.env` 压过 Mod config），现有一条**按名**钉住它的回归
- `voice_routes_tests.rs` 协议级：**404 / 405 / 415 分别钉住** ⇒ 「路由存在」无法用宽松断言蒙过
- ⭐ **判据修正**：同一层里**生产与测试的质量是两件事** —— `web_api` 生产是局部疏漏富矿（7 P1），**测试层却是本审计见过最规范的** ⇒ **「代码老 ⇒ 缺陷在内部」不能外推到同一层的测试**（这是本审计第 N 次修正**自己的判据**而非某条结论）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0149（1 文件 / 定点读 · **0 新发现** · 补「名字 ↔ 断言」）
- `voice_routes_tests_gate.rs:198-208` → `token_check_precedes_gate` 的**断言体真的在测顺序**：config 配 token + 不带 Authorization ⇒ 断言 **401** 且消息写明「**先于总闸判定**」⇒ 顺序反了会红 ⇒ B0148 记的「顺序在代码里看不出来、这里说出来了」**成立**
- ⭐ **正面模式 P8（新）**：`:199-101` `if std::env::var(TOKEN_ENV_VAR).is_ok() { return; }` —— 若跑测试的机器**进程环境**里已有该 token，按红线 R 优先级（`.env`/环境 > Mod config）会**压过** config 里配的 token ⇒ **测试前提不成立** ⇒ **显式跳过**而不是硬断言
- 两条要点：**跳过的判据与被测规则同源**；用**运行期 `return`** 而非 `#[ignore]`（干扰条件是**环境相关**的，静态标记表达不了）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0150（4 文件 / 枚举 30 用例名 · **0 新发现** · `web_api` 收尾）
- `tests_models_core.rs` → 路径守卫是 **1 正例 + 4 类反例**（合法 id / 空·点 / `..` / 分隔符 / 特殊字符）+ `resolve_safe_path` **正反各一** + `atomic_write_round_trip` ⇒ **正例证明不是「一律拒绝」，反例证明不是「只拦 `..`」**
- ⭐ **同一条纪律在两个独立位置各写一遍**：`models_routes` 的 `normalize_relative_id`（模型 id）与 B0092 的 `asset::normalize_resource_ref`（资源路径）—— **两套守卫、同一形态**
- `registry_load_missing_file_returns_default` vs `registry_load_corrupted_returns_error` ⇒ 「一律回落缺省」与「一律报错」**都过不了**对方
- `tests_models_handlers.rs` → **状态规则 + 对照**（`删正在用的被拒` / `删非在用的 204`）+ **三向拒绝理由** + **四向更新语义**
- `voice_routes_tests_say.rs` → ⭐ **`busy_returns_200_ok_false`**：把「忙碌」**钉成具体响应形状**（200 + `ok:false`）而非让实现自选（与 B0005 核的 external 侧契约同形）；`empty_transcript_never_reaches_say` 防空 say
- ⇒ **`web_api` 48 个文件全部核过**（40 生产 + 8 测试）。**如实标注**：测试的「名字 ↔ 断言」只逐条核过 2 处
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0151（2 文件 / 头注 + 定点 · **0 新发现** · 假设证伪 + 模式 P9）
- `client.rs:30-34` 自陈「失败 = `None`（**静默回退**）」；我以为降级对用户不可见 —— **读 `mod.rs` 后证伪**：`:28`「回退原因码见 `FallbackReason::code`，**计数与最近原因见 `PerformanceStats`**」+ `FallbackReason::code`(:85-88) + `is_fallback()`(:96) + `fallbacks: AtomicU64`(:126) ⇒ **本层返 `None`、上一层记类型化原因 + 计数 ⇒ 状态面可见**
- ⭐ 且 `FallbackReason::code` 文档写明「进日志与状态面；**不含正文**」⇒ 可观测原因**排除用户内容** ⇒ 与红线 R 同一条纪律（只是管内容而非密钥）
- ⭐ **正面模式 P9（新）**：**降级在本层静默，在上一层类型化 + 计数**（4 例：`drain_residual_events` · director `staging.degraded` · `client.rs→PerformanceRuntime` · `verify_core_chain` 的 **skipped 而非 passed**）⇒ **与 F-0048/0049/0050 方向相反**（那三条是**信号没有出口**）⇒ **分界线：信号有没有被上一级接住**
- 顺带：`client.rs:20-22` `Auto` 只在**上游 4xx** 时降级重发，传输失败/5xx/超时不重试；`:11-14` body 只有 system+user、**无 `reasoning_content`** ⇒ 与「思考不进下游」一致
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0152（3 文件 / 定点 · **0 新发现** · 模式 F 的反例）
- `client.rs:211-213` → URL 拼接走 `crate::join_endpoint` **单一实现**（runtime 内）
- ⭐ 跨 crate 有**第二份**（`mod-director/staging_http.rs:54`）⇒ 逐边角比对：**尾斜杠、协议、空串处理**全部一致或**更严**，**没有一条会放宽校验**
- 而 `staging_http.rs:52-53` 头注：「**保留一份最小副本**：Mod **不依赖 runtime 的网络层**……**改 runtime 那份时同步对照**」⇒ **架构边界** + **漂移防线**
- ⇒ ⭐ **模式 F 的判据修正**（比发现更有用）：**重复实现本身不是缺陷，「无理由的重复」才是** ⇒ F 的成立条件应是「重复 **且**（无架构理由 **或** 无漂移防线）」
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0153（1 文件 / 定点 · **0 新发现** · 红线 R 第三处独立兑现）
- `performance/client.rs` 两个独立机制保证密钥不进日志：① **类型是 `ApiSecret` 不是 `String`**（:166/:206），下发前须显式 `key.expose_secret()`（:290）② **`Debug` 只输出布尔**：`.field("has_api_key", &self.api_key.is_some())`（:176）⇒ 值永不出现
- ⇒ ⭐ 红线 R「不进日志/状态面」已有**三处互不依赖**的兑现：B0009 端点层（`GET /api/v1/env` 永不回值）· B0105 Mod 状态面（只回 `api_key_set` 布尔）· 本批客户端错误面（`ApiSecret` + `Debug` 布尔）⇒ **任何一处被改坏，另外两处仍拦着**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十三次对账（CONSOLIDATION-13）· ⭐ **正面模式 P1–P9 总纲**（本审计最可带走的资产）
- **P1** 理由要写「不这么做的话**用户会看到什么**」· **P2** 私有汇合 + 策略闭包（6 消费者）· **P3** 更严的写法要连「更松的写法在哪里」写进注释（最强一档：**把「关掉某能力」建模为「不存在该能力」**）
- **P4** 竞态用对抗性评审**编号**记录，在「权威落闩」前关闭 · **P5** 部分投递必须放回/补齐（4 例）· **P6** spawn 的 future 与打开的通道**都必须被驱动到确定终态**（3 例）
- **P7** 每个「会做 X」配一个「不会做 X」的对照（**这正是 B0001/B0002 两处假绿所缺的对照面** —— 那两处不是「没有测试」，是「测试缺对照面」⇒ 断言可恒真）· **P8** 测试要**显式隔离自己的前提**，不成立就跳过 · **P9** **降级在本层静默、在上一层类型化 + 计数**（4 例）
- ⇒ **P9 与 F-0048/0049/0050 方向相反**（那三条是**信号没有出口**）⇒ **分界线：信号有没有被上一级接住**
- **模式 F 判据修正**：`join_endpoint` 两份副本无一条放宽校验 + 头注写「改 runtime 那份时**同步对照**」⇒ **F 的成立条件 = 重复 且（无架构理由 或 无漂移防线）** ⇒ **判据需要「反例」才能定住边界**，否则会把正面样本也算成缺陷
- ⭐ 自评：这轮最值钱的产出**不是 72 条缺陷**，而是 **P1–P9 + 模式 Q 的机械化 + KNOWN-LOSSES 的显式化** —— 前者任何项目可直接采用，后两者防止**审计账本自己**变成不可信资产

### BATCH-0155（2 文件 · **0 新发现** · 结 B0153 未核实项 4）
- ⭐ `secret.rs:16/36-40` → `ApiSecret` 的 **derive 列表刻意不含 `Debug`** + 手动 impl 输出**常量**「`ApiSecret(REDACTED)`」⇒ ① 日后有人把 `Debug` 加进 derive ⇒ **编译错误**（不是静默泄露）② **连长度与前后缀都不泄露**（注释逐字说明）
- ⇒ ⭐ **正面模式 P10（新）**：**「不许 derive 出来」比「记得别打印」强** —— 把禁止项**编码进类型定义**，让违反者**编译不过**；头注 :14-15 写出它支撑的上位性质（**含密钥的结构体整体 `Debug` 打印也安全**），而 `config.rs:4/:10` 对 `LlmConfig` 派生 `Debug` 正依赖这条
- `config.rs:19-23/:33-34` → 「什么值生效」**汇合在 `settings::effective_max_tokens` 一处**（P2 同一形状）；便捷构造的 `max_tokens: 0` 被**显式标记为非生产路径** ⇒ 再次确认 **F-0020-01 是「常量的值不对」而非「机制不对」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0156（1 文件 / 定点 · **0 新发现** · 「恰一次」纪律的最小实例）
- `llm.rs:252-281` → `[DONE]` 的「**恰好一次**」由**一个 flag 两种补发姿势**保证：`self.done` 是唯一闸（见过 `[DONE]` 不再读底层）· 补发前**先 flush 解码器残余**（关流时可能还有未成事件的数据）· 残余非空 ⇒ `Done` **入队尾**（数据先）、残余为空 ⇒ **直接 return**
- 两分支都以 `self.done = true` 收口，而 `map_sse_data` 收到真 `[DONE]` 时**也**置 `self.done`（:263 传 `&mut self.done`）⇒ 真 `[DONE]` 已到时两个 `if` **都不进**
- ⇒ 头注 :6「**对端未发 `[DONE]` 就关流时也兜底补发一次**」= `Done` 是**保证一定来的**；测试注释 :510「收尾必定返回 `Done`（**恰一次**），所以**不能断言「空」**」⇒ **硬形态**断言
- ⭐ **同族纪律的三个层次**：core 双闩锁（B0040）· Stage C `GenerationFinished` 落闩（B0143）· 本批 `Done`（传输层）⇒ **每一处都有自己的唯一闸**
- `:259` 又是 **P5**：「任意切割的字节先进缓冲，再按『完整事件』粒度取出」
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0157（1 文件 / 定点 · **0 新发现** · 判据精化）
- `plan.rs:579-632 parse_v1` → 校验顺序**有意**：**段数上限 O(1) 先拒**（畸形输入不进逐段工作）→ 逐段 `as_str`（带**段序号**）→ **总字符预算循环后判**（带实际字符数）→ 核心不变量 → cues（带**序号 + 越界字段名**）⇒ 与 B0122「合并后再校验最终值」**同形状**
- `:602` `segment.chars().count()` ⇒ **按码点计**，与 B0127 的 `chars().take()` 同一口径 ⇒ CJK 不被按字节虚高计数
- ⭐ **判据精化**：`:611` 硬不变量的两个子句**看似蕴含、实则承重** —— `segments=[""]` 时 `concat()==""` **通过**第一个子句，但该 plan **声称有一段空文本** ⇒ 第二个子句堵真洞
- ⇒ **`model_root.rs:111`（F-0001-02）的判据从「按语法看像不像」换成「反事实测试：去掉另一半会怎样」** ⇒ 同一种语法，一个恒真（P1）、一个承重（正确）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0158（1 文件定点 + 全 workspace 机械扫 · **0 新发现**）
- 表演层 **19 个码**（`PlanWarning` 2 + `PlanError` 17）形状统一为 `<stage>_<layer>_<suffix>`，逐条不同，`stage` 恒为 `performance` ⇒ 前端能按 stage 分流
- 全 workspace 扫 `=> "snake_case"`：**24 出现 / 24 去重 = 零重复** ⇒ **对本扫描集合**，「拿界面上的码去日志里搜」**不会歧义**
- ⚠ **覆盖缺口（如实标注）**：主链 `ErrorKind::code()`（`llm_upstream_401` 等，B0015 核过两端同源）**没进这次扫描** ⇒ 它是 `format!` **拼出来的**，**不是字面量** ⇒ 「全局唯一」**对本集合成立，非全系统**
- ⭐ 由此记一处**结构观察（不记发现）**：表演层是**闭合枚举**（19 条 · 可枚举可文档化）· 主链是**开函数**（含上游状态码）⇒ **两族用不同机制实现同一条契约**；`llm_upstream_401` 本就该是开集合 ⇒ 非缺陷，但**「错误码清单」文档只能覆盖闭合族** ⇒ 写全码表**不能靠一次全局 grep**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0159（1 文件 / 定点 · **0 新发现** · ⭐ 第 14 条红线闭合）
- ⭐ `error_code.rs:44-64` → **`code()` 与 `stage()` 在同一个 `match` 上构造** ⇒ 「stage 是 code 的前缀」**不是纪律而是构造事实**（枚举加变体时两处都要写 ⇒ 漏一处编译不过）⇒ **「接缝无人管」在这里结构上不成立**；:55-57 明写该性质**被回归测试锁定**
- `:55-57` 「**背压归 `tts`** —— 它就是 TTS 队列的背压，**不是独立阶段**」⇒ 概念上不归属任何阶段的变体，**在 `code()` 与 `stage()` 里同样归到它真正所属的阶段**
- `:68-69` `is_fatal()`「**口径唯一**：与 supervisor 的 `saw_fatal_kind` **完全一致**——`Llm` 不致命（见 **D8 裁决**）」⇒ 三处引用同一裁决
- ⇒ ⭐ **红线「错误码两侧同源」四环节全核过（CLOSE）**：产生（主链 B0159 / 表演层 B0158）· 传递（B0015）· 消费（B0036/B0037）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0160（1 文件 / 定点 · **0 新发现** · 错误码红线安全性质结案）
- ⭐ `error.rs:87` → 上游状态码进码走 **`status.as_u16()`（类型化整数）** ⇒ 码恒为 `<stage>_<已知后缀>` 或 `<stage>_upstream_<数字>` ⇒ **码里不可能出现上游响应文本**；而码要流经**三处**（WS `error` 帧 · 日志 · 前端分支）⇒ **三处都不受上游文本影响**
- ⇒ **这是结构性保证，不是「做了清洗」**（与正面模式 **P10** 同形：**把禁止项编码进类型**）
- `:80-84` → 契约性质**连同根因**写明（「改动等同协议变更」+「那正是 2026-09-11『后端出错无具体错误代码』的根因」）⇒ **为什么**可从代码本身恢复
- `:98-101` → `upstream_status()` 单独提供，理由逐字：「**不该去解析 `Display` 字符串**」⇒ **正面模式 P3 的又一实例**（提供类型化访问器并点名更松的那个）
- `:110-119` → 响应体片段**按字符截断**（理由：「**避免劈开 UTF-8**」）+ 超限**补 `…`** ⇒ **有界 + utf-8 安全 + 截断可见**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0161（1 文件 / 定点 · **0 新发现** · 提示词 = 反误诊装置）
- ⭐ `error_code.rs:107-109` → `non_auth_note`「**只在 401/403 时追加（它要直接反驳「是不是我改提示词改坏的」这个误判）**」，且**按段参数化**（`llm`「提示词内容与鉴权无关」/ `tts`「检查 `[tts]` base_url / voice」）⇒ **同一个 401 在不同段给出不同的反误诊**（正面模式 **P2**）
- 与 AGENTS.md 记的 **2026-09-11 事故逐字对应**（用户报「改提示词就崩」；复核：改提示词不失败，真因是**进程环境缺 `DEEPSEEK_API_KEY` 的 401**）⇒ 那不是泛泛提示，**是一条被写进代码的反误诊**
- 状态码分支**穷举**（401/403 · 404「含尾部 `/v1` 的写法」· 429 · 5xx · 兜底）⇒ **`Status` 臂内零个 `None`**，连兜底都指向「上文字段里的**上游响应体**」⇒ **兜底也可执行**
- ⇒ 「**码 → 可执行处置**」链闭合：码可搜（B0160 `as_u16()` 有界）· 帧带 `hint`（B0015）· 前端横幅**按码分流**（B0036/B0037）⇒ **`hint` 不是装饰性字段**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0162（3 文件 / 交叉核 · **0 新发现** · 候选池 KL-2）
- ⭐ 链路**四步全核**：`Error::Status{body}`（`error.rs:32` `#[error("上游非成功状态 {status}: {body}")]` ⇒ Display 含片段）→ `app_event.rs:80` `message: kind.to_string()`（**取 Display**）→ `ws/events.rs:194` `"message": err.message` ⇒ **上游响应体片段会到浏览器**
- 而这是**刻意的**（`error.rs:127-138` 回归钉住，注释「Display 可观测：状态码与片段都在」）
- **不记发现**：触发需「上游在错误响应体里**回显凭据**」⇒ **本仓无任何证据**（正反皆无）⇒ 按纪律**进候选池 KL-2**
- 但**不对称已存在** ⇒ **日志需要**片段（拿码搜日志 = 排障现场）· **前端不需要**（要码 + hint）⇒ **修法是投影层一行**（`events.rs:194` 的 `message` 不带 body）⇒ **不动 `Display`**（日志继续拿得到）⇒ 正是正面模式 **P2（私有汇合）** 的形状
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0163（1 文件 / 定点 · **0 新发现** · 降级门控四性质）
- `client.rs:319-336 request` → 降级是**三重合取**：`mode == Auto` **且** 首次确实发了 structured **且** 响应是 **4xx** ⇒ `Prompt` 模式不会触发无谓重试；**显式 `JsonSchema` 不降级**（失败如实上浮，不被悄悄改写）
- ⭐ **重发是语义性的**（:327）：第二次传 **`prompt::SYSTEM_JSON_ONLY`** 而非调用方原 `system` ⇒ 既然前提是「端点不认识 `response_format`」，重发必须**带上一份能替代它的东西**
- `structured` 标记**如实回传**（:322 真发的 / :332 降级后 `false`）⇒ 调用方能分辨**哪种模式产出**这份 JSON ⇒ 正是 B0151 的 `FallbackReason`/`PerformanceStats` 的**数据来源**
- :336 **只重发一次**后 `None` ⇒ 持续 4xx 也不循环
- ⭐ **正面模式 P11（新）**：**显式的用户意图不被静默改写**（2 例：本批「显式 `JsonSchema` 不降级」· B0109「显式 off 压过推断」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0164（1 文件 / 定点 · **0 新发现** · ⭐ 立正面模式 P12）
- `client.rs:346-355 client_error_4xx` → 判据要求**两个独立信号**同时成立（`"error"` **且** `type/param/invalid/unsupported/unknown` 之一）⇒ **偏保守**（宁可不降级）
- ⭐ 头注 :341-345 把三件事写明：① **承认是启发式**并说明**为何**不精确（「**拿不到状态码**」）② 「**误判的代价只是多一次请求**（然后仍按失败回退）」③ ⭐ 该代价**被结构性封顶**
- ⇒ ⭐ **正面模式 P12（新）**：**保守启发式 + 显式声明误判代价 + 代价被结构性上界约束**；**第三要素是第一、二要素能成立的前提**
- ⭐ **本审计第一次核到「A 处的正确性依赖 B 处的性质」**：B0163 的「**只重发一次**后 `None`」⇒ B0164 的「误判代价只是多一次请求」**才成立** ⇒ 两处**同文件相距约 10 行** ⇒ **不是两条独立事实，是一条**
- `:367-374` `wire_tests` 用**真实 loopback TCP 服务器**并收回**每个请求的原始文本** ⇒ 测试可断言**线上内容** ⇒ 模式 H **强形态**应用于 HTTP
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0165（1 文件 / 定点 · **0 新发现** · ⭐ 立正面模式 P13）
- `prompt.rs:34` → 提示把**三种常见失败模式**点名：解释性散文 / **Markdown 围栏** / 多余字段
- ⭐ **切分纪律在提示侧与强制侧是同一句话**：`:38`+`:49`「segments 只能切分原文，**逐字不变**，拼接后**必须与原文完全相同**」↔ `plan.rs:611` `if segments.concat() != source … Err(SegmentsNotPartition)` ⇒ **不是「提示说一套、代码查另一套」**；解析器拒绝时**模型的指令就是「为什么」的出处**
- ⇒ ⭐ **正面模式 P13（新）**：**同一条约束在提示侧与强制侧用同一句话表述**（**3 处**跨语言对齐：本批 · B0116 `DISCIPLINE_TEMPLATE`↔`sentence.rs`「不许硬切」· B0132 AGENTS「空末块发边界帧」↔`worker.rs` 双边界标记）
- 枚举值也**在提示里写死**并与 `json_schema_strict`(:816-832) 的 `enum`/上下界**逐项对应**；解析侧对词表外**报错** ⇒ **三层同形**：提示给全集 · schema 给机器可校验 · 解析给错误
- `:45` `build_user_prompt` 头注「**不含思考**；**不修改任何文本**」⇒ 思考红线在**提示构造侧**也成立
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0166（1 文件 / 定点 · **0 新发现** · 候选池 KL-3 + 结构观察）
- `client.rs:283` → **超时在使用点钳位** `timeout_ms.clamp(MIN, MAX)` ⇒ 与 B0134 `tts_queue_capacity.max(1)`、B0122「合并后再校验」**同形状**；且**每请求各自** ⇒ 降级路径第二次请求**有自己的完整超时**（总上界 2×）
- `:289-291` key **只进头**（B0153 机制）· `:292/:294` 两个失败点都是 `.ok()?` ⇒ **无 panic 路径**
- ⚠ **候选池 KL-3**：`:294` `response.text().await.ok()?` **把整份成功响应体读进内存、无尺寸上限**；而本仓**处处有界**（`MAX_WAV_BYTES` · `MAX_SNIPPET_CHARS` · `MAX_SEGMENT/SPEAK_CHARS` · `MAX_RECORDS` · 注入预算）⇒ **这里是不设界的一处**，且在**成功路径**
- **不记发现的两条理由**：① **危害上界读不出来** —— `.timeout()` 是否**覆盖 body 读取**取决于 **reqwest 版本语义**，而**我禁止运行任何构建/测试**；最坏推测是「超时后回落」＝**降级而非崩溃** ② 触发需用户把 `base_url` 指到会大量吐数据的地方 ⇒ **无远程默认路径**
- ⭐ **结构观察（比发现有用）**：`post_once` 返回 **`(bool, String)`** ⇒ **状态码没被传出** ⇒ 上层 `client_error_4xx`（B0164）**不得不嗅探响应体去猜** ⇒ **一个函数的返回类型，会决定上一层需不需要写启发式**（正面模式 **P13 的镜像**：那里缺的不是注释，是**一个类型**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0167（4 文件 / 定点 + 断言面 · **0 新发现** · ⭐ P9 两端闭合）
- `performance/mod.rs:124-130` → `PerformanceStats` 六个原子计数 + `last_reason: **AtomicU8**`（存**枚举下标**而非字符串 ⇒ **有界、免分配、可原子更新**；与 B0160 `code_suffix()` 用 `as_u16()` 同族的「用类型化的东西而不是文本」）
- `engine.rs:85-86` → `pub fn performance_stats()` ⇒ **不是私有字段** ⇒ 上一层真能拿到
- ⭐ 消费端**有读者**：断言 `rt.stats().plans() == 1` · `fallbacks() == 0` · `last_fallback() == "performance_ok"`（`tests.rs:647-649`）· **且 `:690` 断言的正是降级侧取值 `"performance_plan_invalid"`** ⇒ **「降级发生了」被一条断言钉住** ⇒ 不会悄悄腐烂成「没人看的计数」
- ⇒ ⭐ **P9 两端闭合**（① 有类型化原因+计数 B0151 · ② 确有人读且回归钉住降级值 B0167）
- ⚠ **诚实标注「可见」的准确含义**：模块→引擎**有**消费者（`performance_stats()` 公开 + 回归）⇒ **P9 门槛已满足**；但 **引擎→HTTP/WS/UI 零命中** ⇒ **用户看不到**「回退了 N 次」⇒ **不记发现**（把内部计数器接进 UI 是**产品决策**，不是纪律缺口）⇒ 同时**精化了 P9 自身的适用边界**（门槛是「上一级接住」，**不是**「用户看得到」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0168（1 文件 / 定点 · **0 新发现** · ⭐ 补完整「类型决定决策」这条）
- `plan.rs:851-857 action_cue_payload` → **三个字段 + 逐 cue `to_json`，无任何加工** ⇒ **不是决策点，是纯投影**；而验证**全在两侧**：生产侧 `parse_plan`（词表外报错，B0157/B0128）· 消费侧 `FieldRuntime::frame` **13 项白名单**（B0015）
- ⇒ **红线 O 的「结构性」主张因此站得住**：白名单**不会被中间层绕过**，因为**中间层里根本没有能改写字段的东西**
- ⭐ **与 B0166 互为镜像**：`post_once -> (bool, String)` ⇒ **状态码被丢掉** ⇒ 上层被迫嗅探猜；`action_cue_payload -> map(to_json)` ⇒ **决策被留在两侧** ⇒ 中间层无需判断
- ⇒ 统一成一句：**类型决定决策落在哪里** —— 好的类型让决策**只出现在真正该出现的地方**，差的类型把决策**挤到不该出现的地方**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0169（1 文件 / 定点 · **0 新发现** · ⭐ 立正面模式 P14）
- `performance/mod.rs:284-289` → 三路前置门：`!enabled ⇒ Disabled` · `assistant.is_empty() ⇒ EmptyAssistant` ⇒ **两个原因码各自独立且零 I/O** ⇒ 「关掉」与「没话说」在观测面上**可区分**（正面模式 **P9** 粒度够用）
- ⭐ `:301`「**V1 拼接基准 = 送进提示词的同一份 trimmed 原文**」⇒ `assistant` **同时**是 `build_user_prompt` 的正文(:294) 与 `parse_plan` 的 `source`(:302)
- ⇒ 所以 `segments.concat() != source`（B0157）拒掉一份 plan 时**「为什么」是确定的**：模型**没照着指令切它实际收到的那段文本**。⇒ ⭐ **正面模式 P14（新）**：**「提示与校验同源」不能只靠两处写同样的文字，必须让两侧消费同一个值**（否则 P13 只是「两句话长得像」）
- `:299`「剥围栏是**宽容解析**，契约不变：**仍然必须过同一个校验器**」⇒ **「宽容」被放在进门前，不渗进契约**
- `:304-305` 成功路径也记账（`plans += 1` + `last_reason = 0`）⇒ 与 B0167 `:690`（断言降级侧取值）**成对**：两边都有断言
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0170（8 次只读核验 · **0 新发现** · 假设两次证伪 + F-0040-01 影响升级）
- ⭐ **我的假设被两次证伪**：「`performance/mod.rs:65`『单一真源 = director 的纯函数』是陈旧引用（因为 director 已删除）」——① 目录在，且**是 workspace member**（Wave 3 G 轨「新导演**最小骨架**」· 注释「**收束时由主 agent 注册**」）② 新骨架**确实拥有**规则 cue 机制（`arbiter.rs` `Cue`/`cues()`/`cue_for()` + `lib.rs:445 rule_cues`）⇒ **注释准确**
- ⇒ 「director 已删除」指的是**旧的动作序列 driver**（归档 `archive/action-layer-p6`），**名字被复用**给了一个**刻意 no-op 的新骨架**（`mod_registry.rs:670`「行为**一字不变**」）⇒ **同名两物**，但注释指的是后者 ⇒ **没错**
- ⭐ **正面模式 P15（新）**：**文档写「某物已删除」时，要核「那个名字是否被复用」**（按名字找会找错）
- ⭐ **F-0040-01 影响升级**（定级仍 P3，已改规范头）：AGENTS 休眠台账写 `main.rs::mod_count_is_three`，**实际是 `mod_count_is_five`**（`main.rs:471`）⇒ **不是「文档内部不一致」，而是「文档指向了一个不存在的符号」**；台账的「三个」比实际**落后两位**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0171（1 文件 / 定点 · **0 新发现** · ⭐ 立正面模式 P16）
- ⭐ `mod.rs:368-369` → **计数口径被一句话写死，且点名最容易被「优化」掉的 case**：「每一次没用上表演层 JSON 的轮次都计入 fallbacks（**含关闸**）」⇒ 「关闸也算」**恰恰是**后来者会顺手「修正」的判断（「关闸是**预期**行为，凭什么计失败？」）
- 且**立刻给出更细信息的人的出路**：「想区分『关闸』与『开了但失败』**看 `last_fallback` 的原因码**」⇒ **一个计数 + 一个原因码 = 两条信息**，不必加第二个计数器 ⇒ ⭐ **正面模式 P16（新）**
- `:375-379` → 降级**进了日志且带结构化 `code=`**（`target: "performance"`）⇒ 与 AGENTS 错误契约一致（**本审计 B0001-01 那条 P1「pre-dispatch 零日志」的对照面**）⇒ 与 B0160（码**有界**）· B0161（码**带可执行 hint**）**接成一条链：降级有码 → 码可搜 → 码带处置**
- 三处形状细节：`:374` 「是不是回退」由 `reason.is_fallback()` 判定（**判定不在这层**，P2）· `:382` host 未注入规则函数 ⇒ cues 为空、**不崩** · ⭐ `:385` `segments: **None**` 而非 `[]` ⇒ 避免「回退」与「不动」混淆（`[]` 在 v1 口径里另有一义）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0172（1 文件 / 定点 · **0 新发现** · ⭐ 红线 R 第四档）
- ⭐ `mod.rs:169-184 to_json` → 七个字段**逐一**核过：五个 `u64` 计数 + 两个 **`&'static str`**（`last_fallback` 来自 `code()` · `last_structured` 是两个字面量之一）
- ⇒ 「**没有正文**」是**结构性**的（**没有字段能装文本**）· 「**没有密钥**」更彻底：`PerformanceStats` 本身(:124-130) **只有六个原子量、连一个 key 字段都没有** ⇒ **密钥在结构上无处可放**
- ⭐ **红线 R「密钥不出口」四档强弱排序（表补完）**：① 端点层「**选择不输出**」② Mod 状态面「选择输出**安全替身**」③ 客户端错误面「**类型 + 选择**」④ 表演层状态面「⭐ **结构上无处可放**」⇒ **第 4 档强于前三档**
- ⇒ 与正面模式 **P10**（不让违规在**编译期**可能）**同一形状**：**让违规在结构上不可能，而不是靠记得**
- 另注：B0167 已核该状态面**目前无生产读者** ⇒ 脱敏是**当前冗余**，但**是正确的冗余**（类型**无论将来被谁读**都安全 ⇒ 冗余在**类型**上，不在纪律上）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0173（1 文件 / 定点 · **0 新发现** · 给 F-0127-01 添对照数据）
- `plan.rs` 两个 `message()` **逐变体一个臂、无 `_ =>` 兜底** ⇒ 与 B0135 `handle_engine_event`（9 臂/无兜底）**同一形状**（新增变体**编译不过**）
- 文案**都带位置 + 越界值**（「segments **第 {i}** 个元素不是字符串」/「**第 {index}** 条 cue 的 **id={id}** 不在能力集内」）⇒ 与 B0157「错误都带序号」**同纪律**；`PlanWarning` 的文案说明**「已丢弃该键」** ⇒ 读者能分清**拒绝 vs 容忍后丢弃**
- ⭐ `SegmentsTooLong` 文案把**策略**写进消息：「segments 超限（**不截断**）」/ `SegmentsNotPartition`「**只能切分，不能改写**」⇒ **同一文件里两类上限处置相反，且各自写明**
- ⇒ **受维护的 v1 `segments` 全部「超限即报错 + 文案写明」**，**只有已弃用的 v0 `speak` 才静默截断** ⇒ **纪律掉落在被弃用的那条路上** ⇒ **F-0127-01 不是「风格不一致」而是「只有 legacy 路径才不一致」**（**加强** B0128 的收窄判断，已按模式 Q 补进规范头）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0174（1 文件 / 枚举 + 定点读 · **0 新发现** · 两处强形态）
- ⭐ 「**5xx 不得重试**」是**数真实请求**钉住的：`assert_eq!(handle.await.expect("mock").len(), 1, "5xx 不得重试")` ⇒ 断言**不是**「返回了 None」而是「**恰好发了 1 个请求**」⇒ **加了重试的实现会发 2 个 ⇒ 测试变红**
- ⇒ ⭐ **正面模式 P7 的最强形态**：**对照不必是另一个用例，可以是「线上真实发生的次数」**
- ⭐ 「**思考不进下游**」在**客户端**被钉住：上游返回 `content:""` **且** `reasoning_content:"想一想"` ⇒ 客户端 `request(...) == None`（**拒绝**把思考当正文），随后**取回记录下来的请求原文**继续断言
- ⇒ 这是该红线**第四处**独立兑现（前三处：解析层单列 `ReasoningDelta` B0009 · WS **独立帧** B0015 · 请求体只有 system+user B0151）⇒ **补上了「客户端拿到带思考的 200 响应时的行为」**；与 B0009 的 `reasoning_never_reaches_the_sentence_assembler` **成对** ⇒ **两道闸，各自独立**
- `wire_tests` 三个用例覆盖**正常 / 降级 / 回落**三态**各有独立回归** ⇒ 与 B0117「正常 / 显式不清空 / 缺省」三向**同形**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0175（1 文件 / 定点读 · **0 新发现** · ⭐ 本审计读过的最强测试）
- ⭐ `auto_mode_degrades_to_prompt_on_4xx` **一条测试钉住五件事**，且**全是外部可观察量**：① `!reply.structured`（降级**能被调用方看见**）② `raws.len() == 2`（**恰好**两个请求）③ `raws[0]` **带** `response_format` ④ `raws[1]` **不带**（降级的全部意义）⑤ `raws[1]` 含「**只输出 JSON**」（重发**带上了替代指令**）
- ⇒ 每一断言都对应一类**具体错实现**：多/少一次重试 · 一开始就降级 · 只改 flag 不改请求 · 只翻 flag 没换 system · 返回值不报状态
- ⭐ **闭合了 B0165 留的口**：那批我记下降级是「**语义性**的」（第二次传 `SYSTEM_JSON_ONLY`）并写明「**除非有测试，否则那只是设计主张**」⇒ **有**：⑤ 断言的正是第二个请求的**线上正文**里含那句指令 ⇒ **从「设计意图」升级为「被测试钉住的行为」**
- ⭐ **同时是 B0164 那个保守谓词的端到端正例**：测试构造的 4xx 体 `{"error":{…,"type":"invalid_request_error"}}` **正好命中 `client_error_4xx` 的两个必要条件** ⇒ B0166 已核「状态码**没被传出**、只能嗅探」⇒ **那条嗅探谓词在真实形状上有效**，B0164 的「保守 + 声明误判代价」**不是孤立的**
- 与 B0174 是**同一手法的两次应用**（数真实请求 + 读真实请求体）⇒ 正面模式 **P7 最强形态** 已有 **2 个样本**；三个 wire 用例覆盖**正常/降级/回落**三态**各有独立回归**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0176（1 文件 / 定点 · **0 新发现** · ⭐ runtime 收尾）
- `Resolution` 五字段**每个都写明语义差别**（不只是类型）；⭐ `structured: Option<bool>` 里 **`None` ≠ `Some(false)`** ⇒ **「没试 structured」与「试了、走 prompt 路」是两个状态**（正面模式 **P11** 又一例）
- ⭐ `is_noop()` **按路径各取自己的字段**（v1 看 `segments` · v0 看 `speak`）⇒ **这依赖** B0171 那个 `segments: **None**`（而非 `[]`）⇒ **类型选择与判定逻辑互为支撑**；若 `segments` 是 `Vec`，这里就得**再加一个「走哪条路」的标志** ⇒ **B0168「类型决定决策」���更小同形**
- ⭐ `Debug for PerformanceRuntime` 是红线 R 的**第五处**，形态与前四处都不同：**七项、零项携带配置内容** —— `allow`（**可能是用户起的表情名**）只给 **`allow_len`** · `rule`（闭包）只给 **`has_rule_fallback`** · `client` 只给 **`kind()`` 类型名**
- ⇒ **红线 R 五种形态表补完**（不输出 / 安全替身 / 类型+常量 / 结构上无处可放 / 逐项给长度·布尔·类型名）⇒ **同一条纪律、五种实现、不靠一处**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十四次对账（CONSOLIDATION-14）· ⭐ 一整个区读完后的复盘 + P12–P16 立档
- **16 条 P1/P2 复核：16/16 仍成立**（十四次对账无一被推翻）；【模式 Q 双向检查】**✔ 通过**
- 本轮 26 批（B0151–B0176）**0 条新发现**，产出 **5 条正面模式**（P12 启发式+代价上界 · P13 提示↔强制同句 · P14 两侧消费同一值 · P15 名字是否被复用 · P16 计数口径写死）**+ 2 张红线表补完**（P9 四例 · 红线 R 五形态）**+ 1 条判据修正**（模式 F）
- ⇒ **58 批里 26 批 0 发现**，而这 26 批的产出是**可带走的方法**而非缺陷清单 ⇒ 印证 CONSOLIDATION-12 §⑦：**零发现不等于无价值**
- ⭐ **最可带走的一条**：**「类型决定决策落在哪里」**（6 个案例：`(bool,String)` 逼出嗅探 · `map(to_json)` 让红线 O 无从绕过 · `None` vs `[]` 让判定能分路径 · `AtomicU8`/`as_u16()` 让码有界 · `ApiSecret` 让违规编译不过 · 只有 6 个原子量让密钥无处可放）
- 两条假说被**我自己证伪**（B0170）⇒ 立 **P15**；同批 **F-0040-01 影响升级**（AGENTS 写 `mod_count_is_three`、实际 `mod_count_is_five` ⇒ **文档指向了不存在的符号**）
- 覆盖率（291 条去重路径）：Rust `.rs` **79** · 前端 `.dart` **35/217** · CI/脚本 4 ⇒ **三个区已读完**：`live2d-ai-runtime` 17/17 · `web_api` 48/48 · turn 管线 100%
- **仍未审最大面**：前端 `.dart` **182/217** · Mod crates 逐文件（8/61）· `shared/` · 根 `tests/`(3 py) · `verification/`
- ⭐ 自评：「找出问题」是审计的**最低标准**；**「说清一处设计为什么是对的」才是它能产生复利的部分**

### BATCH-0178（3 测试文件 / 枚举测试名 · **0 新发现** · ⭐ 换根前端）
- 覆盖率（机械 + **自检**）：**218 个 .dart · 已审 31 · 未审 187**；⚠ 自检发现与账本 35 不一致（差 4，因**路径归一方式不同**）⇒ **权威值以机械口径为准，不选更好看的数**（B0147 规则**第二次生效**）
- ⭐ `display_prefs_test.dart` → **测试名写出「被否掉的另一种做法」**：「**非有限数回落默认（不是夹到边界）**」+「**越界值夹到区间内**」⇒ 相邻 case、两种策略、**两个都命名**；另「**整数形式的 JSON 数值也能读（localStorage 常见）**」「**toJson→fromJson 幂等**」
- ⭐ `action_scales_wiring_test.dart` → **跨层契约从前端侧钉住**：「③ **PATCH 形状 == 服务端 D40 契约**」「④ **body == 精确 JSON**（不是 contains）」「⑤ 合并防抖 + **只发被改过的键**」（正是 B0069 撤回 F-0030-01 时用的推理）+ 一条**源码扫描型**测试且名字注明
- ⭐ `memory_panel_test.dart` → **测试名写出诚实性原则**：「数字照抄，**缺失/非数字 → —（不假装是 0）**」= 正面模式 **P1 用在测试名上**；「**每个码都带码且可处置**」= 从前端侧钉住 B0161 的 hint 纪律
- ⭐ **跨语言共同形态**：**例外是局部的，周边文化往往很强**（`web_api` 生产差/测试强；Dart 测试比 Rust 更强；而团队 45 条里的两处假绿是**局部例外**）⇒ **按「典型文件」抽样会系统性高报**；**要找例外必须刻意去找** —— 找名字里带「不是…」「不假装…」的断言（**写下了这种名字的人正是知道自己想防什么的人**，而它**最容易被悄悄破坏**、**不会让旧测试变红**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0179（test/ 名义扫描 + 1 条定点 · **0 新发现** · 假怀疑被证伪）
- 按 B0178 判据搜出 **16 个「自我设防」测试名**（`不是`/`不假装`/`不得`/`否则…`）⇒ 其中 **`F-0001-3` / `F-0002-2` 是团队 45 条审计的回归**，而「**F-0002-2：_store 一开始就是真库（不是占位）**」**正是我这类审计专门找的那类**，已被写成回归钉住
- ⭐ 挑最可疑的一条「`LiveRegionThrottle` **被用上**」核验 ⇒ **被证伪**：**时钟可注入**（`now: () => now` ⇒ 控制时间而非 sleep）· 断言**累积后的完整播报串**（语义，不只是 bool）· **表驱动 6 个句末标点**
- ⇒ ⭐ **判据也会产出「假怀疑」** ⇒ **启发式必须逐条核**；核完的收获常是**更好的东西**
- ⭐ 顺带**新的 P13 样本**：句末标点集合「。！？…；**\n**」在**两侧各自实现**（`semantics_test.dart:151` ↔ B0132 的 `sentence.rs` `is_terminator` 含 `\n`）⇒ **一句规则、两种语言**；⚠ AGENTS 列的是「」。！？…`**等**」⇒ 「等」留了口子**不算不一致**，但**完整集合应以两处实现的 6 个为准**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0180（1 文件 / 定点读 · **0 新发现** · ⭐ 立正面模式 P17）
- `action_scales_wiring_test.dart:16-21` 头注把**扫源码降级的技术原因**写成**三条可核事实**（`main.dart` 依赖 `package:web` 在 Dart VM **加载不了** · `_stub.dart` **不回调 `onTransport`、没有 bridge** · **有先例** `wiring_test.dart`）⇒ **都不是「懒得测」**
- 断言本体是**字符串包含**（`settings.contains('pinnedScales: _actionScalesSyncer.pinned')`）⇒ 确实弱于行为测试（可能因注释/别处误命中），**但它能失败** ⇒ **不属模式 H 的「结构上无法失败」** ⇒ **不记发现**
- ⭐ 真正让它合格的是**头注最后那句**：「**发帧那条路由真机验收**」⇒ **扫源码覆盖不到的那一半被显式指给了另一个手段** ⇒ ⭐ **正面模式 P17（新）**：**测试方法被降级需两样才合格 —— ① 技术原因（可核）② 覆盖不到的部分由谁负责**；缺①=偷懒 · 缺②=半个洞
- ⭐ **跨层观察**：这条降级的**根因是红线 K** —— `app/browser_io.dart` 为离线优先隔离了 `package:web` ⇒ **正因为**隔离，`main.dart` 才在 VM 里加载不了 ⇒ **一处好的设计会制造新的约束**；**判断设计好不好，要看它制造出来的约束有没有被诚实地处理**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0181（1 文件 / 定点读 · **0 新发现** · ⭐ 立正面模式 P18）
- `chat_notice_test.dart:150-168` 三条结构型断言**各带 `reason` 说出它要防的失败模式**：① 「判据写在**纯模块**里才测得到；控制器自己写分支就[测不到]」② 「另写字面量 → **改文案时漏改一处，测试与界面不一致**」
- ⭐ **分工完整，不是「用 grep 代替测试」**：**决策逻辑**（`settleTurn` 七种结局）由 `turn_liveness_test.dart` **行为**测（B0139 核）· **控制器确实调用它**由结构断言守（本批）· **调用真的存在**我逐行核过 `:429` 签名 `:460` 调用（B0140）⇒ **turn 收口有三处独立确认**
- ⭐ **正面模式 P18（新）**：**结构型断言要（1）自带 `reason`（2）有**行为**测试覆盖被引用的纯逻辑**；两者合起来才叫「测过」，**只有结构断言、找不到行为测试的，才降级为「仅防架构被改回去」**
- ⚠ 诚实标注共性弱点：`controller` 是**源码文本** ⇒ `contains` **可能在注释或无关处命中**；不构成发现（能失败、意图清楚），但它是这一类测试的边界
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0182（1 文件 / 定点读 · **0 新发现** · ⭐ P18 已验证 + 闭合 B0139 留点）
- `turn_liveness_test.dart:71-107` → `mustReleaseTurnOnWsLoss` 有**真实调用**的行为测试（非 `contains`）⇒ **P18 第 (2) 条满足**
- ⭐ **三个状态全测**：该收的 `closed`(:84) + **两个不该收的** `connecting`(:91) / `connected`(:100) ⇒ **P7（反向对照）在一个状态谓词上被完整执行**
- ⭐ **闭合了 B0139 留的未核点**：那批我读头注时记下「判据用 `isProblem` 而**不是** `!isUsable`，因为**`connecting` 也不可用但它是「正在建连」**」，当时列为未核 ⇒ **现在有测试钉着** ⇒ 日后改成 `!isUsable` **测试会红** ⇒ **一个容易被「顺手修正」的判据被保住了**
- ⇒ **P18 三条结构型断言全部满足两条件**（`settleTurn` 7 条行为测试 B0139 核 · `kWordlessTurnNotice` 常量 · `mustReleaseTurnOnWsLoss` 3 状态全测）⇒ **P18 从「提案」变成「已验证」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0183（1 文件 / 定点读 · **0 新发现** · ⭐⭐ 立 P19 + 修正 B0181）
- ⭐⭐ `background_hydration_window_test.dart:232-233` → 「扫的是**调用形状**（不是某一行字），而且先**剥掉注释与字符串** —— 否则**注释里提一句旧实现就会把守卫自己判红**」+ `:234-236` `stripCommentsAndStrings(File('lib/main.dart')…)`
- ⇒ ⭐ **它正好消掉了我 B0181 标注的那个弱点**（「`contains` 可能在注释处命中」）⇒ `contains` **不再是 grep，而是「调用形状」断言** ⇒ ⭐ **正面模式 P19（新）**
- `:242-245` `reason` 给的是**因果链**不是口号：「`createBackgroundStore` 是**同步**构造（open 惰性在实现内部）⇒ **根本不必等水合** ⇒ 占位库暴露 ⇒ **窗口内导入的字节就进了这个即将被丢弃的实例**」
- `:249` `reason` **主动承认**「代码路径，**不是注释里提一句**」⇒ **正是我提的那个关注点，他们自己先提了**；`:229` 组名诚实标注「（**结构性守卫**）」并写明「只能扫源码：`main.dart` 在 VM 里加载不了」⇒ **P17 两条件齐**（B0180）
- 且是**正反两条**（`contains('_store = createBackgroundStore()')` 为真 ＋ `contains('MemoryBackgroundStore(')` **为假**）⇒ **重引入占位库会被第二条抓住** ⇒ **P7 用在否定式守卫上**
- ⇒ ⭐ **修正 B0181**：**不剥注释的 `contains` 只算「防架构被改回去」；剥了才算「断言调用形状」** ⇒ **P18 + P19 合起来才是完整判据**
- ⏰ 待核：**`stripCommentsAndStrings` 本体**（**P19 的承重实现**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0184（9 文件 / 机械比对 · ⭐ **P2 1**）
- ⭐ `grep -rln "String stripCommentsAndStrings"` ⇒ **8 份副本**；逐份 `md5sum` ⇒ **7 份相同、1 份不同**；`diff` ⇒ 漂移那份**多一行** `out.write('S');`
- ⇒ **多数版把字符串「删空」**（剥完 `continue;` 不写）⇒ **两侧代码会粘连**；**漂移版保留分隔符 `'S'`** ⇒ **更强**，不是更弱；且**零共享 helper**（无 import 共享版）
- ⇒ **F-0184-01（P2）· 判据是「同目录 8 份承重辅助、零共享、且已漂移一份」这个可核事实**，**不是**「某条守卫已被骗过」（**我无法演示**：现有 needle 都是长而具体的）⇒ 且它正是 **P19 的承重实现**
- 修法：收敛成 `test/support/source_scan.dart`，**以漂移那份为准**，并把「保留分隔符」**写进函数名**（P3/P19 的应用）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0185（3 次只读核验 · **0 新发现** · ⭐ 修正自己那条发现的建议）
- 落点核对：`test/` 下**无任何子目录** · **无** `support/helper/util/common/shared` 共享文件 · **0 处 import 共享版**、**无 `part`** ⇒ **8 份各自自足、`test/` 是扁平的**
- ⭐ **这推翻了我自己 F-0184-01 的建议**：「抽成 `test/support/source_scan.dart`」**会打破扁平约定**，并让测试之间**产生 import 依赖**（而 8 份副本正是没有依赖的）
- ⇒ **修正后的建议二选一**：**(a) 保持扁平 + 8 份收敛成同一内容**（采用加固版 `out.write('S')`）**代价是接受重复**、换来「每个测试文件自足」/ **(b) 引入 `test/support/`** 一次抽取、**代价是打破扁平** ⇒ **无论哪条，8 份必须一致；漂移才是问题，重复不是**（已按模式 Q 写进规范头，**观察不变**）
- ⇒ 这是 **B0152 那条判据的第二次应用**（「重复 **且**（无架构理由 **或** 无漂移防线）」）⇒ 这里的「理由」很可能是「**测试独立性**」，而「**无漂移防线**」仍成立（**已漂移 1 份且无人察觉**）⇒ **判据不要求「必须收敛成一份」，只要求「别让它们各走各的」**
- ⚠ **诚实标注**：「`test/` 扁平是否刻意」**无注释** ⇒ 未核实；间接证据较强（8/8 各自定义 + 0 处 import + 无子目录 ⇒ 若偶然早该有人抽）⇒ 只写「强烈暗示」，**不写成「这是刻意约定」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0186（3 次只读核验 · **0 新发现** · ⭐ 红线 M 三层闭合）
- ⭐ **红线 M 三层闭合**：① **平台拆分**（`_web` / `_stub` / 条件导出 ⇒ 正面模式 **P10 的形状**）② **5 个使用点**（`app_shell` / `nav_host` / `session_sheet` / `stage_host` / `confirm_discard_dialog`），而 **`live2d_stage.dart` 零控件** ⇒ 它是**纯展示**，「谁需要垫层」在结构上就清楚
- ③ 测试**枚举完整**（AGENTS 记的 4 个中招控件**全部有**，另**多测** `loading` 覆盖层与**设置面板三种宿主形态**）**+ 有行为**（「error 覆盖层的重试有垫层（**并且真的点得着**）」⇒ `testWidgets` **真点一次**）**+ 验 stub**（「非 web 上垫层**完全透明**（不改尺寸、不挡内容）」⇒ 降级**不是「什么都不做」**）
- ⚠ **两条自己的 grep 失误**（规则 3 变体 7）：① `grep -oE "test\('[^']+'"` 对 `test/stage_pointer_interceptor_test.dart` **零命中**（它用 `testWidgets(`/`group(`）⇒ 差点误判「文件里没测试」—— **今天第二次**栽在「grep 模式与实际声明形式不符」
- ⇒ ⭐ **由此记一条待办**：**我此前对 Dart 测试面的枚举可能系统性漏项** ⇒ B0178/B0179 那两条高产结论（16 个自我设防测试名 / Dart 测试文化强于 Rust）**都建立在这个 grep 上** ⇒ **B0187 第一件事：用 `(testWidgets|test|group)\(\s*'` 全量重扫并与旧结果比对**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0187（4 次只读核验 · **0 新发现** · ⭐⭐ 方法论重大修正）
- ⭐⭐ **覆盖面：839 / 1602 ⇒ 旧模式只覆盖 52%** —— 原因：Dart 有**三种声明形式**（`test(` / **`testWidgets(`** / **`group(`**），而我多批只 grep 了 `test(`
- ⭐ 而**漏掉的那些恰好是价值最高的一类**：「**expanded 靠的是 onEnsureSectionLoaded**（不是靠换分区）」「**纯文本消息不进 `Text.rich`**」「**读屏语义保留**：标签去掉不等于身份信息丢掉」「10 次宿主重建：…**ImageCache 条目数不增长**」等
- ⇒ **B0178 的结论基于 52% 样本**；新露出的那一半**在那三点上密度更高** ⇒ **结论成立且更强**，**但必须说清：当时只有一半数据，是现在补齐的**
- ⭐ **自我设防测试名 86 → 208**（2.4×，占 13%）；新增含**多条真正的并发/一致性不变式**：⭐「**GET / PATCH 乱序：响应回来得晚也不能覆盖新状态**」**（旧口径从未见到）**·「DEC-6 运行时上下文由外壳**注入**（不是设置页自己猜）」·「A5 编辑 base_url/voice/model **只落进 tts 草稿，不碰别的段**」（与 F-0060-01 同域）·「Mod 未启用：主界面**常驻红字**（不是只说「识别了」）」·「EmphasizedText：画出来的是**粗体，不是星号**」
- ⭐⭐ **第三次被自己的机械检查抓住系统性盲区**（① 模式 Q → 账本分叉 ② 覆盖率自检 → **差 5 倍** ③ 本次 → **枚举漏 48%**）⇒ **共同形状：每一条「我用 grep/脚本得到的事实」都需要独立检查问「你确定吗」**；**触发点最便宜的一次是 B0186 顺手核一个文件**
- ⇒ ⭐ **工具侧教训**：**审计工具的错误不会自己暴露；它只会安静地少报** ⇒ **规则：每次用 grep/脚本得出覆盖面结论后，都要问「这个模式的漏报率是多少、怎么估的」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0188（1 文件 / 定点读 · **0 新发现** · ⭐ 立正面模式 P20）
- ⭐ `settings_controller_test.dart:565-567` → 第二次 GET 的 body 被 `replaceAll('deepseek-flash','later-model')` 改过，注释明写「**用来分辨「哪一次的响应生效了」**」；序列 **load→edit→save→load** 让第一次响应**可能落在 save 之后**
- ⇒ ⭐ **正面模式 P20（新）**：**「乱序/迟到响应」的不变式测试必须让两次响应可区分** —— 否则同值 ⇒「谁生效了」不可观测 ⇒ **该测试对它要防的 bug「结构上无法失败」**（**模式 H 假绿的变体，且最容易被顺手做错**：复用同一 fixture 响应）
- ⇒ 与 **P7 最强形态同族第三次**（B0174 数**次数** · 本批让**内容**可区分）⇒ 合起来是「**并发/时序类不变式必须可观测**」
- 顺带：`:591` 把**不变式与 UI 义务**绑进同一测试名（「load 会丢弃未保存草稿（**调用方必须先问用户——见 `confirmLeave`**」）⇒ 与 B0040 核过的 `confirmLeave` 三处共用一入口**接上了**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0189（3 次只读核验 · ⭐ **P2 1** · 19 批以来第一条新发现）
- ⭐ **F-0189-01（P2）** `settings_controller.dart:261-277` → `load()` **无 staleness 守卫**：`grep "epoch|_gen|generation|seq|stale|inflight"` **零命中**；`:266` `_remote = await api.fetchSettings();` **无条件覆盖**
- ⭐ **触发路径已核**：`shell_settings.dart:224` `onPressed: () => unawaited(_settings.load())` ⇒ **按钮 + 不 await** ⇒ **连按两次即产生重叠**（`:18` 初始化 / `:108` 另一处）
- ⭐ **而声称守它的那组测试是纯顺序的**：`test/settings_controller_test.dart:559` 组名「GET / PATCH 乱序：响应回来得晚也不能覆盖新状态」，而体是 `:579 await c.load()` → `:583 await c.save()` → `:586 await c.load()` ⇒ **全程无重叠** ⇒ `:587` 断言**顺序执行下恒真**
- ⇒ **一个被写进测试名的并发不变式，既无实现守卫、也未被测试** ⇒ 与模式 H 假绿**同形但方向相反：不是断言写错，是断言太弱**
- 与本仓**真实事故同域**（AGENTS：手改 toml → `GET /settings` 不跟随 → 下次保存覆盖）——那次修的是**磁盘跟随**与**草稿丢弃**两半，**「两个 GET 乱序」这一半两样都没有**
- 建议：① `load()` 入口加自增序号 + `await` 后判 `my != _loadSeq` 早退（**与 B0040 的 epoch 闸同形**，一行级、不需取消请求）② 补**真正制造重叠**的测试（第一次 GET 延迟）③ `:224` 按钮在 `_loading` 期间禁用
- ⚠ **诚实说明为何 P2 而非 P1**：**未核** `save()` 的 patch 由 **`_draft`** 还是 **`_remote`** 派生 ⇒ **若由 `_remote` 派生则升 P1**（B0190 第一件事）；反证 (a) `load()` 只**写** `_loading`、入口无早退 (b) `ApiClient` 内部**可能**有串行化
- ⚠ **模式 Q 第四次复发**（B0128/B0144/B0184/本次）⇒ **四次都发生在「刚想到好发现、急着写下来」时** ⇒ **补一条执行提示：新发现先写规范头、正文随后**（头是索引、正文可长）——顺序反过来就会漏头
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0190（2 次只读核验 · **0 新发现** · ⭐ 结清 F-0189-01 的未决点）
- ✅ **升 P1 的条件被排除**：`save()` :281 `final SettingsPatch patch = _draft.toPatch();` ⇒ **由 `_draft` 派生、不是 `_remote`** ⇒ **过期 `_remote` 永远到不了线上** ⇒ **不存在「旧响应 → 保存 → 写回旧值」** ⇒ **F-0189-01 维持 P2**
- ⚠ **而真正的危害比我原记录更重**：`grep "_draft = "` ⇒ **`load()` :267 是无条件执行** ⇒ **一个迟到响应会直接丢掉用户未保存的草稿**（用户只按了一次按钮，只是第二次慢）
- ⇒ 而 `save()` 头注写着「失败时**保留草稿**」⇒ **本项目自己在意不丢草稿** ⇒ **被迟到响应丢掉与他们的口径冲突** ⇒ 这比「界面显示旧值」**更接近数据丢失**
- ⇒ **危害陈述改为**：① **迟到响应丢弃未保存草稿**（主）② 面板显示较旧的值（次）；**修法范围明确**：守卫须**同时**保护 `_remote` 与 `_draft`**（只挡一半挡不住真正的危害）
- 顺带把「设计行为」与「副作用」分开：B0188 那条测试把「load 丢弃草稿」钉为**设计**，但那是**用户主动**的顺序 load；对**迟到**响应是**副作用** ⇒ **两者必须分开**，否则「设计如此」会变成「bug 是特性」的挡箭牌
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0191（1 文件 / 定点读 · **0 新发现** · 反证 (b) 被推翻）
- ⭐ `api_client.dart:177-183` → `fetchSettings()` 是 `await _guard(() => _client.get(...))` **裸发**：**无队列、无序号、无 in-flight 判据** ⇒ **客户端层没有消解这个竞态**
- ⇒ **F-0189-01 的范围维持**：两个重叠 `load()` 仍会乱序到达，后者覆盖前者并**丢弃草稿**；**反证 (a) 仍开着**（`load()` 只**写** `_loading`、入口无早退 ⇒ **未核** `:224` 按钮是否在 `_loading` 期间禁用）
- ⭐ 顺带一处**正面样本**（`api_client.dart:185-188`）：「**是 PATCH 不是 PUT**：服务端 `match_route` **只认 `Method::Patch`**，用 `PUT` 会 **404**（**实测** `PUT→404` / `PATCH→200`）」⇒ **三要素齐备**：用哪个动词 · 为什么 · **实测证据**
- ⇒ 这是 **P1 的强化版**：**不只说「会 404」，还把两次观测的结果记下来** ⇒ 日后有人「顺手统一成 PUT」时**注释本身就是反证**；而它防的正是**协议漂移**（前后端对同一端点的方法认知不一致）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0192（1 次只读核验 · **0 新发现** · ⭐ 两条反证全部结清）
- ⭐ **反证 (a) 推翻**：`shell_settings.dart:224` `onPressed: () => unawaited(_settings.load())` **永远非空**（无 `loading ? null : …`）⇒ **连点两次即两个重叠 `load()`**
- ⇒ 且**上下文让它更容易发生**：该按钮在**「读不到服务端设置」的界面**（:215）⇒ **用户恰在「以为没反应」时连点「重试」**——这是最可能连点两次的动作
- ⚠ **危害收窄一格**：`load()` 的 `_draft = SettingsDraft()` **在 `try` 内的成功路径**（:267），两个 catch（:269-272）**只设 `_error`** ⇒ **失败响应既不覆盖 `_remote` 也不重置 `_draft`** ⇒ **「丢草稿」只发生在两个都成功的 load 乱序时**；「显示旧值」任何乱序都可能
- 两条反证最终状态：**(a) 推翻**（本批）· **(b) 推翻**（B0191）· **(c) 连按两次是否常见 —— 仍无法从仓库证明** ⇒ **维持 P2**
- ⏰ 未核：`:108` 那处 `await _settings.load()` 的**触发方式**（是否也是可连点按钮）⇒ 决定主路径的**现实性**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0193（1 次只读核验 · **0 新发现** · ⭐ 主路径现实性**上调**）
- ⭐ `shell_settings.dart:108` 位于「**保存本模型覆盖**」之后 ⇒ **不是按钮**，是「**保存 ⇒ 回读**」的尾巴 ⇒ **每次保存覆盖都触发一次整份回读**
- ⇒ ⭐ **重叠不需要「连点两次」**：`打开设置 → init GET(:18)` **与** `改覆盖→保存→GET(:108)` **天然交叉** ⇒ **「打开设置后立刻改东西」即可触发**（正常操作）⇒ 编辑越多、GET 越多、重叠机会越多
- ⭐ 同一处 `:99/:107/:109` 的 `if (!mounted) return;` 说明**异步纪律存在、只是漏了 staleness** ⇒ **缺的不是异步意识，而是「两次异步的结果谁作数」这一个判据**
- ⇒ **修复定位更清楚**：**不需要取消机制**（其心智模型里没有），**只需一个自增序号**（与 B0040 的 `epoch` 闸同形、仓里已有先例）
- ⚠ **仍定 P2**：t0（init GET 的位置）是**推断**（`:18` 上下文未读）⇒ **反证 (c) 未被证明，但已不再依赖「连按两次」这个前提**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0194（2 次只读核验 · **0 新发现** · ⚠ 修正 B0193 的高估）
- `shell_settings.dart:15-17` → `_ensureSettingsLoaded()` 有 **`if (_settingsLoadedOnce) return;` 一次性守卫** ⇒ init `load()` **只发生一次**（首次进入设置面）
- ⇒ **它不能**作为**后续任意** `:108` 回读的搭子（除非第一个动作就是保存覆盖且 init GET 仍在飞）⇒ ⚠ **B0193 那句「重叠不需要连点两次、是正常操作」高估了，如实修正**
- ⇒ **修正后最现实的触发**：**① 连续保存两次覆盖**（每次都触发 `:108` 的整份回读、**无按钮参与**，现实性最高）② init GET 在飞时立刻保存覆盖 ③ `:224`「重试」连点两次 ⇒ **发现本身不变（无守卫/可乱序/可丢草稿），只是主路径换了**
- 顺带正面：`:12-14`「**不放在 `initState`**：这些数据多数时候用不到，**启动就打三个请求是白花钱（也拖慢首屏）**」= **P1**（理由具体到**首屏延迟**）；三个请求用 `Future.wait` **并发**且失败**分别**处理，与 `:108` 的「PATCH 完再**串行**回读」形成对照 ⇒ **两种时序都有明确理由**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0195（1 文件 / 定点读 · **0 新发现** · ⭐ F-0189-01 调查收口）
- ⭐ `api_client.dart:229-235` `_guard()` **只有 5 行、只做一件事**（把传输异常翻成 `ApiException`）⇒ **无超时/无重试/无去重/无串行化** ⇒ **反证 (b) 不是「没找到」而是「彻底排除」**：客户端层**不提供任何能给两个并发 GET 定序的机制**
- ⇒ ⭐ **F-0189-01 全部未决点结清**（B0189–B0195 共 5 批）：实现无守卫 · 测试顺序（对该主张**结构上无法失败**）· 触发已排序 · (a) 推翻 · (b) **彻底排除** · (c) 无法证明（**维持 P2**）· 危害 = **两个成功的 load 乱序 ⇒ 丢弃未保存草稿**（主）+ 显示旧值（次）· 修复 = **一个自增序号、不需取消机制**
- ⭐ 顺带：`_decodeObject`(:237-245) **坏 JSON ⇒ 返回空 map、不抛** ⇒ 与表演层「失败 ⇒ 回落 + 计数」（B0151 的 **P9**）**同形** ⇒ **降级不留异常给 UI**；`_errorFrom` 从 `data['error']` 取详情 ⇒ 与 B0015/B0161 的「前端从码/字段分流、不猜文案」一致
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0196（1 文件 / 头注+类结构+测试面 · **0 新发现**）
- `dev_tools_section.dart:1-7` 头注**在有人问之前**就给了不拆分的结构性理由：「共享同一套『列表+动作+内联结果』的骨架，**拆成四个文件只会让同一段列表渲染代码出现四遍**」+「每个类都短、职责单一，找起来**靠类名**」
- 核验**成立**：类确实短而单一职责（`AdminRow`/`AdminEmpty`/`ModelsSection`/`_ImportModelField`/`ModsSection`/`_ModConfigTile`/`DiagnosticsSnapshot`/`DiagnosticsSection`）· 私有 widget 仅 3 个且**各绑一个分区** ⇒ **文件内无共用基类**，**但头注也从未这么声称**（它说的是**形状**）
- 真正共享的逻辑在**文件级**：`String modStateLabel(...)`(:378 顶层函数) ⇒ **状态标签集中一处**（P2 的文件内形态）；**24 处列表渲染点** ⇒ 形状在文件内重复 ⇒ 与头注自洽（**文件合一收敛** + 形状重复**可见**，好过四文件各抄一遍**不可见**）
- ⭐ 该文件有 **5 个测试文件**覆盖（`restart_notice` / `models_section` / `pet_desktop_state` / `admin_api` / `mods_section`）⇒ 四个分区**各有对应测试**
- ⚠ **我这一批两次猜名都错**（规则 3 变体 6 用在自己身上）：按「应有共用基类」的预设 grep ⇒ 零命中 ⇒ 差点记「头注不实」⇒ 而头注说的是**形状**、我核的是**基类** ⇒ **可执行规则：核「注释声称」时，先把注释里的词换成代码里真实存在的名词再 grep**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0197（1 文件 / 定点读 · **0 新发现** · ⭐ 红线 R 第六形态）
- ⭐ `dev_tools_section.dart:1178-1190 _snapshotLines()` → 快照全是**非敏感标量**（ws 标签 · app/version/schema/ws_proto · `active_model` **id** · epoch · uptime · audio/llm/tts/dev_mode）
- ⇒ **`llm`/`tts` 来自 `status[...]`，而那个 status 对象本身已被脱敏**（B0123 已**穷举**核过 9 个 section 全部 `api_key_env`→`has_api_key`）⇒ **快照不可能带出密钥**，且**不是靠在这里脱敏**，而是**只拼装已脱敏的源**
- ⭐ ⇒ **红线 R 六形态表补完**，第 6 档（诊断快照·可复制到剪贴板）**最强**：前五档是「**在这里不泄露**」，第六档是「**这里根本拿不到**」
- ⭐ `:1193-1194`「复制到剪贴板（**复制是唯一的导出方式，v1 不做上传**）」⇒ 导出面被**显式限定**、**无第三方出口**
- 两处刻意取舍（正面）：`:1140`「需先开『开发模式』；只读最后一屏，**不做实时跟随**」（成本+简单性都明写）· `:1157`「**必须虚拟化**：日志是长期运行会累积的东西」+ `ListView.builder`+`itemExtent:20`+固定高 220 ⇒ **长期累积列表必须虚拟化**且**理由写在前面**（P3 同形）
- ⚠ 未核：**`logs` 的来源与是否脱敏**（是否与 `dispatch.rs`「不记录请求体」同一份）⇒ B0198 第一件事
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0198（4 次只读核验 · **0 新发现** · 日志内容面）
- `log_routes.rs:86` → `read_tail_lines(file_path, MAX_LINES)` **逐字转发**日志文件尾部（「读失败 = 空，**不暴露 IO 错误给前端**」）⇒ 加上 B0197 的**可复制到剪贴板** ⇒ **内容面是红线 R 在「非密钥」维度上的出口**
- ⭐ **主链零文本**（两条 grep 各验证）：① 结构化字段 `text=|prompt=|content=|message=|assistant|user_text` 在**全 workspace** 的 `tracing::` 调用里**零命中** ② **生产 `tracing::` 共 69 处**，带变量的逐条看过：`env_routes.rs:150` 只记**键名**（「值不记录」）· `dispatch.rs:250/252/254` 只有 `%method, path, route, status`（**无 body**）· `supervisor.rs:458` 错误+路径 · `mod_registry.rs:377` 只有 id
- ⇒ **主链（LLM/TTS/WS/设置/环境）日志只含标量与状态码，不含用户或助手文本**
- ⚠ **两条经 Mod 通道的候选路径（未闭）**：`mod_registry.rs:402` `ModLogger::new(|_lvl,msg| tracing::info!("{}",msg))` ⇒ **Mod 打印什么就进宿主日志什么**（与 B0103「Mod 日志**透传**」同源）；`:399` 直接落 **`payload`**（若是 `say_tx {text}` 就含用户文本）
- ⇒ **未记发现**：**我未核任何 Mod 是否真的打印文本**（未证实）；且 **Mod 打印什么由 Mod 作者决定、宿主无法在 logger 层做内容级脱敏** ⇒ 记为未核实：**逐个 Mod 检查**（B0199 第一件事）
- ⭐ 而这呼应 B0103：**Mod 日志透传是「设计」（便于排障），代价是宿主失去对内容的控制** ⇒ **是一次自觉的取舍、不是疏漏** —— 但**取舍要被写下来**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0199（2 文件 / 定点读 · ⭐ **P2 1** · 本审计第一条「内容面」发现）
- ⭐ **F-0199-01（P2）** `external-input/src/lib.rs:247` → Mod 把 **`TextDelta` 的 payload（= 助手正文块）整块**写进 Mod 日志（它只订阅 `TurnStarted`/`TextDelta`，:234-235）
- ⇒ **四跳链路每跳核到行**：① `:247` 整块 payload 进 Mod 日志 → ② `mod_registry.rs:402` `ModLogger` **透传**进宿主日志文件 → ③ `log_routes.rs:86` **逐字转发** → ④ `dev_tools_section.dart:1139-1165` 诊断面板**显示**
- ⭐ **核心是「同仓两种口径」**：`dispatch.rs:250/252/254` 刻意**不记 body**（合 AGENTS「不记录请求体」）· 而 Mod 整块记 + 宿主透传 ⇒ **Mod 绕过了一条宿主已立的规矩**
- ⚠ **我先猜错了、已改正**：第一反应「payload 是**用户弹幕文本**」⇒ ❌ 该 Mod 只订阅 `TurnStarted`/`TextDelta` ⇒ 是**助手正文**；**弹幕走端点、不作 Mod 事件** ⇒ **用户输入不走这条路**（这是定 P2 而非 P1 的理由之一）
- ⚠ **另一条自我修正**：先以为「日志会经**复制快照**出去」⇒ ❌ `_snapshotLines()` **只含标量**（B0197 核）⇒ **泄漏面只有「显示」，没有「剪贴板」**
- 修法：① `:247` 改记 `payload.chars().count()`（**类型与规模**才是排障所需）② `mod_registry.rs:402` 提供**只记 `topic`+`len`** 的通道，或**把「Mod 日志原样进宿主」这条代价写下来** ③ 若保留正文 ⇒ 考虑给日志文件 `.env` 同样的 `0600`（B0120 已有 chmod 先例）
- ⏰ 未核：**director 的 `format!` 参数含 `{payload}`** ⇒ 同一形态可能存在（B0200 第一件事）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0200（1 文件 / 定点读 · **0 新发现** · ⭐ 修法被「定死」）
- ⭐ `mod-director/src/lib.rs:711` **先算 `len = payload.chars().count()`**、`:712` payload 传给**分析器**（不是日志）、`:719-720` **只记 `len={len}`** ⇒ **director 不记 payload**
- ⇒ ⭐ **那正是我建议的修法 ①** ⇒ **这不是设计问题，是「让 `external-input` 做 director 已经做的事」** —— **同仓 · 同 Mod 模式 · 两个 Mod · 相反的选择，而正确的那个已在树里**（我能给出的**最强形式的修法建议**）
- ⚠ **据此降权我自己的修法 ②**（在 `mod_registry.rs:402` 的 `ModLogger` 层统一收口）：director 证明 **Mod 自己就能选对** ⇒ **宿主层强制 = 过度设计** ⇒ ② 降为「可选：把约定写进 `ModServices` 文档」
- ⭐ 顺带（**P2 边界**）：`ModLogger` 签名**不区分「记标量」与「记内容」**，而 `len=` 表明**约定可学且已被遵守过一次** ⇒ **不需要改 API**；**再次印证 P2**（B0152 的 `join_endpoint` 是「有理由的重复」，这里是「**重复的是调用方选择、不是实现**」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0201（7 个 Mod / 50 处日志 · **0 新发现** · ⭐ 范围定为「孤例」）
- 另 6 个 Mod 的日志处数：`persona` 12 · `memory` 27 · `voice-input` 5 · `wallpaper` 3 · `pet-desktop` 1 · `template` 2 · `mod-system` 0 ⇒ **共 50 处**
- ⭐ 逐处查参数**零处携带内容**：`memory` 具名参数全集 `{covers}{e}{id}{pct}{reason}{removed}{role}{session}{total}{turn}` **全非内容** + 45 处 `{}`（静态文案/已算好的值）⇒ **`payload`/`text`/`prompt`/`content` 零命中**
- ⚠ 两处**疑似**命中**已逐一定位、均非内容**（**不凭形状下结论**）：`voice-input/commands.rs:105` 的 `message` 是**固定状态串**（:95/:101）且**只在被拒时**记（:104 `if !accepted`）· `persona/sessions.rs:270` 是**文件路径**
- ⇒ **F-0199-01 是「一个文件里的一个站点」的孤例** ⇒ **修法的一行性质更明确、发现的精确度更高**（不是「一类问题」是「一处」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0202（1 测试文件 / 枚举用例名 · **0 新发现** · ⭐ 立正面模式 P21）
- ⭐ `design_tokens_test.dart` **40 个 `expect`，无一是「能不能渲染」** ⇒ 全部是**结构 / 相干性**断言：「**登记表长度与预期一致**（新增档位必须显式改测试）」·「取值**两两互异**（防四档同值）」·「**字号+字重**无冗余槽位」·「**全阶梯只有一个** 16 px（**治旧的『两个 16px 同级标题』病**）」·「字号**下限 11 px**」·「声明 ↔ 引用**双向对账**」·「未被引用的**必须在未接线台账里**」·「台账**不得含过期条目**（只减不增）」·「台账名字**都真的存在**（防改名/删档后陈旧）」·「ThemeExtension 结构枚举（防**加了字段忘了 copyWith / lerp**）」
- ⇒ ⭐ **正面模式 P21（新）**：**「登记/声明」与「引用」并存 ⇒ 建双向台账 + 给台账本身加三条约束**（名字必须真实存在 · 不得含过期条目 · 未被引用的必须显式登记）+ **成对互异/唯一性/下限** ⇒ **把设计系统的「品味规则」变成可执行断言**
- ⭐ **一处意外的收敛**：P21 第 ③ 条「未被引用的必须显式登记」与我在 B0129 建的 **`KNOWN-LOSSES.md`**（「已知的丢失必须**显式记录**，而不是靠正则藏掉」）+ 模式 Q 的机械检查 **同形** ⇒ **两边独立演化出同一种结构** ⇒ 这条模式是「声明 vs 引用」类问题的**自然解**，不是团队偏好
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0203（1 测试文件 / 2 条断言体 · **0 新发现** · ⭐ 最好的一条测试注释）
- `:248-249` 互异断言形状正确：`expect(values.toSet().length, values.length)` ⇒ **全部互异**、**能失败**（不是假绿）
- ⭐ `:239-241` **规则本身也有作用域，且写明 + 交叉引用**：「注意**不含** `AppFontSizes.sizesOf`：`bodyMedium` 与 `titleSmall` **同为 14 px**（w400 正文 vs w600 小标题）**这是有意的**……判据**见下一条**」
- ⇒ **照字面执行会让一条正确的设计被判违规** ⇒ 他们**没有改那个合法的值**，而是**排除该表 + 写明原因 + 指向接管者**（下一条 `:253` 按 **`(size, weight, lineHeight)` 元组**判互异 ⇒ 两个 14px 因字重不同而合法）
- ⇒ ⭐ **正面模式 P22（新）**：**一条规则要写明「它不适用于谁、为什么、以及谁接管」** —— 否则后来者只看到「有例外」，可能**为了让测试变绿而改掉一个正确的设计**
- ⇒ 与 B0165 核的 `SYSTEM_JSON_ONLY`「三层同形」同族：**提示 / 规则 / 测试** 三处对同一件事的表述必须一致、且**各自的作用域要写清**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0204（1 测试文件 / 2 条断言体 · **0 新发现** · ⭐ P21 核到实现）
- `:302-305` 过期条目检查：判据 `refs[n] > 0` ⇒ **已接线必须删** ⇒ 台账**不能只增不减**；⭐ `reason` **把违规名字插进失败文案**（「请从 kNotYetWired 里删掉：$stale」）⇒ **P1 用在失败文案上**
- ⭐ `:309-316` 名字存在性检查：**声明集是从 `families` 推导的**（`for e in families … '${e.key}.$n'`）⇒ **没有第二份清单要同步** ⇒ **这个检查本身不会漂移** ⇒ **P2 用在测试上**（真源只有 `families`）
- 顺带：`refs` 是**引用计数**而非布尔 ⇒ 同一遍扫描既知「有没有被引用」也知「几次」⇒ **计数比标志多给信息**、两条约束共用一次遍历
- ⇒ **P21 三条约束全部核实**（名字必须存在 B0204 · 不得含过期条目 B0204 · 未被引用的必须显式登记 B0202）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十五次对账（CONSOLIDATION-15）· ⭐ P21/P22 立档 + F-0199-01 三批收窄复盘
- **17 条 P1/P2 复核：17/17 仍成立**（含新增 F-0189-01）；【模式 Q + KNOWN-LOSSES】**✔ 通过**
- ⭐ **一条 P2 怎样被「查到只剩一行」**（F-0199-01，3 批每步都缩小它）：B0199 读到整块记 `payload`（并**猜错内容**）→ B0200 发现 **`director:711` 先算 `len`、只记 `len={len}`** ⇒ **修法被「定死」**（照 director 做那一行），**我自己的修法 ② 被降权为过度设计** → B0201 查另 **6 个 Mod、50 处** ⇒ **孤例确定**（两处「疑似」逐一定位、均非内容）⇒ **级别始终 P2**
- ⭐ **正面模式 P21**：「登记/声明」与「引用」并存 ⇒ **双向台账 + 台账三条约束**（名字必须真实存在 · 不得含过期条目**只减不增** · 未被引用的**必须显式登记**）+ **成对互异/唯一性/下限** ⇒ **把设计系统的品味规则变成可执行断言**（`design_tokens_test.dart`，**40 个 `expect`、无一是「能不能渲染」**；三条约束**全部核到实现**，且**声明集从 `families` 推导** ⇒ **该检查自身不会漂移**）
- ⭐ **正面模式 P22**：**一条规则要写明「它不适用于谁、为什么、以及谁接管」**（`:239-241` 排除 `AppFontSizes.sizesOf`、写明「同为 14 px **是有意的**」、指向下一条按 **`(size,weight,lineHeight)` 元组**判互异）⇒ **否则后来者会为了让测试变绿而改掉一个正确的设计**
- ⭐ **一处意外的收敛**：P21 第③条「未被引用的必须显式登记」= 我的 **`KNOWN-LOSSES.md`**（「已知的丢失必须**显式记录**，而不是靠正则藏掉」）= 模式 Q 的机械检查 ⇒ **令牌台账与审计台账独立演化出同一种结构** ⇒ **这不是团队偏好，是「声明 vs 引用」类问题的自然解**
- 覆盖率（301 条去重路径）：Rust `.rs` **79** · 前端 `.dart` **35/218** · CI/脚本 4；**已读完**：`live2d-ai-runtime` 17/17 · `web_api` 48/48 · turn 管线 100%
- ⭐ 自评：**本轮最常做的事不是「找到问题」，而是「发现自己上一批说宽了/说错了」** —— 四次里**三次是我自己上一批的话** ⇒ **「发现自己说宽了」比「找到新发现」更频繁，也更不容易被计入任何指标**

### BATCH-0206（1 测试文件 / 定点读 · **0 新发现** · ⭐⭐ 最自我审视的一段测试注释）
- ⭐⭐ `design_tokens_test.dart:73-76` → **他们发现并诊断了自己那道假绿门禁**：「旧实现只匹配 `AppColors.hairline` ⇒ 这 **9 个令牌的引用数永远是 0**：『声明↔引用双向对账』**在那条路径上恒真**，`kNotYetWired` 台账里**已接线的条目也永远不会被判成过期** —— **门禁在假装工作**」
- 修法：认**三种访问形式**（`Family.name` · `appColorsOf(...).name` · **先绑局部再用** `alias.name`）
- ⭐ `:83-84` **P22 的第四例**：「靠**类型标注**识别别名，**不做类型推断** —— 只扫 `lib/**`，且**别名写法在本仓库只有 `colors` 一种，精确够用**」⇒ **不假装精确** + **把「让它够用的假设」写在注释里** ⇒ 日后引入第二个别名写法时，**注释会告诉他这里会漏**
- 顺带：`kNotYetWired` 定义在**测试**里（:179）而非 `tokens.dart` ⇒ 「哪些令牌还没接线」是**测试侧维护的事实**、生产代码不需要知道（P2 的分层）
- ⭐ **对本审计也是一条证据**：本审计最常见的发现类别就是**模式 B（假绿）**，而这里**项目自己发现了自己的一道假绿门禁**、写清机制、修好、并把**修法的边界与假设**也写下来 ⇒ **这是个「测试会被复审」的仓库**；也正因如此，我用「声明侧 vs 引用侧双向对账」去找 F-0184-01 那类问题时，**这些门禁本身没给我假绿**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0207（1 测试文件 / 1 条断言体 · **0 新发现** · ⭐ 立正面模式 P24）
- ⭐ `design_tokens_test.dart:351-368` → 构造 **12 个「只改一个字段」的副本**，然后 ① `:365` `expect(singles.length, AppColors.fieldCount)` ② `:367` 逐个 `isNot(base)`
- ⇒ **Flutter 最经典的两个 `copyWith` bug 各被一层覆盖**：**漏**（加第 13 个字段而 `copyWith` 没处理）⇒ **计数对账**变红 · **错**（字段接错到别的字段）⇒ 该副本**等于 base** ⇒ `isNot(base)` 变红；另一条「ThemeExtension 结构枚举」覆盖**枚举面** ⇒ **枚举 + 计数 + 逐字段效果**三层
- ⇒ ⭐ **正面模式 P24（新）**：**把「新增一个成员」变成一次必然失败** —— 把测试里的成员数与代码里的 `fieldCount`/`registry.length` **对账** ⇒ 「加了字段却忘了在某处处理」**必然变红**，**不需要任何人记得去加断言**
- ⇒ 与 P21 第①条（登记表长度与预期一致）**同源**、与 B0194「新增一个 load 的守卫」**同族** ⇒ **让「扩展」这个动作自带绊线**
- 顺带 `:371-374` 是**第三处**计数对账（「`toValuesMap` 字段数与 `fieldCount` 一致……一个都不能少、也不能多」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0208（1 文件 / 定点读 · **0 新发现** · 三个独立锚点）
- ⭐ `tokens.dart:572-596 AppColors.lerp` → **按字段类型分两种插值**：10 个 `Color` 用 `Color.lerp(a,b,t)!`、2 个 `double` 用 `a + (b-a)*t`；**12 个字段一个不缺**
- ⇒ ⭐ **这个分法不是风格、是语义必需**：`Color.lerp` 签名是 `Color? lerp(Color?, Color?, double)` ⇒ **对一个 `double` 调它编译不过** ⇒ **类型系统在这里帮忙**（两个家族的字段不可能写错插值方式）
- ⇒ 另记：那 **10 个 `!` 不是掩盖真 null**（入参是非空类型 ⇒ `Color.lerp` 不返回 null）⇒ **免得日后被误当成「危险的强制非空」**
- ⭐ `:598` `/// **供测试做结构枚举**：字段名 → 值` `toValuesMap()` ⇒ **测试的枚举源与字段声明同处维护** ⇒ **不会漂移** ⇒ **P2 用在生产/测试边界**（不是「测试抄一份清单」，而是「**生产把那份清单以 API 形式交出来**」）
- ⇒ 与 B0207 合起来 **`copyWith` 有三个独立锚点**：**结构枚举**（挡「加字段忘了枚举」）· **计数对账**（挡「加字段忘了进 copyWith」）· **逐字段效果**（挡「字段**接错**」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0209（2 文件 / 交叉核 · ⭐ **P3 1** · 文档落后 13 天）
- ⭐ **F-0209-01（P3）** `display_prefs.dart:152` 自述「**为什么换掉 `syncShellStageBg`**（2026-09-27 **修一个真缺陷**）」⇒ 新机制是 **`backgroundSource` 枚举**、旧键**只作迁移输入**(:654)、**新默认是「背景库」**(:643「也就是新默认」，而「绝大多数用户」落在这一支)
- ⇒ 而 **AGENTS.md §0.1.0-rc.5 仍写**「`syncShellStageBg`（**默认 `true`**）……`effectiveShellImage` 是一份真相」⇒ **照文档理解的人会以为「壳默认跟随舞台」**
- ⇒ **时间可核**：AGENTS.md 变更历史**最后一条是 2026-09-14** ⇒ **无更新条目能覆盖 09-27** ⇒ **漂移确认**（非「另有新章节」）
- ⚠ **但这是「实现很好、文档落后」⇒ 定 P3 不定 P2**：`:645-646` 把**两种做法的差别与原缺陷成因**写进代码（「改默认值会让『显式设过 true』和『从没设过』分不开，**而那正是本缺陷的成因**」）· `:661-665` `effectiveBackground`「**判据只有这一处**……判据散到两处就会出现『设置说用背景库、画的不是背景库』」= **P2** 又一例 ⇒ **代码里每一处都写明了，漂移只在 AGENTS.md**
- ⭐ 附带独立观察：**AGENTS.md 是任何 agent（含本审计）最先读的文件，而它落后 13 天** ⇒ 「文档比代码旧」本身值得作为一条独立观察跟进
- `wallpaper` 侧 **0 条新发现**且口径对齐：`:212`「壁纸策略：壳跟随舞台（**等价 `DisplayPrefs.syncShellStageBg = true`**）」= **Mod 在决策点直接点名等价的前端字段**（P13 同族）· `:15`「只回答何时换/要不要同步，**不画图**」= 决策与渲染分离 · `:110` 缺省 `off` ⇒ **不接管** ⇒ 与前端新默认**不冲突**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0210（3 次只读核验 · **0 新发现** · 🔻 撤回自己上一条的核心论据）
- ⭐ `grep -oE "2026-09-[0-9]{2}" AGENTS.md | sort -u | tail -3` ⇒ **09-26 / 09-27 / 09-28**；而**代码侧最晚的日期标记也是 09-28** ⇒ **两者同步**
- ⇒ 🔻 **我 B0209 说的「AGENTS.md 落后代码 13 天」是错的** —— **错的只是「变更历史」那一节**（最后一条 2026-09-14）⇒ 应改为「**变更历史一节落后 14 天、正文并未落后**」
- ⇒ 而 **F-0209-01 本身仍成立**（收缩后）：AGENTS **正文** `:123` / `:641` **仍把 `syncShellStageBg` 写成现行机制**（默认 `true`）
- ⭐ **我同一形态上的第 4 次犯错**：B0104 凭局部印象断言「唯一」· B0186 grep 零命中差点断言「没有测试」· B0196 按预设找基类差点断言「头注不实」· **本次读了一节、推广到全文**
- ⇒ **教训升级**：**「某节落后」不能推出「全文落后」** ⇒ 要推广任何结论，**必须先机械核一遍全文**（成本一次 grep）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0211（3 次只读核验 · **0 新发现** · 🔻 撤回一条 + 扩大一条）
- 🔻 **撤回 F-0040-01（P3）—— 审计的第二次撤回**：`grep -n "mod_count_is" AGENTS.md` **只命中 `:45/:46/:73`** 且**三处都写 `mod_count_is_five`/`mod_count_is_seven`** ⇒ **全文已无 `mod_count_is_three`** ⇒ 与代码一致（`main.rs:471`）⇒ **该文档缺陷已被修复**
- ⇒ ⚠ **连带撤回 B0170 写的「影响升级」**（「文档指向了不存在的符号」**已不成立**）
- ⇒ ⭐ **教训**：**一条发现的「对象」可能被修好** ⇒ 复核时**必须先核对象是否还存在**再谈它是否成立 —— **与复核代码缺陷同构**（第一次撤回是 B0069 的 F-0030-01「150ms 防抖我漏了」）
- ⭐ **F-0209-01 范围扩大**：`grep -rn "effectiveShellImage" shell/flutter/lib/` **零命中**（已改名 `effectiveBackground`:665）⇒ **一处机制在 AGENTS.md 里有三处描述、三种都已失效**：① 字段（`syncShellStageBg`→`backgroundSource`）② 默认值（`true`→背景库）③ getter 名（`effectiveShellImage`→`effectiveBackground`）
- ⇒ **不是「一个字段名写错」，是「一个机制整体停留在替换前」**；而这**与 B0210 不矛盾**：**「变更历史那节落后 ≠ 正文落后」**，真正落后的是**正文中对某个机制的三处描述**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0212（机械差集 + 逐条分类 · **0 新发现** · ⭐ F-0209-01 收缩为孤例）
- 机械差集：AGENTS.md 的 **63 个**带点标识符里 **18 个在代码里找不到** ⇒ **逐条分类后零个是真漂移**
- ① **6 个是我的正则把「文件名」切开**（`client.rs`/`plan.rs`/`prompt.rs`/`normalize.rs`/`gpu.rs`/`mod_registry.rs` ⇒ **文件都在**）② **2 个不在本仓代码**（`CONTRIBUTING.md`/`nightly.yml`；`Element.updateChild` 是 **Flutter 框架 API**）③ **10 个是 AGENTS 自己以过去时记录的「已拆除/已补过」**
- ③ 逐条：`:49-50`「被拆掉的只是 wallpaper **Mod** 的接线（Flutter 侧 `wallpaper_api.dart`/`shell_wallpaper.dart`/`applyWallpaperPatch`）**一并拆除**」· rc.2「`HostChannels.trigger_action` **删除**」· rc.4「Flutter **删** `persona_card.dart`/`persona_import.dart`」· rc.5「`stage_css.rs` **随之删除**」· `:692-693` 变更历史「模板 + `descriptor.api_version` 门禁」「**补** `ModServices.settings`（脱敏读取）」· `:480` 修复记录「F-0005-2 重建放大链：…`_settingsTick`」
- ⇒ ⭐ **F-0209-01 收缩为「孤例」** —— 不是「文档不随代码走」的一类问题，是「**一个机制被替换后忘了改文档**」的一处
- ⇒ ⭐ **这反而是正面结论**：变更历史里**大量「已删除」都留了记录**（类③ **10 条**）⇒ **没有把删掉的东西从文档里抹掉** ⇒ 与 B0039–B0042 / B0170 核到的「记录推翻过的方案」**同族**
- ⭐ **方法**：**脚本给出差集后必须逐条分类、不能直接计数**（否则会把 18 记成「18 处漂移」）⇒ B0147「脚本先自检」的同族，**再加一条：输出与认知一致时也要问「这些差集项是同一种东西吗」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0213（1 文件 / 定点读 · **0 新发现** · 强正面样本）
- ⭐ `mod-wallpaper/src/strategy.rs:115` `.filter(|f| f.is_finite() && *f >= 0.0)` **在转换前**挡掉 `NaN`/无穷 ⇒ **Rust 里 `NAN as u64` ⇒ 0、`INFINITY as u64` ⇒ `u64::MAX`** ⇒ **若无此 filter，`NaN` 会静默变成 0 间隔** ⇒ **失败被挡在「转换前」而非转换后补救**
- ② `:107-108`「结果钳在 `[MIN, MAX]`（**永不返回 0** ⇒ `interval_ms` **恒非 0** ⇒ **策略里没有除零路径**）」+ `:99-100` `saturating_mul(1000)` ⇒ **两级保证**：钳位（入口）/ 饱和乘（出口）⇒ **下游不必自己再判一次**
- ③ ⭐ `:124-130` `playlist_len` 超上限**明确拒绝「报错」这个选项并给理由**：「**前端多写一个 0 不该让 Mod 起不来**，策略只需知道有几张可切」⇒ **宽容放 Mod 侧、错误留在真的错处**（与 B0149 的 `non_auth_note` 同族：**按「谁能修」决定宽容度**）
- ④ 两个读取器**同口径**，且 `:129` 把「与 `interval_secs_from_config` **同口径**」**写成可核断言** ⇒ **P2 的灵活形态**：不必共享函数，但**不能各写各的**
- ⏰ 待核：**`MIN_INTERVAL_SECS >= 1`** —— 头注的「永不返回 0」**依赖**它（B0214 第一件事）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0214（1 文件 / 3 个常量 · **0 新发现** · 承重行核过）
- ⭐ `strategy.rs:24/27/30` `DEFAULT_INTERVAL_SECS = 300` / **`MIN_INTERVAL_SECS = 5`** / `MAX_INTERVAL_SECS = 86_400` ⇒ **下界 5 > 0**
- ⇒ 头注 :108 的「**永不返回 0** ⇒ `interval_ms` **恒非 0** ⇒ **策略里没有除零路径**」**成立** ✓ ⇒ **B0213 的未核项关闭**
- ⇒ 三个常量**都是具名 `u64` 字面量**、范围合理（5s ~ 1d）⇒ 「**两级保证**」完整：**钳位入口**（下界 ≥ 5）· **饱和乘出口**（不回绕）
- 且「用户填 1 秒会被钳成 5」是**声明过的范围**（settings_spec 应有同一个 `MIN`）⇒ 非缺陷
- ⏰ 下一批第二件事：核 `settings_spec` 的边界声明**是否复用同一常量**（若是两份就会漂移 ⇒ **P21 台账那类问题**，而 B0212 刚证过本仓有双向对账机制）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0215（1 文件 / 定点读 · **0 新发现** · ⭐ 立正面模式 P25）
- ⭐ `mod-wallpaper/src/lib.rs:115-116` `min: MIN_INTERVAL_SECS as f64` / `max: MAX_INTERVAL_SECS as f64` ⇒ **引用常量本身**，不是另写一个 5 ⇒ **改一处，钳位与前端声明同时变 ⇒ 不可能漂移**（`:62-63` 的 import 也确认真源在 `strategy`）
- ⇒ ⭐ **同一约束在四处出现、其中三处是「引用」、只有文档散文（`:24`「5..=86400」）是手写** ⇒ ⭐ **正面模式 P25（新）**：**当同一约束出现在多处时，让「除文档外」的全部都是引用** —— 改一处即可全对；**代价是文档里的数字没有任何机制能跟着变**
- ⭐ 而「看起来会漂移」与「真的会漂移」**成对**了：B0152 `join_endpoint`（runtime/director）**真的两份**（md5 不同）· 本批 `MIN/MAX_INTERVAL_SECS` **只有一份** ⇒ **两者必须分别核**，而我**各有一个样本**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0216（1 文件 / 定点读 · **0 新发现** · 计时器风暴被挡且写明）
- ⭐ `strategy.rs:330` 「**换后重新计时：一次 tick 至多换一张，长挂起也不补播**」⇒ **计时器补播风暴**被挡：若写成 `while (elapsed >= interval) advance()`，进程挂起一小时后**会一次性重放几百张**（CPU/WS 打满）⇒ 这里一次至多换一张 + **重置累加器**；`:326` `saturating_add` 防溢出
- ⇒ ⭐ **「刻意少做」也需要写明**，否则后来者会以为这是 bug 而「修好」它（与 P5「不得静默截断」同族但方向相反）
- `FollowStage` 是**一次性闩**（:308-316 只发一次、不每个 tick 重复写 `DisplayPrefs`），`reconfigure`(:275) **复位同步标志**（B0110 已读 `lib.rs:190`）⇒ 改配置会重新断言一次 ⇒ **行为闭合**；首次 tick **立刻换第一张**（不等一个间隔）
- ⭐ **「列表为空」被挡在策略层**（`tick` 里 `len == 0 ⇒ None`，:314-316）⇒ 投影层**产不出悬空下标** ⇒ **P25 正面实例：不变式在上游强制，下游就不可能违反**
- `:160` `None` 的含义被**反向定义**：「什么都不做（**不是**『假装成功』）」⇒ **P22** 又一例
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0217（1 文件 / 头注 · **0 新发现** · 模板的最高价值不在代码形状）
- ⭐ `mod-template/src/lib.rs:18-20` 头注的「**不要做什么**」**指向别处的执行机制**：「不接动作通道：`ModServices.action_tx` **自 rc.2 起休眠**，请求会被 host 丢弃并**返回 `false`**（见 `core-chain-baseline.md` §3.3）」·「不直接读进程环境/密钥：走 **`apply_settings`（一等 API）**」
- ⇒ ⭐ **它不复述规则、而是指向「规则在哪被执行」** ⇒ **规则一处改动、指向它的读者自动跟着对** ⇒ **模板因此不会过期** ⇒ **P25 在文档层的对应物**
- ⇒ ⭐ 而**「不要做什么」是复制者最容易做错**的部分：**看得见的**（类名/字段/骨架）照抄即可，**看不见的**（「有一条休眠通道看起来能用」「进程环境里确实有密钥」）**只能靠文档说** ⇒ **模板的最高价值不是代码形状，而是「别这么做 + 为什么 + 去哪查」**
- 另两条禁令也带机制：「settings 是**纯数据 schema**…**禁止**注入 HTML/JS」⇒ **面板层有类型背书**（B0105）·「**模板 crate 自身不注册**…注册表里只放真实能力（见 `main.rs` 的 `mod_count_is_*` 断言）」⇒ **连「别注册自己」都写了**
- 使用步骤也是五步清单，每步都有守它的机制（根 `Cargo.toml` → desktop 依赖 → `AVAILABLE_MOD_FACTORIES` → **数量/id 断言** → 文档一行）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0218（3 次只读核验 · **0 新发现** · ⭐⭐ 修正我自己上一批的撤回理由）
- ⓘ 我 B0211 写「**全文已无 `mod_count_is_three`**」—— `grep -c` = **2** ⇒ **该断言不成立**
- ⭐ 而**真实理由更根本**：`:105`「`mod_count_is_three` **→** `mod_count_is_five`」· `:608` 同 ⇒ **那两行从未断言「护栏是三个」，它们描述的是一次变更** ⇒ **原指控从一开始就误读了语义** ⇒ **永久撤回（理由已更正）**；`:310`「护栏是两条断言：`main.rs::mod_count_is_five`」亦正确
- ⚠ **错因是一条比「别用 `head`」更精确的规则**：我用了 `| head -3`，而**前 3 行恰好都是已修正的说法** ⇒ **截断被「可见前缀恰好支持我的结论」掩盖、不留痕迹**，我还写下了一句**可核验为假**的断言而没让命令报计数
- > ⭐ **`head` 最危险的时候，是「被截断掉的那部分恰好不含反证」的时候。** **凡是要下「不存在/全部/没有」这类全称判断，必须让命令自己报计数（`grep -c`）或不加 `head`** —— **计数不会骗人，前缀会。**
- ⇒ **第 5 次 grep 模式失效、第 2 次 `head`**（第 1 次 B0130）；与 B0212「差集要逐条判性质」**成对** ⇒ ⇒ **共同对策同一句：让机器报出你需要的那个量（计数/完整集合），别让它只给一个前缀**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十六次对账（CONSOLIDATION-16）· ⭐ 一次对账里撤了两条「我自己的结论」
- **17 条 P1/P2 复核：17/17 仍成立**；【模式 Q + KNOWN-LOSSES】**✔ 通过**
- ⭐ **三条新正面模式**：**P23** 门禁自己被复审（「**门禁在假装工作**」这句话是项目自己写的）· **P24** 把「新增一个成员」变成一次必然失败 · **P25** 同一约束多处出现时让「除文档外」全部是**引用**
- 🔻 **撤了两条「我自己的结论」**：① **F-0040-01 永久撤回** —— 那两行（`:105`/`:608`）**从未断言「护栏是三个」**，它们**描述一次变更**（`three` **→** `five`）⇒ **原指控从一开始就误读语义** ② 「**AGENTS 落后 13 天**」撤回 —— 正文已到 **09-28**（与代码同步），**错的只是「变更历史」那一节**
- ⚠ 而 ① 的**撤回理由我在 B0211 给错过一次**（写「全文已无 `mod_count_is_three`」，而 `grep -c` = **2**）⇒ **错因是 `| head -3`**
- ⭐⭐ **由此得到本轮最重要的规则**：**`head` 最危险的时候，是「被截断掉的那部分恰好不含反证」的时候** —— 截断**不留痕迹**，而你会带着一个「已核实」的错误结论继续走 ⇒ **凡是要下「不存在/全部/没有」这类全称判断，必须让命令自己报计数（`grep -c`）或不加 `head`** ⇒ **计数不会骗人，前缀会**
- ⭐ **「聚合结果」的两条纪律成对**：**差集要逐条判性质**（B0212：18 个「缺失」里真漂移 **0** 个）· **枚举要数全**（本批）⇒ **共同对策：让机器报出你需要的那个量，别让它只给一个前缀**；本审计**第 5 次 grep 模式失效、第 2 次 `head`**
- 覆盖率（304 条去重路径）：Rust `.rs` **79** · 前端 `.dart` **35/218** · CI/脚本 4；**Mod crates 13/61**（本轮 +2）
- ⭐ 自评：**本轮做的是「核自己说过的话」，而不是「读新代码」**；而若没有 B0119–B0145 建立的 P13/P14/P21/P22，**我核到的正面样本只会以「这段写得不错」留在对话里，不会成为可复用的模式**

### BATCH-0220（2 文件 / 头注+落盘 · **0 新发现** · Mod crates 第 14 个）
- ⭐ `persona/sessions.rs:1-16` 头注**先说清「这一半归谁」**：宿主 `SessionPromptSink` **只活在进程内存、进程一停就没了** ⇒ 「**回原会话仍在**」跨重启那半**必须由本 Mod 自己落盘** ⇒ **点名对方模块** ⇒ 正是 **F-0062-01「三段职责无人认领」的反面做法**
- 另：存的是**规范化后的 V2 卡**而非原始文本（「**可读、可审计**，也能原样再解析」）+「会话 id 是**宿主归一化过的**（`sanitize_session_id`），**本文件不自己发明**」⇒ 与 **P25/P2** 同形，且**不发明第二套归一化**（B0215 同纪律）
- ⭐ `:29`「tmp + rename（**与 host 写 `mods.json` / 本 Mod 写导入卡同一条纪律**）」+ `:274-300` 原子写（tmp 名**带命名空间** `persona-mod-cards.json.tmp` · **rename 失败清理 tmp** · **失败上抛给调用方**）
- ⇒ **原子写纪律在本仓有四处**（toml / `.env` / `mods.json` / 本文件），**不是共享一个函数，而是「同一条纪律」被点名四次** ⇒ ⭐ **P25 的又一形态：跨 crate 无法共享时，就用「命名 + 指名」代替「引用」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0221（1 文件 / 定点读 · **0 新发现** · 跨 Mod 语义观察）
- ⭐ `mod-memory/src/summary.rs:126-155` → 「**开了总闸但没接上**」被建模成**带原因的独立状态**（`degraded`），**不是** `enabled=false`；**构造器 `degraded(reason)` 强制给理由** ⇒ **写不出「降级了但不知道为什么」**；`Disabled` 与 `degraded` 是**两个不同结果**（:155「开了但缺 base_url / model 或 URL 非法 → Disabled **+** `degraded_note`」）
- ⭐ **跨 Mod 的同一语义，四处形状各异**：`external-input` **403 mod_disabled**（B0005）· `director` **`DisabledStaging` 空对象 + `degraded_note`**（B0106）· `persona` `apply_settings` **返 `false`**（B0110）· `memory` **`degraded_note`**（本批）
- ⇒ **形状不统一，语义统一**；而**这正是「跨 Mod 边界的正确状态」**：Mod 是独立 crate（P25/B0217 已核「架构边界导致的有理由的重复」）⇒ **它们不该共享实现；该统一的是语义、不是类型**
- ⇒ 与 **B0152 `join_endpoint` 判据同源**：**架构边界 ⇒ 允许形状不同，但要求语义可对照**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0222（1 文件 / 定点读 · **0 新发现** · F-0111-01 可达性钉成确切形状）
- ⭐ `mod-voice-input/src/lib.rs:196/214` → `settings_spec` 把 **`sidecar_script`** 与 **`sidecar_python`** 都声明成**普通 String 字段**（`secret: false` / `default: None` / label「Python 解释器（缺省 python3）」）⇒ 面板**按 kind 渲染**（B0105）⇒ **它们是两个普通文本框**
- ⇒ **用户不经改文件、不设环境变量，就能在正常设置界面里把本 Mod 指向机器上任意可执行文件 + 任意脚本路径**；而 ASR token 随后进入**那个进程**的 argv
- ⚠ **但这不构成提权**（用户自己选了解释器与脚本 ⇒ 信任本来就是他的）⇒ **P2 不变**；**真正的问题仍是那两条**：① **无白名单**（打错字/好奇即可启动任意程序）② **token 落在那个进程的 argv、同机可读** ⇒ 而 B0120 核过 `.env` 侧**已**用「先 chmod tmp 再 rename」关过一次窗口 ⇒ **这一处没有对等处置**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0223（1 文件 / 定点读 · **0 新发现** · 契约层观察）
- ⭐ `mod-system/src/services.rs` 每一项宿主能力都是**闭包 newtype** + **`disabled()`**（:109）+ **`enabled()`**（:117）⇒ **「休眠」是契约的一部分**（有名字、有构造器、有查询）⇒ **不可能出现「半接上的能力」**
- ⇒ 与 **B0106 `DisabledStaging`（没有 HTTP 字段 ⇒ 结构上不可能发请求）同一形状**，且更进一步：**休眠态进了契约 ⇒ 别处不需要为它写分支**
- ⭐ **它解释了 B0221「四处形状各异」的根因**：契约给的是 **`bool` 返回**、**不是带状态的名字** ⇒ **「为什么没接上」在返回类型里被压扁** ⇒ 各 Mod 只好自己传出去（403 / 空对象 / 不传 / 另开 `degraded_note`）
- ⇒ ⭐ **一个只返 `bool` 的能力接口，会把「状态」挤到调用方去自己编码** —— 这是 B0130「`(bool,String)` 逼出嗅探」与 B0168「纯投影让红线 O 无从绕过」的**第三次**，这次在**契约层**；要让四处形状统一，**该改的是这个返回类型**（如 `enum Capability { Ready(..), NotWired(reason) }`）
- ⚠ **设计建议而非缺陷**：现状**无功能性问题**（四处都能观测到「没接上」），**只是排查时要在四个地方各认一次**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0224（1 文件 / 定点读 · ⭐ **P3 1**）
- ⭐ **F-0224-01（P3）** `mod-system/src/settings.rs:23` `secret: bool` 的注释声称「**内容不落日志**」，而 `grep -rn "secret" mod_registry.rs` **零命中** ⇒ **宿主从不读这个 flag** ⇒ **该半边是约定、无机制**
- ① 前端半边（渲染 password）**有机制**（B0105：面板对 secret 字段**留空不提交**）② 「不回显」**有机制**（`mods_routes.rs:494` 的 `obj.remove(key)`，B0066）③ 已发布 Mod **确实都没记**（B0201：6 个 Mod、50 处日志**无泄露**）
- ⇒ ⚠ **而它防错了面**：「日志」靠各 Mod 纪律；而**真正发生过的**那条路是 **F-0111-01 的 token 进子进程 `argv`** ⇒ **`secret` 覆盖「日志」，泄露发生在「进程参数」**
- 建议二选一：**(a) 把注释对齐到它真正在做的事**（「仅供前端渲染成 password 输入；**日志纪律由各 Mod 自负**」+ 像 B0217 模板那样把纪律**指名**）或 **(b) 让 `ModLogger` 成为唯一日志入口**（传 `&ModSettingField` 而非裸串 ⇒ secret 的值**无法被传进去**）⇒ **(b) 更彻底，且契约层是唯一能一次性覆盖 8 个 Mod 的地方**（接 B0223）
- ⚠ **定 P3 而非 P2**：**无实际泄露**、声明在注释里、无运行时后果、「无机制」只是**未来风险**；且**不并入红线 R**（那是「密钥不出口」，「日志纪律」是另一条）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0225（1 文件 / 定点读 · **0 新发现** · 修正修法成本）
- ⭐ `services.rs:132-149` → **`ModLogger` 已经是唯一日志入口**（`info`/`warn`/`error` 都是一行转发到同一个 `inner` 闭包）⇒ **不需要「收敛入口」——它已经是了**
- ⇒ 修法 (b) 只是**加一个「按字段记」的入口**（传 `&ModSettingField` + 值，**由契约决定 secret 字段只记 key 与长度**）；而宿主那侧（`mod_registry.rs:402`，B0198 已核）**也已经是那一个闭包** ⇒ **改一处覆盖 8 个 Mod** ⇒ **成本比 B0224 写的「更彻底」小得多**
- ⚠ **但修法的形状必须避开「启发式」陷阱**：**不要**在 `inner` 加「按字符串模式猜密钥」的过滤器（那是**猜**，而 B0164 刚教过其代价）⇒ ⭐ **正确形状是「类型」**（结构化字段 + 值 ⇒ 由**契约**决定记什么）
- ⇒ 与 **B0213 `.filter()` 教训同源**：**在转换/传输之前按类型挡住，别在事后按文本猜**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0226（1 文件 / 头注 · **0 新发现** · ⭐ 正式转前端）
- ⭐ `lib/ui/chat_panel.dart:1-5`「**只依赖纯模型**（`chat/chat_message.dart`、`state/ui_phase.dart`），**不** import `package:web` 那条链 ⇒ **整个面板可 VM/widget 测试**」
- ⇒ ⭐ **我那两次「零命中」（无帧类型、无 `ChatRole`/Notice）是正确结果、不是漏搜** ⇒ **帧处理在 controller 层**（`ws_client` → `ChatController` 决策 → 面板渲染），而**决策层 B0189–B0192 已审** ⇒ **该文件的职责是渲染**
- ⭐ **本仓第 4 处「把可测性设计进去并写下来」**：`main.dart` 隔离 `package:web`（B0180）· `LiveRegionThrottle` **时钟可注入**（B0179）· `interval_secs_from_config` **转换前挡 `NaN`/无穷**（B0213）· **本文件不 import 那条链**
- ⇒ 而本批收获是**读法**：「面板应该处理那些帧」是**错的**，**头注 5 行就说了职责** ⇒ ⇒ **B0213 立的读法（先头注）在换根的第一批就立刻回本**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0227（1 文件 / 头注 · **0 新发现** · ⭐ 63 行含三条正面模式）
- ⭐ `lib/ui/error_banner.dart:5-8` → **设计决策引用外部调研并给可查索引**：「**禁止把失败渲染成「只有降低不透明度」或「红色小字」** —— 同类项目**最一致的缺陷**（**Nexus / OLV-Web**）【**调研 survey §8 缺陷 17**】」⇒ **P1 最强形态：后来者可以去核对这条主张**
- ⭐ `:9-12` → **一条关于规则的要求**：「**失败要有「下一步」**：`429 busy`→打断并重发 · `503 no_supervisor`→去 LLM 设置 · `network_error`→重试……**只报告不给出路的错误提示会让用户停在原地**」⇒ **红线 hint 要求在 UI 侧的实现**（B0136 核过分支 · B0161 核过后端 · **本批核了本体**）
- ⭐ `:14-17` → **一次三合一收敛 + before/after**（这里 / `field_row` 的 `'⚠ $error'` / `dev_tools_section` 同一句字面文本 → **只有 `InlineNotice` 一处**，「**错误长什么样以后只改一个文件**」）⇒ **P2 + 记录了收敛前的样子** ⇒ **本仓第四处「把同一件事收敛」**
- ⇒ ⭐ **「文件长度 ≠ 信息密度」第一例**：**63 行**含**三条独立正面模式** + 三处码→动作 + 一条禁令（对照 `dev_tools_section.dart` 1874 行）
- ⇒ **读法上再看一次**：**先看头注的收益高于先看代码**（B0226 已验一次，本批再验一次）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0228（3 文件 / 头注 · **0 新发现** · ⭐ P22 第五例，5/5 命中）
- `message_bubble.dart` → import 边界的**理由是「可测性」**：「**不** import `chat_controller.dart`，**那样会把 `package:web` 拖进来，让整个气泡不可测**」；带日期编号的改动记录**指路而非复述**（规则在 `chat_markdown.dart` 头注）
- `memory_panel.dart` → 独立「**# 边界**」段**逐条枚举数据源**（含**排序**「最新在前」与**身份**「按 `id` 定位」）+ **归属声明**（「本文件由 memory 轨道独占，其他轨道不要改」）
- ⭐ `persona_panel.dart` → **点名一个不存在的 API**（「**没有** POST /mods/persona/config」）⇒ **读者才知道这个设计是被逼的而非选的**；并给三步效果（Mod 侧解析 → 规范化落盘 → 立刻生效，**与 B0113 persona 侧接得上**）；把**跨 Mod 的 last-writer-wins** 写进头注
- ⇒ ⭐ **P22 第五例**且**成规模**：**5 个前端文件的头注无一例外先说职责与边界（5/5）** ⇒ **「先头注」这条读法连续五批回本**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0229（1 文件 / 定点读 · **0 新发现** · ⭐ 立正面模式 P26）
- ⭐ `message_bubble.dart:461-462` → **门是 `hasChatMarkdown(text)`**：纯文本走 `SelectableText`、有记号才走 `.rich` ⇒ ⇒ B0187 那条测试「**纯文本消息不进 `Text.rich`**（选择/复制行为保持原样）」**在实现里成立**
- ⭐ `:445-449` **理由是「行为差异」而非性能**：`.rich` 会把 **span 边界带进选区** ⇒ ⇒ **他们知道确切机制**；且**预先挡掉最可能的错误重构**：「**走捷径那条不是微优化**」
- 另：行内码底色**复用语义槽位** `surfaceContainerHighest`（「『比面板稍微突出一点』的语义槽位），**不新造颜色**；**浅色主题下它也自动是深一档的灰**）⇒ **tokens P2 纪律的内联应用 + 带理由**
- ⇒ ⭐ **正面模式 P26（新）**：**在决策点写「这不是微优化」+ 写出「若不这么做」的具体代价** ⇒ 后来者要改得**先推翻那条具体的代价**，而不是推翻一个「风格偏好」；本例的代价是「**span 边界进选区**」这一**可核对的行为事实**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0230（1 文件 / 定点读 · **0 新发现** · 让 B0192 的观察变完整）
- ⭐ `memory_panel.dart:283-296` → **所有主动变更走同一条路** `_send` ⇒ **一个漏斗** ⇒ 「变更后要刷新」**不可能只对某一类动作做到**；其尾巴含**三件必做的事**（刷新运行态 + 刷新列表 + **通知宿主「可能要重启/重新点火」**）⇒ 而第三件**正是 Mod 契约要求的** ⇒ **面板与契约在同一个漏斗上对齐**
- ⭐ `:292` **重入闩 `if (_busy) return;`** ⇒ **「双击」在 UI 层就被挡住**（与后端守卫**互补**，不是替代）；且 **`if (!mounted) return;` 四处都有**（:250/:258/:269/:296）⇒ 「await 之后组件已卸载」**成规模应用**
- ⇒ ⭐ **让 B0192 的观察变完整**（我当时的措辞不够准）：**闩出现在「会改状态」的地方、不出现在「只读」的地方** ⇒ **这不是不一致，是一个连贯的设计**（**B0192 那个观察本身仍成立**：F-0189-01 的竞态在 `load()` 上**确实**没有被闩住）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0231（1 文件 / 定点读 · **0 新发现** · 「指路而非复述」第四处）
- ⭐ `persona_panel.dart` 把 **2 入口（粘贴 JSON / 选 PNG）× 2 作用域（绑当前会话 / 旧的全局行为）= 4 个组合逐个给了文档行**（:301/:314/:331/:373）⇒ ⇒ **会话绑定在 UI 上是可见的选择、不是隐式行为**
- ⇒ 「**前缀由 Mod 剥**」（:331）⇒ **责任在 Mod 侧、UI 不做** ⇒ 与 **B0113** 核的 persona 侧（`render_injected_text`/`render_from_config` 是 **Mod 侧纯函数**）**一致**
- ⭐ 跨 Mod 关系用**用户能懂的话 + 可行动建议**：「后写覆盖、不做仲裁——**想稳定用全局人设就别同时开「记忆」的注入**」（:595）⇒ **P1 的 UI 版**；并注「**与 memory 文档同口径**」（:585）
- ⇒ ⇒ **第四处「指路而非复述」**（B0217 模板 / B0220 归属 / B0228 改动记录 / 本批）；**组合面逐案记录**是让 4 路矩阵保持诚实的办法
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0232（1 文件 / 头注 · **0 新发现** · 一次搬家顺带修对一个语义）
- ⭐ `live2d_stage.dart:18-20` → 「[`PresetStatus`] 的**唯一真源**已搬到 `preset_status.dart`……**它现在是「渲染面 ack 的显示快照，不是本地计时器」**」⇒ ⇒ **这个 UI 状态曾经是「本地猜的」，现在改成「读渲染面的回执」** ⇒ **UI 不再自己发明状态**
- ⇒ 与 **B0116** `shouldPreserveStage`（本地启发式 → 对真实属性的 `mapEquals`）、**B0136** 的 epoch 闩锁**同族**（**不要推断，去读真源**）；且搬家**兼容**（**re-export**，**既有调用方一行都不用改**）⇒ **P2 带迁移方案**
- ⇒ ⭐ **关于「私有汇合」的观察**：**把真源搬到一处，常常伴随「那个值的语义也变对了」—— 搬家的副产品往往比搬家本身更值钱**；本例 **成本 ≈ 0（re-export）、收益 = 语义修正**
- 顺带：「**平台边界用条件导出**」在本仓**出现第二次**（`live2d_host_{stub,web}` + B0186 的 `stage_pointer_interceptor{,_web,_stub}`）⇒ ⇒ 是**约定**而非权宜
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0233（1 文件 / 头注 · **0 新发现** · ⭐ 第 6 处「可测性设计进去」）
- ⭐ `director_observer_section.dart:2-4`「**`dev_mode=false` 时整块不在语义树**；本组件**自己也短路一次，测试可单点**」⇒ ⇒ 与 B0226/B0228 的 import 边界**同形**（都是「**为了让测试能单点跑**」而写的代码）
- ⭐ 「数据怎么进来（**不改 `shell_settings.dart` 的硬约束**）」⇒ **点名自己正在绕开的约束**再给机制（单例 `DirectorObserverFeed` + `main.dart` 只 push + `ListenableBuilder` 消费）
- ⭐ 「构造函数仍保留**可注入的假数据口**，widget 测试注入 fake，**不发真网络**」⇒ **生产路与测试路在构造函数处分开**
- ⚠ **代价也写在头注里**：那个单例 feed ⇒ `main.dart` 持有**全局可变**句柄 ⇒ 这是「不改 `shell_settings.dart`」这个**真实约束**的**代价**，**被写下来而非被隐藏** ⇒ 与「有理由的重复」（B0152/B0221）**同属自觉取舍**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十七次对账（CONSOLIDATION-17）· ⭐ 一条读法连续七批回本
- **18 条 P1/P2 复核：18/18 仍成立**（含 F-0224-01 的依据：**契约侧 `secret` 在宿主零命中**）；【模式 Q + KNOWN-LOSSES】**✔ 通过**
- 本轮唯一新发现 **F-0224-01（P3）**，且**两次修正其修法估计**：① B0224 定「约定无机制、且**防错了面**（泄露在 argv 不在日志）」② ⭐ B0225 读 `ModLogger` 本体后发现**它已经是唯一入口** ⇒ 修法从「改架构」缩成「**加一个按字段记的入口**」，宿主侧也已是那一个闭包 ⇒ **改一处覆盖 8 个 Mod**；且**形状必须是「类型」不是「文本模式」**（B0164/B0213 教训）
- ⭐ **本轮最重要的收获是读法**：「**先头注**」**连续七批回本**（B0226–B0233），八份头注各自改变了审计方向 ⇒ **元结论：头注往往比代码更能改变审计方向 —— 因为头注写「为什么这样、以及不这样会怎样」，而代码只写「现在这样」**
- 「把可测性设计进去」**六处**（每处都写下了理由）：`main.dart` 隔离 `package:web` · throttle **时钟可注入** · config **转换前挡 NaN** · 两个组件**不 import `package:web`** · `director_observer_section` **双重门 + 可注入假数据口**
- 覆盖率（306 条去重路径）：Rust `.rs` **79** · 前端 `.dart` **37/218**（**本轮 +6**）· CI/脚本 4 ⇒ **前端仍 181 未读（最大面）**；Mod crates 19/61
- ⭐ 自评：**本轮唯一新缺陷是 P3，而真正的产出是「我少犯了几次错」与「我知道怎么少犯」** ⇒ **审计的价值不与它找到的缺陷数量成正比，而与它「不把不确定的东西说成确定的」的能力成正比**

### BATCH-0235（1 文件 / 定点读 · **0 新发现** · ⭐ P26 补全为两种形态）
- ⭐ `message_bubble.dart:78-85` → 塌陷规则**显式**：空 ∧ ¬流式 ∧ ¬失败 ∧ ¬只有思考 ⇒ 才 `shrink` ⇒ ⇒ **失败轮永远不会被塌成「什么都没有」**
- ⭐ **三个例外各被命名并写明各自的 UI 语义**（「表示还在等」/「**用户唯一的恢复入口**」/「**思考就是本轮唯一内容**」）⇒ 它们的**差别**是可见的；且注释**直接对下一次重构说话**：「**别跟着一起删**」
- ⭐⭐ 而它防的不是「有人觉得这三条差不多」——**恰恰相反，它防的是「有人因为这三条看起来像而把它们合并」**
- ⇒ ⭐ **P26 补全为两种形态**：**P26-a**（B0229）防「这**不是微优化**」被「优化」掉 · **P26-b**（本批，更锋利）**规则相似处最容易的错误重构是「合并它们」** ⇒ **注释的任务是让「它们哪里不同」变得可见**
- ⇒ **推论**：`a && !b && !c && !d` 这类**同形多子句**最容易被「顺手化简」⇒ **给每个子句写一句「它对应什么用户可见的东西」比写一句总说明更抗重构**；失败态用**语义调色板槽位**（不新造颜色，同 B0229）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0236（1 文件 / 定点读 · **0 新发现** · ⭐ P2 用在「安全属性」上）
- ⭐⭐ `memory_panel.dart:398-415` → 删除确认框**引用那条记录自己的正文**（`memoryRecordText(record['text'])`）⇒ 用户看到「**要删的是哪一条**」，而不只是「确定吗？」
- ⇒ 而 `memoryRecordText` **正是列表显示用的那个函数**（B0187 核过：压平空白 + 截断 + 空记录兜底）⇒ ⇒ ⭐ **显示与确认共用同一个渲染函数** ⇒ **确认框不可能显示出与列表不同的东西** ⇒ ⇒ **P2 用在「安全属性」上**，不只是「一致性」
- ⭐ `if (confirmed != true) return;`（:377/:415）⇒ **`null`（ESC 关掉对话框）不会被误判为「确认」** ⇒ **破坏性动作取的是「null 安全的那个方向」**；两处破坏性动作各有确认框（:352/:398）
- 清空的**失败**也被映射成可处置文案（:14 + :120-123 `memoryCommandErrorMessage(e, '清空')`）且用**共享**映射函数（B0187 核过其测试「每个码都带码且可处置」）⇒ **P2 贯穿整个面板**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0237（1 文件 / 定点读 · **0 新发现** · 教科书式拦住一类缺陷）
- ⭐ `persona_panel.dart:103-110` `personaStatusIsFailed` → 「**两个值都要认**：GET …/mods 序列化的是 `ModStatus::as_str()` 的 **failed**，而 `ModInfo.statusLabel` 认识的历史写法是 **error**。**只认一个，失败提示就永远不会出现** —— 「**界面看起来没事**」的**那类缺陷**」
- ⇒ ⇒ **漏掉的后果被写成「缺陷类别」**（而非只描述这次现象）· 判据是**纯函数且公开可测**（**不是散落内联比较**）
- ⇒ ⇒ **这正是我在别处记成缺陷的那一类**（同一状态的两个表示不一致：B0123 视图字段对等性 · F-0060-01 `SettingsPatch` 静默忽略未知键）⇒ **这里找到了，并让判据两个都认**
- ⭐ **与 B0159 `ErrorKind::code()` 构成一组对照**：**错误码 ⇒ 契约层统一**（一个码、两侧同源）· **状态字面量 ⇒ 判据层兼容**（两个都认）⇒ ⇒ **取决于你是不是引入迁移的人：引入方统一、消费方兼容**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0238（1 文件 / 定点读 · **0 新发现** · ⭐ P1 的新落点：写在 API 契约里）
- ⭐⭐ `live2d_stage.dart:154-159` `onAck` 的文档**逐字引用了 AGENTS rc.5 ⑧ 的真缺陷**（缩放读数永远是 `—`／没人读首帧 `stage-ack`）**并带症状与日期**：「**缺了它**……**首屏会一直显示一个「—」，看起来像坏了**（**2026-09-11 在真浏览器里就是这么显示的**）」⇒ 修复在位、且**为什么需要它**写在**签名旁边**
- ⭐ 「每收到一条**新的**」⇒ **去重**（非每帧回调）⇒ 与 B0132/B0136 的「不刷屏」纪律同族
- ⭐ `:161-165` 把「**不维护镜像状态**」写成**协议级**条款：**O13 冻结名** · 前端**不**建镜像 · `presetStatus` 是**唯一例外且同样只由 ack 驱动** ⇒ ⇒ **B0232 的发现在回调层被重述一遍**（两处独立地说同一句）
- ⇒ ⭐ **P1 的一种新落点**：四个回调（`onError`/`onAck`/`onRenderEvent`/`onReady`）**每个的文档都写「缺了它会怎样」** ⇒ ⇒ **只读签名 + 文档**的调用方就知道自己该负责什么 ⇒ **纪律不只活在「决策旁边的注释」里，它活在 API 契约里**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0239（1 目标 / 跨 3 文件定点 · **0 新发现** · ⭐ 立 P27）
- ⭐ `main.dart` **四处 push**（:319 `onEvent: …pushRenderEvent` / :710 `pushStageClock` / :726 `pushWsEvent` / :782 `pushPresetRequest`）⇒ ⇒ **feed 有真实生产者、面板不是装饰**（否则就是 B0237 说的「界面看起来没事」那一类）
- ⭐ 消费端**四栏且类型明确**（:278 `Listenable.builder` + `ObserverBuffer`:447 / `List<PresetRequest>`:537 / `List<RenderEvent>`:538 / `List<TtsSentence>`:587）⇒ 与头注声明的「四栏」**逐栏对应**
- ⭐⭐ `director_observer_section.dart:313` **`instance.clear()`** ⇒ **那个全局可变单例在拆卸时被显式清空** ⇒ **B0233 记的「代价」被补上另一半：代码顺手清理了它**
- ⇒ ⭐ **正面模式 P27（新）**：**当一个全局可变的「代价」被写下时，若代码还顺手清理了它，那「代价」就只是「机制」，不是「债」**；⇒ **推论：看到「用了全局」不构成缺陷，要看的是「有没有主人、什么时候被清空」**（与 B0166 同源：**先问代价是否被承担，再问形态是否理想**）
- ⏰ 下一批第一件事：**核 `ObserverBuffer` 的容量上界**（只被 push 的缓冲区若无上界 = B0132 核过的「日志是长期运行会累积的东西」那一类）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0240（1 文件 / 定点读 · **0 新发现** · 我承诺的检查通过）
- ⭐ `ObserverBuffer` **有界**：`:19`/`:75` 声明「环形缓冲**上限 200**」· `:136-139` `void _trim<T>(List<T> list) { while (list.length > kObserverBufferCapacity) { list.removeAt(0); } }`
- ⇒ ⭐ `_trim<T>` 是**泛型 helper** ⇒ **一个实现约束四个栏** ⇒ **将来加第五栏自动受约束** ⇒ ⇒ ⭐ **P2 的第三次落点**：一致性（B0215）· **安全属性**（B0236）· **资源上界**（本批）
- ⭐ `:464`「共 N 条 / **上限 $cap**」⇒ ⇒ **上界在界面上可见** ⇒ **触顶对用户显形**、非静默截断（与 B0120 的 `+…`、B0227 的「不假装成功」同族）
- ⇒ **补齐 B0132 那一族的两种正确处置**：**列表 → 虚拟化**（B0227）· **缓冲 → 上界 + 显形**（本批）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十八次对账（CONSOLIDATION-18）· ⭐ 一次对账里纠正了我自己手数的覆盖率
- **18 条 P1/P2 复核：18/18 仍成立**；【模式 Q + KNOWN-LOSSES】**✔ 通过**
- 🔻 **本轮第一件正事**：我手写的「前端 `.dart` 44→…→**49**/218」与**机械数 37** **差 12** ⇒ **第 4 次手数漂移**，但**性质不同**：前几次是**算错**，这次是「**我以为每批 +1、其实没加**」⇒ **根因：用「这批读了 1 个文件」推出「覆盖率 +1」**
- ⇒ ⭐ **执行规则升级**：**凡要报「覆盖率 +N」，必须由脚本给出**；**「本批新增几个文件」也交给脚本**（比对 INDEX 快照），**不在批次文件里手写**
- ⭐ 而这与「模式 Q 为什么有效」形成对照：**模式 Q 有效是因为它每批都在跑**；「计数必须机械算」失效是因为它**只在写总结时才想起** ⇒ **审计工具自身也是被审计对象**
- 本轮三条新正面模式：**P26** 补全两形态（第二形态防「**合并相似规则**」的错误重构）· **P27**（**代价被写下且被清理 ⇒ 只是机制、不是债**）· **P2 第三落点**（`_trim<T>` 泛型 helper **一个实现约束四个栏** ⇒ 新栏自动受约束）；另 **P1 新落点：纪律写在 API 契约里**（四个回调各写「缺了它会怎样」）
- 覆盖率（**机械**：306 条去重路径）：Rust `.rs` **79** · 前端 `.dart` **37/218** · CI/脚本 4 ⇒ **前端仍 181 未读（最大面）**
- ⭐ 自评：**本轮发现的最大的问题在**自己的账本**里，而不在被审的代码里** —— 这不是坏事：**审计工具自身也是被审计对象**

### BATCH-0242（1 文件 / 定点读 · **0 新发现** · ⭐「上界」分三族）
- `error_banner.dart:23-27` `ErrorAction = 标签 + 回调`（极小的值类型 ⇒ 造一个**成本极低** ⇒ 与 B0227「失败要有下一步」**一致**：既然便宜，就没有不提供的理由）
- ⭐ `:44-45` 「建议动作（**0–2 个**）。**超过 2 个说明这条错误该拆**」⇒ ⇒ **这不是 UI 裁剪，是一条诊断信号**；且它写在**字段文档**上 ⇒ 每个调用点的读者都会看到；`onDismiss` 可空且**空态有定义**
- ⇒ ⭐ **「上界」分三族（此前没分开过）**：**截断族**（越界=丢掉多出来的 ⇒ 用**代码**：`MAX_SEGMENTS`/`MAX_RECORDS`/`kObserverBufferCapacity`）· **信号族**（越界=**「你这里错了」** ⇒ 用**注释**：本批的 `actions` 0–2）· **绊线族**（让「新增成员」必然失败 ⇒ 用**断言**：登记表长度 / `fieldCount`）⇒ ⇒ **混用就会失效**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0243（1 文件 / 定点读 · **0 新发现** · ⭐「截断族」上界的完整写法）
- ⭐ `chat_panel.dart:22-34` `kChatHistoryLimit = 500` 的注释**五样齐**：① 数字的理由且**理由含无障碍**（「不至于让 `ListView` 的**语义树与读屏浏览退化**」）② 截断方向（**丢最旧的**）③ 不截断的后果（「让『往上翻』**变成不可用操作**」）④ **用「选了另一层」论证本层**（「**刻意在视图层裁剪而不是数据层**：数据层裁剪会让『**完整对话无法导出排查**』」= **P22 用在设计选择上**）
- ⭐⭐⭐ ⑤ **注释记录了「自己曾经是错的」**：「（**2026-09-13 rc.3 更正**：原注释写『聊天历史**不落盘**』……**那是多会话存储接线之前的口径，早就不成立**）」⇒ ⇒ **第 5 处「记录推翻过的方案」**、**第 2 处「就地更正自己」**
- 另：注释点明数据层是 `main.dart` 注入的 `saveChatSessions` ⇒ 与我核过的多会话架构**接得上**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0244（1 文件 / 定点读 · **0 新发现** · ⭐ 立正面模式 P28）
- ⭐ `persona_panel.dart:36-38`「命令通道与运行态**共用 runtime 锁**：Mod 未启用时 host 回 **503 command_unavailable**。所以停用时导入按钮是**禁用**的（**而不是让用户点一下才吃一个错误**），并**就地说明原因**；导入成功后**不必**再启停一次」
- ⇒ ⇒ **约束被点名到机制层**（不是 UI 的选择）· **被否掉的替代方案被写出来**（P22+P1 同款）· **替代做法给了**（禁用的控件在它被禁用的地方解释自己）· **顺带去掉一步冗余操作**
- ⇒ ⭐ **正面模式 P28（新）**：**把「不可用」做成「禁用 + 就地说明」，而不是「可点 + 吃错误」** ⇒ 因为后者把**契约限制**伪装成**用户失误** ⇒ 与 B0227「**不假装成功**」**同族**（那管假装成功，这条管**假装可用**）
- 另：「**基线**」概念在**四处**同一套措辞（:131/:393/:493/:495）⇒ **字符串层面的 P2**，且与 **B0113** 核的 persona Mod 侧（停用 → 还原基线）**两侧同词**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0245（1 文件 / 定点读 · **0 新发现** · ⭐ P28 补第二样本）
- ⭐⭐ `dev_tools_section.dart:469`「为 `null` 时展开的表单**只读展现、保存按钮禁用** —— **不假装能保存**」⇒ ⇒ **P28 第二样本且更强**：被写成**一条原则**（而非一次性处置），且**写在回调的可空性文档上** ⇒ 与 B0238 同一手（**纪律写在 API 契约里**）
- ⭐ **「不假装」家族的三位成员**：`不假装成功`（B0227 `error_banner`）· `不假装可用`（B0244 `persona_panel` P28）· **`不假装能保存`**（本批）
- ⭐ **无障碍成员**：`:533`「**状态用文字**（「运行中」/「已停用」），**不靠颜色**」⇒ **不要把意义只编码在单一感官通道**（灰度截图 / 色觉障碍仍可读）
- 另 `:299` `enabled: !widget.busy` ⇒ **禁用态由真实状态驱动**、不是猜的；`:759` 成功文案**区分「已保存」与「已保存但停用」**；`:172` 按钮**禁用防重复提交**（B0230 `_busy` 同形）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0246（1 文件 / 定点读 · **0 新发现** · ⭐「不假装」家族第四位）
- ⭐⭐ `memory_panel.dart:51-54`「`activeSessionId == null` 时必须**说清降级**，**不能假装绑好了** —— 这是 **L1 验收句「还没有会话：记忆会落到全局桶（与所有会话共享）」的 UI 落点**」⇒ ⇒ **字符串可对验收标准逐字核**；`:92`「与 **Rust 侧口径逐字对齐**」⇒ 两侧引同一条规则（B0113/B0114 核过 Rust 侧）
- ⭐ **两种情形是两个不同字符串**：`:58` 无会话 + **怎么脱困**（「发一条消息后…」）· `:61` 哪个会话 + 「**只影响这个会话**」+ 切换后会变
- ⭐ **纯函数可单测**，且 **B0187** 的测试把两种情形都钉住（测试名的两个分句与文案的两情形**逐字对应**）
- ⇒ ⇒ **「不假装」不是口号，而是四个具名位置上的检查表**：**成功**（B0227）· **可用**（B0244 P28）· **能保存**（B0245）· **绑好了**（本批）；其中**两处**还溯到**验收句**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0247（2 文件 / 定点读 · **0 新发现** · ⭐ rc.3 N0 链四跳全通）
- ⭐⭐ `chat_message.dart:84-85`「**刷新后那段文字还在，就必须还带着「未收尾」这个限定**」⇒ ⇒ **字段的理由是一个「条件」** ⇒ **落盘是让那句话继续为真的唯一办法** ⇒ **不是「顺手做了持久化」，是「不做就不成立」**
- ⭐ `:101` **只在为真时写键** ⇒ **正常消息的存档里没这个键**（不增常态体积）；`:125` `raw['unfinished'] == true` ⇒ **缺键/非 true 归 false** ⇒ **向前兼容**（老存档能读）⇒ **P25 思路的小实例：字段按需写、读取端给默认**
- ⇒ ⭐ **rc.3 N0 链四跳全通**（B0133 记的那条）：WS `text_fallback` 帧（B0135）→ `_applyTextFallback` **整段设置 + 打标记**（`controller:369/387`）→ **落盘**（`message:101`）→ 刷新后**仍带限定**（`message:125`）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0248（1 文件 / 定点读 · **0 新发现** · ⭐ N0 链在两种语言里全读完）
- ⭐ `chat_controller.dart:366-377` → **整段覆盖 vs 追加**，理由精确：fallback 带**整轮正文**、已上屏的只是**（大致）前缀** ⇒ **追加会让正文出现两遍**
- ⭐⭐ **拒绝「算已上屏前缀到哪结束」并说清不可靠**：「**那受切句器 trim 影响，算不准**」⇒ **P22 + P26-a 同一行**；理由里的知识来自 **B0132** 核过的切句器/清洗 ⇒ **UI 的一个决定被后端的一个事实支撑**
- ⭐ **能建气泡**并点名两个到达竞态（**服务端主动开口** / **帧先于 HTTP 响应到达**）⇒ 选择是「**照样要收下** —— **丢帧就等于又把正文丢了**」⇒ **规则从那次失败推出，不从方便推出**
- ⇒ ✅ **rc.3 N0 链四跳全链**：B0133 服务端残余排空 · B0135 事件面 9 臂无兜底 · **本批前端施加** · B0247 落盘 + 刷新后仍带限定
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0249（2 文件 / 定点读 · **0 新发现** · ⭐⭐ 判据精化：P25 有条件）
- `chat_panel:144-145` 施加点**与 B0243 核的文档一致**：视图层裁剪、`sublist(length - limit)` **保留最新** ⇒ **文档说的做了**
- ⭐ `chat_session:150-155` 的 `kMaxMessagesPerSession = 500` 是**另一份声明**（**非引用**）⇒ 按 B0152 判据本是**候选重复**；且文档**点名不同步的症状**（「界面上能看到但重开就没了」）并**自评难度**（「最难查的偏差」）
- ⭐⭐ **但核完发现：收敛的代价更大** —— 让 `chat_session.dart` import `ui/chat_panel.dart` ⇒ **`data/` 层依赖 `ui/` 组件 = 层间反向依赖** ⇒ **比「两个 500」更糟** ⇒ ⇒ **重复可能是两个不完美选项里更便宜的那个**
- ⇒ ⭐⭐ **判据精化**：**「重复是否可接受」不只取决于「有没有理由」，还取决于「收敛的代价」；若收敛会引入层间反向依赖，则重复更便宜。** ⇒ **P25 不是无条件的** —— 它**在「能引用、且引用不造成层间倒挂时」才成立**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0250（1 文件 / 定点读 · **0 新发现** · ⭐ 一处极易混淆的两态）
- ⭐⭐ `live2d_stage.dart:53-54`「`applyForSeq`：**取到 cue 才下发**；`preset_id` **为空串 = 「本轮不动」（不调用下发）**；`preset_id == 'none'` = **显式撤销，照常下发一次**」+ `:77`「空 = 本轮不动；其余（含 `'none'`）**恰好一次**」
- ⇒ ⇒ **两个值、两种语义、且在函数文档上分开写**；**协议层也是两个值**（B0015 的 WS `action_cue` · B0128 schema 的 `expression_ids` 含 `"none"` 哨兵）
- ⇒ ⇒ **「恰好一次」**与 SSE `Done`（B0156）、音频块 `first/final_chunk`（B0132）**同纪律**；两个同名 `kDefaultPresetIntensity` **刻意不同**并注明理由 ⇒ **P22**
- ⚠ **本批没核到（如实标注）**：**iframe 重建（`error` 后 `retry`）时 `presetStatus` 那份 ack 快照会不会留旧值** ⇒ **B0251 第一件事**（B0232 已确认它是「ack 驱动的显示快照」；若重建后不清 ⇒ **短暂显示一个已不生效的表情状态** = 「看起来没事」那一类）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0251（1 文件 / 全读 106 行 · **0 新发现** · B0250 的问题被精确界定）
- ⭐ `preset_status.dart:60` `enum PresetStatusAction { **set, clear, ignore** }` ⇒ ⭐ **「清空」是一等类型**，**不是「没有值」** ⇒ **清除是一个可以被要求发生的状态**，而不是靠「值自然消失」
- ⭐ `:68-76` `presetStatusUpdateFor(RenderEvent, now, {source})` 是**纯函数**，四种 ack 的映射**逐条写明**（`applied`→set · `replaced/expired/dropped`→clear「该动画段结束」· `segment-ended`→不动「**音频事件**，与预设显示无关」）⇒ **不需要渲染面就能测**；与 B0232「**只由**渲染面 ack 生成、**真源在渲染面**」接得上
- ⇒ ⭐ **缺口的形状很轻**：若 iframe 重建后 Flutter 侧仍留着上一条快照，**修法是「重建后调一次 `clear`」**，不是「造一套新机制」⇒ 这正是**把清理建模成一个动作**买到的東西
- ⚠ **仍未核**：**宿主在重建后是否主动发 `clear`**（宿主侧接线，不在本文件）⇒ **不夸大**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0252（1 文件 / 定点读 · **0 新发现** · ⭐ 码与散文同名）
- ⭐⭐ `dev_tools_section.dart:254`「**删除激活中的模型会被服务端拒绝（409 model_active）**」⇒ ⇒ 用户**点之前**就知道结果（**而不是点完吃一个 409**），且**码与散文同名 ⇒ 可对账**
- ⇒ 与 **B0246 的「L1 验收句」**同一手法：**把「会怎样」写成可核对的句子/码**；属「**提前说清 / 不让人白点**」家族**第五例**、**第二例带服务端真实码**的
- ⭐ `:231` 在途按钮标签用**文字**（「**激活中…**」）⇒ 与 B0245「**状态用文字、不靠颜色**」同纪律；`:172` 重入闩 + 理由；`:168` 说出「没有这个入口会怎样」（P1）
- ⚠ **未核**：**服务端是否真回 `409 model_active`**（我只核了前端散文；Rust 侧 `models_routes` 错误码定义**未读**）⇒ **B0253 第一件事**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0253（4 目标 / 跨层对账 · **0 新发现** · ⭐ 对账通过）
- ⭐ `409` + `model_active` 在**四处同拼写** + 回归钉住：`handlers.rs:403-404`（**`StatusCode(409)`**）· `:375`（码）· `:376`（文案「**请先 activate 另一个模型**」）· `models_api.dart:153`（409 + 码）· `dev_tools_section.dart:254`（409 + 码）· `tests_models_handlers.rs:132/136`
- ⇒ ⇒ **「用户拿界面上的码去日志里搜」这条契约，在这一处完整成立**；且**两端都给了可执行的下一步**（UI **点之前**说 · 服务端 **拒之后**说）⇒ 「**提前说清 / 不让人白点**」家族里**唯一一个两侧都给路**的样本
- ⚠ **不外推**：这类「散文说的码」**目前对得上 1/1**，但**样本太少** ⇒ **不能据此说「这类都靠得住」**；`models_routes` 其它码（`model_not_found`/`persist_failed`）**是否也被前端引用未核**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0254（3 目标 / 跨 3 模块 · **0 新发现** · ⭐ 把两类东西分开数）
- `model_not_found`（`models_routes/handlers.rs:133/:254/:326/:371` + 回归 `:108/:276`）与 `persist_failed`（`settings_routes/mod.rs:212/218/223` · `env_routes.rs:147` · `models_routes/handlers.rs:408`）⇒ **两者都只存在于服务端，前端散文零引用**
- ⇒ ⇒ **它们根本不是「散文 ↔ 码」的对** ⇒ ⇒ **B0253 的对账样本仍是 1、没有增长** ⇒ **不把两类东西混着数**
- ⭐ 而 `persist_failed` 验证的是**「码的一侧收敛」**（**P2 第四种落点**）：**一个码跨三个端点模块复用** ⇒ **一次搜索命中三类失败**；代价是「哪个端点」不在码里 ⇒ 而 **B0198 已核 `dispatch.rs` 会把 `route` 记进日志** ⇒ **两维（码 + route）合起来仍可搜** ⇒ **正确的分工**
- ⇒ **P2 的落点至此四种**：一致性（B0215）· 安全属性（B0236）· 资源上界（B0240）· **码的一侧收敛**（本批）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0255（机械对账 · ⭐ **P3 1** · 样本 1 → 12）
- 机械对账：Dart 侧 **18 个**「像错误码」的 token 逐个回 Rust 查同拼写 ⇒ **12 对得上**；6 个查不到里 **4 个不是错误码**（`active_id`/`prev_active_id`=**JSON 字段名** · `error_banner`=**文件名** · `invalid_use_of_protected_member`=**Dart 分析器诊断码**）⇒ **我的关键词过滤器有假阳性**（如实记）
- ⭐ **F-0255-01（P3）**：真正对不上的两个**都是前端自造** —— `network_error`（**6 处** API 包装器在「请求没到服务端」时抛）· `turn_failed_no_detail`（「失败但没带回码」的占位）⇒ **Rust 侧两者零命中**
- ⇒ ⇒ AGENTS「**拿界面上的码去日志里搜**」对这两个码**保证落空** ⇒ **不是码写错，是事件没到服务端** ⇒ **定 P3**（无功能影响；只影响「搜日志」这一步，而它**在排障最需要时白跑一次**）
- 建议 (a) **把这条边界写下来**（与 B0222 点名**不存在的 API**、B0244 点名**硬约束**同形）；(b) 或让前端本地记一行同名 code
- ⭐ 顺带正面：这两个码在**六处**包装器里**拼写一致** ⇒ **前端这一侧整齐**，缺的只是**与服务端的边界说明**
- ⚠ **另纠正我自己的计数脚本**：它同时打印两个数（`len(cur)-len(wd)` 与**分级 Counter 之和**）**曾差 1** ⇒ 我此前引用的是**错的那个** ⇒ **口径定为「分级 Counter 之和」**（可加、可核）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0256（1 文件 / 定点读 · **0 新发现** · ⭐ P2 第五落点）
- ⭐ `models_routes/handlers.rs:399-401` `not_found_response(code, message) => json_error_response(**StatusCode(404)**, code, message)`；四个 `model_not_found` 站点（`:133/:254/:325/:371`）**全走它** ⇒ **该码在任何站点恒为 404** ⇒ **没有站点能漂移**
- ⇒ ⇒ **码↔状态的配对是结构上单值的**（比 B0254 的「出现在 4 处」更强：那次是**计数**，这次是**收口**）
- ⇒ ⭐ **P2 第五落点（最纯）**：**状态与码的绑定由一个函数保证** ⇒ 任何调用点都改不了那个状态码，**除非改那个函数** —— 而**改那个函数是显眼的改动**
- ⇒ 与 **B0159 `ErrorKind::code()`** 是**方向相反的同形**（那里**码由变体算出** · 这里**状态由帮助函数写死**）⇒ **两侧都是「让配对由一处保证」**
- ⚠ **最后一块地基未核**：`json_error_response` **本体**（会不会改写 `code`）⇒ B0257 第一件事
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0257（1 文件 / 定点读 · **0 新发现** · ⭐ 地基更硬）
- ⭐⭐ `json_error_response(status, **code: &'static str**, message)` ⇒ `ErrorDetail::new(code, message)` **原样传、不改写** ⇒ ⇒ **B0256 的结论成立**（该码在任何站点恒为 404）
- ⭐⭐⭐ **而更硬的一层是那个类型**：**码是编译期常量** ⇒ **调用点无法传入运行时算出来的码** ⇒ ⇒ 「四个站点拼写一致」**不只是纪律，而是**类型**保证的** ⇒ 这是 **P2 第五落点的实现机制**
- ⭐ 另两点：`ErrorDetail` 是**跨错误面共用的载体**（与 B0015 核的 WS 侧同类型）⇒ **错误细节只有一个类型**；`to_vec(…).unwrap_or_default()` ⇒ **序列化失败给空体、不 panic**（与 B0117/B0138 同取向）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0258（3 目标 / 定点读 · **0 新发现** · ⭐ 一个看着危险的口）
- `dto.rs:157-164` `ErrorDetail { code: &'static str · message: String · details: Option<Value> }`，而**文档点名 `details` 可含 `value`** ⇒ 看着是自由字段的出口
- ⭐ `security.rs:255-259`（**mutating 校验 403**）⇒ 只塞 `{"status": …}` ⇒ **没有回显被拒的 body** —— 而这**正是**最容易被写成「回显被拒内容」的那条路
- ⭐ `settings_routes/mod.rs:189-193`（`api_key_env` 非法）⇒ 只塞 `{"section": llm|tts}` ⇒ **不回显那个非法的变量名**
- ⇒ ⇒ ⭐ **「有一个自由字段」本身不是缺陷，「往里塞了什么」才是** ⇒ **红线 R 在这个口上关着**
- ⚠ **不夸大**：`message: String` 是**另一个自由文本口**，而 `:186` 判的是 `msg.contains("api_key_env 非法")` ⇒ **那个 `msg` 会不会带上用户填的值，本批明确未核** ⇒ **下一批第一件事**（**两个自由字段，一个核了、一个没核**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0259（1 目标 / 定点读 · **0 新发现** · ⭐ 两种「码↔文案」形态的对照）
- `settings_routes/mod.rs:185`「**apply_patch 返回的 String 形态错误码语义不细；按消息前缀判**」⇒ ⇒ **码↔文案绑定 = 在人类可读字符串上做子串匹配** ⇒ **脆弱**（改措辞 ⇒ 码静默变错）
- ⇒ ⇒ **但不是疏漏** —— 它**在决策点被写明了** ⇒ **已知的、被接受的限制**；⭐ **与 B0159 `ErrorKind::code()`（码由变体算出）构成鲜明对照**：**一边「码是算出来的」，一边「码是从措辞里猜出来的」**，**而两边都把自己的形态写在脸上**
- ⇒ ⭐ **可提炼**：**知道自己弱在哪、并写在旁边，比假装强更有用**
- 另：`message` **可能**回显用户填的**变量名**（**非密钥** ⇒ **不违反红线 R**）；而 **B0123 已核 `SettingsView` 刻意不暴露 `api_key_env`**（只给 `has_api_key`）⇒ **口径不对称** ⇒ **不记发现**（值非法时告诉用户「你填的是什么」正是用处）
- ⚠ **未逐字核**：`api_key_env 非法` 那条文案**是否真带值**（B0122 读到的是 `base_url` 那条的形态）⇒ B0260 第一件事
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0260（3 站点 / 定点读 · **0 新发现** · ⭐ 闭合「脆弱」那条链）
- ⭐ `settings.rs:78` / `patch.rs:334`「api_key_env 非法: **{name:?}**（仅允许 ASCII …）」⇒ ⇒ **文案确实带上值**（Debug 形态）⇒ **但值是环境变量「名」、不是密钥** ⇒ **不违反红线 R**（B0259 的判断得到逐字确认）
- ⭐⭐ **而两侧措辞共用不是巧合** —— desktop 侧**正是靠这个字符串做前缀匹配**（`settings_routes/mod.rs:188`）⇒ ⇒ **两侧收敛是「前缀匹配能成立」的前提**
- ⇒ ⇒ 「码靠措辞猜」**今天成立，是因为措辞被刻意收敛**；**一旦只改一侧，匹配就静默失效** ⇒ ⇒ **B0259 那句「语义不细；按消息前缀判」正是他们为这份收敛付的价、且自认的价**；文案还**顺带说明了规则**（「仅允许 ASCII …」）⇒ **是诊断，不只是标识**
- ⚠ **这条链唯一剩下的敞口**：**两侧措辞「总是」同步没有机制强制**，只靠人
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0261（1 文件 / 定点读 · **0 新发现** · ⭐ 红线第四个落地层次）
- ⭐ `message_bubble.dart:264-276` → 复制取 **`message.text`（原文）而非渲染 span** ⇒ 与 B0229「纯文本不进 `Text.rich`」**同源** ⇒ **显示与复制两条路都取原文**
- ⭐⭐ 而**隐藏复制能力的理由引用了本仓自己的语音规则**：`:266-268`「流式期间文本还在变，复制到的会是**半句话**（按『**一句一单元**』的口径，**半句话本身也不该被当成一轮的产出**）」
- ⇒ ⇒ **「一句一单元」已落地在四个层次**：**提示侧**（B0116）· **装配侧**（B0132）· **上界侧**（B0243）· ⭐ **界面侧**（本批）⇒ **各自落地、且各自引用它** ⇒ **该约定是活的，不是归档文字**
- 另两处正面：`kMessageBubbleSurfaceKey` 带注释「**测试按它读约束，不靠 widget 类型猜**」；阅读宽度 `480` 的理由是**人因**（「中文一行超过 ~45 字之后，眼睛回找行首就开始费劲」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0262（1 文件 / 定点读 · **0 新发现** · ⭐ P26-b 第二例）
- ⭐ `chat_panel.dart:233-237` → 消息列表**已虚拟化**（`ListView.builder`）⇒ 与 B0227 核过的日志列表**同一纪律**
- ⭐⭐ `:234`「**不**给 `itemExtent`（**气泡高度可变**）」⇒ ⇒ **P26-b 第二例**（B0235 三例外之后）⇒ **该模式从观察变成惯例**
- ⭐ `:240` `visible[length - 1 - index]` = **倒序、最新在前** ⇒ 与 B0246 记忆面板「**最新在前**」**同一排序约定**（**约定层面的 P2**，跨两个特性）
- ⭐ 而 `:25` 上限的理由（**语义树**与**读屏浏览**）与这里的虚拟化**是同一个关切** ⇒ **两个决定其实是同一个决定**（正因它是**带语义树的真列表**，才需要上限）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0263（1 文件 / 定点读 · ⭐ **P3 1** · 模式 Q 第四次拦截）
- ⭐ **F-0263-01（P3）** `live2d_stage.dart:366-372` → `presetStatus` 的**唯一写者**是 `presetStatusUpdateFor`（只吃 ack），`clear` 只来自 `replaced/expired/dropped`；`_handleHostError`(:380-382) **只设 `_hostError`**
- ⇒ ⇒ **重建后新渲染面没有正在播的动画 ⇒ 不会发 `expired/dropped`** ⇒ **旧快照可能一直留着**（**B0251 的未决点结清**）⇒ **修法一行**（重建路径显式清空）⇒ 便宜**正因为 `clear` 已是一等动作**
- ⇒ **不升 P2**：B0232 已核它是「**显示快照**」⇒ **无行为依赖** ⇒ 危害限于「诊断区显示已不生效的状态」
- ⭐ **可推广**：**「清理由 ack 驱动」在「重建后**没有东西可清**」的场景下会失灵** ⇒ 纯函数+单一写者（P27 同族）是好的，**但它有边界**；**边界必须被写下来** ⇒ 建议字段文档补「**仅由 ack 更新；重建后需手动清**」
- ⚠ 另：`:65` 的 `..clear()` 是 **`ActionCue` 的 map**，**与 `presetStatus` 无关** ⇒ 两处都核过、不混为一谈
- ⚠ **模式 Q 第四次拦截我的同一形态违规**（`F-0255-01` 缺规范头）⇒ 四次复发形态相同：**急着写正文、忘了先写头** ⇒ **这道检查第四次发挥了作用**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0264（1 文件 / 定点读 · **0 新发现** · ⭐ 排除一类真实失败模式）
- ⭐ `memoryBucketNotice`：**一个定义**（`memory_panel:55`）· **一个生产调用点**（`:577`）· **测试调的是同一符号**（`test:181-187`）⇒ ⇒ **「测试的纯函数 ≠ UI 调的那个」这一类真实缺陷在此不成立**
- ⇒ ⇒ **B0246 核的那两句文案，就是用户看到的那两句**（不是「被测的那两句」）；且 `:187` 传 `'  s-1  '` ⇒ **顺带钉住 trim**
- ⭐ **与 B0236 构成「两个都对的形态」**：`memoryRecordText`（**显示与删除确认共用** ⇒ 确认框不可能显示出不同东西）· `memoryBucketNotice`（**单一调用点 + 被测** ⇒ 不可能分叉）⇒ ⇒ **「共用」不是唯一正确，「单一且被测」同样正确**（**P25：看约束在哪一层保证**）
- 顺带（**非缺陷**）：测试名比覆盖面**窄**（名未提 trim、体钉了 trim）⇒ **多覆盖不缺** ⇒ **不记发现**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0265（2 目标 / 双向确认 · **0 新发现** · ⭐ 撤回已被验证）
- ⭐ `mod_count_is_three` 在 AGENTS.md 只有 **`:105` / `:608`** 两处，**都在变更历史里、且都写成「three **→** five」** ⇒ ⇒ **B0211 的撤回是对的**
- ⭐ 而**规范性位置**（休眠台账 **`:310`**「护栏是两条断言：`main.rs::mod_count_is_five`」、`:168`）**写的也是 `five`** ⇒ ⇒ **完全自洽**（代码唯一真实符号：`main.rs:471`）
- ⇒ ⇒ **两个方向都有结论**：否定方向（`three` **不是**过时的断言名）· 肯定方向（**规范段没有漂移**）⇒ **不再「不敢确定」，而是「已核，自洽」**
- ⇒ ⭐ **B0218 那次 `head -3` 截断的代价有界**：信前缀 ⇒ **误报**；不核就收工 ⇒ 撤回**悬着**；**这次全文 grep 把两端都定了**
- ⇒ ⭐ **方法论**：**「撤回一条发现」和「确认一条发现」需要同样强度的证据** —— 撤回必须**核到规范段**，而不只是核「那个词还在不在」
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0266 · ⭐⭐ **Phase 4 开张**（攻 F-0002-01）
- **攻法**：找 `supervisor_opt` 的产出 + 是否有**第二个** watcher 创建点
- ⇒ `cli_entry.rs:119` **`supervisor_opt` 为 `None` ⟺ `live2d-ai.toml` 尚不存在**（第一次运行）；`FileWatcher` **唯一构造点**在那个 `if let Some(sup)` 块内（`:337`）⇒ **没有第二个 watcher** ⇒ **机制成立**
- ⇒ ⭐ **但影响是「窗口」不是「常态」**：只首跑 · 只影响 `.env` · **自愈**（toml 落盘后下次启动即生效）⇒ **我原措辞「首跑无热重载」过宽** ⇒ 准确表述：**「第一次运行时 `.env` 的改动要等到下次启动才生效」**
- ⇒ ⇒ **级别仍 P1**，但**降为「首跑窗口内」**；⚠ 反证 (a)「别处会补 `refresh_from_disk`」**未核**（PATCH 路径是 UI 显式调用，非文件监视）
- ⭐ **且加重**：反证 (b)「首跑用户不会改 `.env`」**不成立** —— 首次配置流程**正是**「建 toml + 写 key」⇒ **顺序上极可能正好撞上这个窗口** ⇒ **危害是流程默认路径，不是假设**
- ⇒ ⭐⭐ **Phase 4 判据（本批确立）**：**攻一条发现必须产出 ①推翻 ②收窄 ③加重 三者之一** ⇒ **「没攻动」不算结果** ⇒ **本批 = ②收窄 + ③加重**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0267 · ⭐ Phase 4 攻 F-0006-03（最强一击失败 + 前提收窄）
- **攻法**：**若锁是 `parking_lot::Mutex` ⇒ 根本没有毒化 ⇒ 本条该被推翻**
- ⇒ `mod_registry.rs:34` `use std::sync::{Arc, Mutex}` · `:85` `Arc<Mutex<…>>` ⇒ ⇒ **不是 `parking_lot`** ⇒ **毒化是真的** ⇒ **最强一击失败** ⇒ **机制成立**
- ⇒ ⭐ **但前提被收窄**：准确前提是「**某个 Mod worker 在持有该 Mod 的槽位锁时 panic**」（**不是** `blocking_mods` 那把锁、**不是**别的线程的 panic）⇒ **比原措辞窄得多、也硬得多**
- ⚠ **③ 加重那一半未证**：三个站点全是裸 `.lock().unwrap()`（`:334` 写 / `:434` take / `:617` take）；若 worker 侧**没有** `catch_unwind` ⇒ **毒化与 worker 死亡同时发生**（会加重）—— **但「worker 循环处是否有 catch_unwind」本批明确未核** ⇒ **记为敞口、不并入结论** ⇒ ⭐ **这正是判据的用法：不把「可能」写成「已证」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0268 · ⭐ Phase 4 结清 F-0006-03 的 ③ 加重
- `grep -n "catch_unwind" mod_registry.rs` ⇒ **零命中** ⇒ ⇒ **Mod worker 循环没有 `catch_unwind`**（**整个文件都没有**）
- ⇒ ⇒ ⭐ **一次 panic 同时做两件事**：**杀死 worker** + **毒化它当时持有的那把槽位锁** ⇒ **两半同时发生** ⇒ HTTP 侧三处裸 `.lock().unwrap()`（`:334`/`:434`/`:617`）**没有兜底**
- ⇒ ⇒ 并把 **B0117** 的「hook 侧没有 catch_unwind」**扩展到整个文件**；**B0267 收窄的前提仍成立** ⇒ 两者合起来是 F-0006-03 的**完整形态**
- ⭐ **Phase 4 的节奏示范**：**上一批留的敞口，下一批只查那一个**、不做别的 ⇒ 成本可控、结论可追
- ⚠ 仍未核：**能否点名哪些 Mod 会在持锁期间 panic**；即便点不出名，**兜底缺失本身已成立** ⇒ **建议不变**（三处 `.unwrap()` 换成毒化后仍能取回数据的写法）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0269 · ⭐ Phase 4 攻 F-0001-01（措辞过宽 ⇒ 收窄）
- **攻法**：AGENTS 说「请求级日志在 `dispatch.rs`」⇒ 若那条日志**足够早或足够全**，本条该被推翻
- ⇒ `dispatch.rs:62 match_route` → `:65 is_mutating_route` → `:76 tracing::warn!` → `:88 match route` ⇒ ⇒ **路由先于日志**；而 **B0198 已核那条日志按状态阈值发**（≥500 error / ≥400 warn / …）⇒ **必然在「响应已知」之后** ⇒ ⇒ **纯前驱失败才真的没有记录**
- ⭐ **我原措辞「pre-dispatch endpoints 产生零条日志」过宽** ⇒ 这四个路由**只要返回了响应**（哪怕 4xx/5xx）**`dispatch.rs` 仍会记它** ⇒ ⇒ **收窄为「自身零日志 + 兜底只在响应之后」**
- ⇒ ⇒ **级别维持 P1**（那正是 AGENTS 禁止的「排障时一片空白」），但**范围从「这些端点」收窄为「这些端点的*无响应*失败」**
- ⇒ ⭐⭐ **Phase 4 的第二种收益**：B0266 拿到**边界** · B0267 拿到**前提** · **本批拿到「我自己说宽了」** ⇒ **三批把三条 P1 各修一次，而级别一条都没降** ⇒ **Phase 4 的主要作用不是找新问题，是把已有结论变得更准**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0270 · ⭐ Phase 4 攻 F-0013-01（最强一击失败 + ③ 加重）
- **攻法**：若 `mods.json` 是**内部产物**、不该由人编辑 ⇒ 「未知 id 丢失」无害 ⇒ 该推翻
- ⇒ ⭐ **被文档挡回来**：`docs/external-input.md:125`（**给出 JSON 片段教人写**）· `:128`（「**或改 `mods.json`**」把手改列为**与 API 并列**的选项）· `:127`（文件不存在时用内建 manifest）⇒ **`mods.json` 明确是可手写的配置面**
- ⇒ ⇒ 而 `mod_registry.rs:252-253` 整份从 `self.entries` 重建 ⇒ **手写的 id 不在 `entries`** ⇒ **第一次落盘**（任何配置保存/启停）**静默抹掉** ⇒ ⇒ ⭐ **③ 加重**：不是「可能发生」，而是「**文档正好叫你这么做**」
- 反证排除：「程序原子写 ⇒ 用户不会手改」**不成立** —— **原子写只保证「程序写时不撕裂」，不禁止人先写**
- 同类提醒：`persona-mod-cards.json`（B0220）走「**规范化后落盘 + tmp+rename**」⇒ 那类**程序独有的文件**不该照 `mods.json` 的重建法处理
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0271 · ⭐ Phase 4 攻 F-0001-02（重核当前树 ⇒ 攻击失败 + 红线正面证据）
- **攻法**：若 `assets/models/bai` **现在存在** ⇒ 那个 `||` 的后半段变承重 ⇒ 本条该推翻
- ⇒ `test -e assets/models/bai` **不存在** · `ls` 只有 `README.md` · `git ls-files` 只有 `.gitkeep`+`README.md` ⇒ ⇒ **273 批后仍成立** ⇒ **攻击失败** ⇒ **该 `||` 后半段仍是空的** ⇒ **`assert` 实际从未校验过那张 model3.json**
- ⇒ ⭐ **顺带一条独立正面**：`assets/models` **无任何被跟踪的二进制** ⇒ 与 AGENTS「**不捆绑二进制**」**完全一致**（**不是因为本条才去看**）
- ⇒ ⚠ **而本批最大收获在「我差点犯的错」上**：我**差点直接引用**「B0001 记过不存在」⇒ ⇒ **那正是 CONSOLIDATION-18 禁止的事**（**关于世界的事实会过期；前提必须重核**）⇒ ⇒ **本批的收益是那次教训的兑现**
- ⇒ ⭐ **推广**：**凡以「世界状态」为前提的发现，都该这样重核一遍**（`F-0046-01` 依赖构建产物布局 · `F-0049-01` 依赖 CI 内容）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0272 · ⭐ Phase 4 攻 F-0049-01（两半仍是事实 + 修法收窄到一句话）
- 「**GitHub CI 额外会跑 Gitleaks 覆盖完整历史**」仍在（`:5`）· `grep` 整个 `.github/` **零命中**（既没调它，**也没有 Gitleaks**）⇒ ⇒ **274 批后两半都还是今天的事实** ⇒ **攻击失败**
- ⇒ ⭐ **② 收窄（修法更紧）**：脚本**自陈范围**是 `crates/`+`xtask/`+顶层配置/脚本/测试、**跳过二进制** ⇒ ⇒ 「`INCLUDED_PREFIXES` 没有 `shell/`」**不是列表的 bug、是范围选择、与自述一致**
- ⇒ ⇒ 而**覆盖 `shell/` 的正是那句 Gitleaks 承诺**，而它不成立 ⇒ ⇒ **真正的缺陷就是一句话** ⇒ 修法只剩：① **加真的 Gitleaks 步骤** ② **或删掉那句话**（并说明 `shell/` 不在范围）
- ⇒ ⇒ ⚠ **而不是**「补 `INCLUDED_PREFIXES`」—— 那会**悄悄改变脚本自陈范围**，**比原状更糟**；⭐ **可提炼**：**修「文档说 X」时先分清「X 是范围选择」还是「X 是错误承诺」** —— **改错了比不改更坏**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0273 · 🔻 **撤回 F-0020-01（P1）** · Phase 4 第一次推翻
- **攻法**：若那个 `MAX_TOKENS` **就是主链的预算** ⇒ 我的批评成立；**反之前提就错了**
- ⇒ ⭐ `performance/client.rs:1`「**表演层** HTTP 客户端」· `:4-6`「**独立配置**…**独立于 `[llm]`（主模型）**；**二路断了不影响主链**」⇒ ⇒ **我原来的框架前提不成立** ⇒ **撤回**；而主链的默认在 `settings.rs:187 DEFAULT_MAX_TOKENS = 4096`（**另有其人**）
- ⇒ ⭐ 而「**一场调用一个预算**」是这一层的**既有设计**：`director/staging_http.rs:48 = 512` · `memory/summary.rs:54 = 512` · `performance/client.rs:139 = 1_024`；且这一路返回**一份 JSON 动作计划**（`:1`「要的是一份 JSON，**不是逐字流**」）⇒ **比对话小是合理的**
- ⇒ ⇒ **我没有任何证据说 1024 不合适** ⇒ 按「**拿不出证据就不进 FINDINGS**」**撤回**；⚠ 重启它需要的**不是**与主链比较，而是「**一份动作计划 JSON 实际需要多少输出 token**」⇒ **仓库里拿不到** ⇒ **永久敞口**
- ⇒ ⭐⭐ **审计第三次撤回**（B0069 F-0030-01 · B0211 F-0040-01 · **本条**）· **Phase 4 第一次「推翻」** ⇒ ⇒ **攻自己不是走过场**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0274 · ⚠ **F-0046-01 的前提「移动了」** ⇒ 两条都不说
- 三次 grep 全零，而**第三个零最有信息量**：**连 `postMessage` 本身都不在 `main.rs` 里了** ⇒ 全目录搜只命中 `web/surface/input.rs` 的两处**注释**（`:43`「**任何能 postMessage 到它的东西都能**」= **正是本条描述的威胁，被写成了已知属性**）
- ⇒ ⇒ 我引用的 `main.rs:380-386` / `:705-707` **在当前树上都不存在**；**原因清楚**：AGENTS rc.3 ②「`surface.rs` 1167→**24**（render/input/idle/gpu）」⇒ **文件被拆过**
- ⇒ ⚠ **本批真正的价值是「不下结论」**：三次零命中说明的是**代码移走了**，**不是**「那里没有 origin 检查」⇒ 若照惯例写「前提重核、仍然成立」⇒ **那是我没核过的话**
- ⇒ ⭐ **B0271 的规则第二次拦住我**：第一次拦住「**凭记忆**」，这次拦住「**凭零命中**」⇒ **两条是不同的失效**（① 凭记忆 ② 凭零命中）⇒ **共用的对策：顺着变化的痕迹去找，而不是在原地再搜一次**
- ⏰ **下一批第一件事**：对着 `web/surface/input.rs` 重核（`origin` 检查是否在那里、`targetOrigin` 现在是什么）⇒ **在做完之前不得声称本条成立或不成立**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0275 · 🔻 **撤回 F-0046-01（P1）** · 第 4 次撤回 / Phase 4 第 2 次
- `input.rs` 里的 `postMessage`/`origin`/`fetch_bytes` **只有两条注释**（`:43` `:61`）⇒ **wasm 源码树真实调用 = 0**；`crates/l2d-wasm-demo/dist/*.js` **`git ls-files` 空** ⇒ **未跟踪的过期产物**
- ⇒ ⭐ **幸存的那一侧用的是具体 origin**：`live2d_host_web.dart:78` `target.postMessage(json.toJS, **web.window.location.origin.toJS**)`
- ⇒ ⇒ **我引用的 `main.rs:380-386` / `:705-707` 都不存在了**（文件被拆过）⇒ 且 ⭐ **连修法 ③ 要调的 `normalize_resource_ref` 也是 0 命中** ⇒ ⇒ **发现与修法都锚在一个已不存在的树版本上** ⇒ **永久撤回**（同 F-0040-01 判据）
- ⇒ ⚠ **不夸大**：**是因为对象不存在而撤回，不是因为问题被证明不存在** ⇒ **留下的是「这一版没有这条通道」，不是「已修复」**
- ⇒ ⭐⭐ **三条失效形态，三条对策合成一句**：① **凭记忆**（B0271）② **凭零命中**（B0274）③ **凭一次 grep**（本批追了四步：注释→整树→跨 crate→产物）⇒ ⇒ **不要在原地下结论，要顺着痕迹往外追，追到能定性为止**
- ⇒ ⭐ **Phase 4 战果（攻 9 条 P1）**：**撤 2 · 收窄 4 · 加重 1 · 复验 2** ⇒ **两条被攻掉，剩下五条一条都没降级** ⇒ **攻自己提高了结论质量，而不是制造工作量**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十八次对账（CONSOLIDATION-19）· ⭐⭐ **Phase 4 攻掉自己 9 条 P1**
- **7 条 P1 复核：7/7 仍成立**（被攻掉的两条不在核验表 —— 它们的对象已不存在）；【模式 Q + KNOWN-LOSSES】**✔ 通过**
- ⭐⭐ **Phase 4 战果**：**① 推翻 2**（F-0020-01 前提错=表演层自己的客户端 · F-0046-01 对象已不存在=wasm 树 `postMessage` 真实调用 0）· **② 收窄 4**（F-0002-01 边界 · F-0006-03 前提 · F-0001-01 措辞 · F-0049-01 **修法收窄到一句话**）· **③ 加重 1**（F-0006-03 `catch_unwind` 零命中 ⇒ 毒化与死亡同时发生）· **复验 2**（F-0001-02 · F-0049-01 重核世界状态仍成立）
- ⇒ ⇒ **两条 P1 被攻掉，剩下五条一条都没降级** ⇒ ⭐ **攻自己提高了结论质量，而不是制造工作量**
- ⭐ **四次撤回、三种理由**：**我漏看了**（F-0030-01）· **我误读语义**（F-0040-01：那两行**描述一次变更**）· **前提错**（F-0020-01）· **对象已不存在**（F-0046-01）
- ⭐⭐ **最可迁移的产出：三条失效形态 → 一条对策** —— ① **凭记忆** ② **凭零命中** ③ **凭一次 grep** ⇒ ⇒ **不要在原地下结论，要顺着痕迹往外追，追到能定性为止**
- 本轮立档 **P25–P28**（P25 另加**适用条件**：收敛的代价也算理由）+ **P2 的六种落点** + **「上界」三族** + 判据「**撤回与确认需要同样强度的证据**」
- 覆盖率（**机械**：313 条去重路径）：Rust `.rs` **81** · 前端 `.dart` **37/218**（最大面）· CI/脚本 4
- ⭐ 自评：**我这一轮最值钱的产出不是「找到了什么」，而是「推翻了自己两次」** ⇒ 而**推翻的前提是先承认「我上一轮的话可能不成立」** ⇒ **一份从不被自己推翻的审计，读者无法判断它的置信度**

### BATCH-0277 · 🔻 **更正我自己撤回里的一句话**（B0274 规则第三次生效）
- 我在 B0275 写「**这一版没有这条通道**」⇒ ⇒ **这句是错的** ⇒ **通道在、只是形态变了**：`input.rs:100-102`「**`stage-bg.dataUrl`**」⇒ `pub bg_data_url: Option<String>`
- ⇒ 三段形态：**发送侧**（Flutter→iframe）**仍 `postMessage`**，但 `live2d_host_web.dart:78` 带 **`location.origin.toJS`**（**不是 `"*"`**）· **接收侧**是**版本化 JSON 消息字段** · **回程**是 **`emit_event`**（`main.rs:151/618/675/682`），**不再由 Rust 调 `postMessage`**
- ⇒ ⇒ **撤掉的是「我引用的那段代码」与「Rust 侧直接调 postMessage」，不是「这条通道」**
- ⇒ ⚠⭐ **B0274 规则第三次现身，且这次反噬一条**看起来严谨的撤回**：**我为了让撤回显得干脆，把「我没找到」写成了「它不存在」** ⇒ **比原发现错得更隐蔽**
- ⇒ ⭐ 顺带一条**未核实**：`input.rs:101` 说背景走 **canvas CSS background-image**，而 **AGENTS rc.4 ⑦ 逐字说该写入「已删除」** ⇒ **未读真正应用它的代码 ⇒ 不记发现**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0278 · ⭐ **F-0278-01（P3）** · B0277 的「未核实」结清
- ⭐ **结清**：CSS vs framebuffer ⇒ **framebuffer 是对的**。`web/surface.rs:8` + `background.rs:1-3`（「**画进 framebuffer，而不是依赖 canvas 的 CSS**」）+ `background.rs:5-13`（wgpu 29 只报 `Opaque` ⇒ 透明 clear 变**不透明黑** ⇒ **盖住 CSS**）+ `input.rs:236-247`（「**不在这里**画」+「**实测证伪**」）⇒ **四处一致** ⇒ **AGENTS rc.4 ⑦ 不是漂移**
- ⭐ 而**唯一说 CSS 的那一行就在这个文件里**：`input.rs:100-102`「**应用为 canvas CSS background-image**」⇒ ⇒ **一个消息结构体的字段注释在描述一条已被本文件证伪的路**
- ⇒ ⇒ **影响不止「文案陈旧」，而是会把人引到错误的层去查**（真正的落点在 `background.rs` 的**背景预通道**）⇒ **修法两行、零风险** ⇒ **定 P3**（实现是对的）
- ⇒ ⭐⭐ **真正教训在结构上**：**一个文件里同时存在「过时的说法」与「它被证伪的记录」**，⇒ **只搜字段名的人只拿到旧的那段** ⇒ **作废的说明要就地改掉，不能只在别处记「已作废」**；与 B0212 正好相反（**那次更正写在正文、注释留旧；这次正文已更正、注释还是旧的**）⇒ **两种都要治**
- ⚠ **未证**：`apply` 的分支体未读 ⇒ 「某路径上 CSS 仍生效」未证；但该注释**没说「仅在某条件下」** ⇒ **至少不完整** ⇒ **不因未证例外而上调级别**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0279 · ⭐ 「必须保留」的不变式写在拆分现场（0 新发现）
- ⭐ `web/surface/idle.rs:2-6` → 「与动作系统是**两套机制**：动作层**已整体删除（rc.2）**，本模块是产品路径上待机体征，**必须保留**」⇒ ⇒ **AGENTS 标了「绝不可删」的东西确实还在，且理由写在原地、写在拆分现场**
- ⇒ ⭐ **它点名了造成风险的那次删除**（「动作层已整体删除」）并**记录了那次删除的后果**（`:12-15`「**其上不再有动作 `final_override` 层**」）⇒ ⇒ AGENTS 台账说「只共用 override 层」，**而那层现已不存在**
- ⇒ ⭐⭐ `:12-15`「breath/blink 参数…**仍然只由这里驱动**」⇒ ⇒ **一个状态、一个写入者、名字写在文件里** ⇒ ⇒ **「唯一写入者」纪律本轮又见一例**（`presetStatus` 只由 ack 驱动 B0232 · `presetStatusUpdateFor` 唯一写者 B0268 · `canPreserveStage` 不建镜像 B0116 · 事件面 9 臂无兜底 B0135）
- 另：保留**出处与许可**（照抄 py 版三个 `idle-*.ts`，**MIT**）⇒ **跨重写仍留署名**（本仓另有 `mod-community-license.md`）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0280 · ⭐ F-0278-01 敞口结清（0 新发现）+ 一个强正面样本
- **「全 wasm 树还有没有活的 CSS 背景写」⇒ 没有**：六处命中里 `:101`（过时注释）· `:239`（正确说明）· `background.rs:203`（**都是注释**）；**唯一含 `background-image` 字面的活代码在 `#[test]` 黑名单里** ⇒ ⇒ **F-0278-01 成立、不扩大**
- ⇒ ⭐ **冒出来的第二个注入面是守得最严的一处**：`input.rs:295-314`「**注入面**：这个值会进 `style.setProperty("background-color", …)`，所以**只认纯十六进制**」+ `stage_color_rejects_anything_that_is_not_plain_hex` 逐条列**五类**反例（形状 · 字符集 · CSS 关键字/函数 · 外链与 `var()` · **声明拼接 / 规则逃逸 / `expression()`**）
- ⇒ ⭐ **注释先说「这个值会去哪里」、再说「所以只认什么」** ⇒ **先知道去处、再定判据** ⇒ 与 **B0213 互为镜像**（那边「先挡 `NaN` 再转换」）⇒ **同一条纪律两种用法**；「拒绝 ⇒ `None` = **回落默认色**」⇒ **失败有确定落点**（同 B0238）
- 另：`render.rs:4-6` 把**四种 Surface 状态 + 「Validation 连续两次视为 fatal」**写在头注 ⇒ **又一次「阈值写在决策点」**（同 B0242）
- ⏰ 下一批第一件事：**追 `render_to_view_submit_with_load` 在哪被调用**（AGENTS rc.4 ⑦ 说背景走它，而 `render.rs` 头注只写了不带 `_with_load` 的那个）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0281 · ⭐ B0280 那条线闭合（0 新发现）
- ⭐ **规则是写成禁令、放在边界上的**：`web/surface.rs:12-13`「实时路径只调用 **`render_to_view_submit`**（**不得调用阻塞的 `render_to_view`**——C1/C6/C8 **硬门禁**）」⇒ ⇒ **禁令带编号** ⇒ **可引用、可核对** ⇒ **又一次「禁令要指名被禁的那个」**
- ⇒ `l2d/src/renderer/model_core.rs:144-152`：`render_to_view_submit` **自己转调** `render_to_view_submit_with_load`（「**兼容入口：历史行为 = 清成全透明**（离屏读回 / 桌面壳依赖它）」）⇒ ⇒ **B0280 的疑问有解**：`_with_load` 在**下一层**（`render.rs:413` 调它）
- ⇒ ⚠ **但包装关系没有任何一处头注写明** ⇒ `surface.rs:12` / `render.rs:2` / `:349` 都说「只调 `render_to_view_submit`」，而 `render.rs:413` 真实调用是 `..._with_load` ⇒ **单看像自相矛盾** ⇒ **读者要自己推出包装关系**
- ⇒ ⇒ **不记发现**（不是缺陷，是**读法成本**），但**本审计自己就撞上了它** ⇒ 建议补一句「`render_to_view_submit` 内部即 `..._with_load`；实时路径只应经前者」，零风险
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0282 · ✅ F-0278-01 的链彻底闭合（0 新发现）
- `render.rs:406-413` → **背景预通道**（`background.draw(&mut pass)`）→ 提交 → **模型通道用 `wgpu::LoadOp::Load`**，且注释**点名默认会做什么、为什么错**：「预通道刚铺好的底色与背景图**必须留下**，**若仍用默认的 `Clear(TRANSPARENT)` 会把背景整块擦掉**」
- ⇒ ⭐ **P26-a 的加强版**（B0229 是「这不是微优化」）—— **这次直接点名「那个默认值」及其后果**
- ⇒ ✅ **八步链条全核过**：① 消息字段 `bg_data_url` ② ⚠**该字段注释说「走 CSS」**（**F-0278-01，唯一错的一步**）③ 真机制是画进 framebuffer ④ 同文件 `:236-247` 已写「实测证伪」 ⑤ 全树无活的 CSS 背景写（唯一字面在 `#[test]` 黑名单）⑥ 禁令写在边界、带 C1/C6/C8 编号 ⑦ 包装关系 ⑧ **模型通道 `Load`**
- ⇒ ⇒ **结论：一个 322 行的文件里，40 行实现与说明都写对了，错的只有那 2 行注释** ⇒ **F-0278-01 定 P3（两行注释、零运行时后果、修法两行）站得住**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0283 · ⭐ 「记录失败尝试」的最佳样本（0 新发现）
- ⭐ `gpu.rs:6-10` → 「曾把这里改成『WebGL2 优先』以换取透明 canvas，但 HUD 实测用户机仍走 WebGPU（`request_gl` **未生效**），且透明 canvas 在 WebGPU 下**结构性不可达**。舞台底色与背景图现在**画进 framebuffer**，因此**不再需要动后端顺序——恢复 WebGPU 优先**」
- ⇒ 五个可核点：**① 记的是失败尝试、且那段代码确实不在** · **② 写明失败的样子**（可核现象，非「效果不好」）· **③ 用「结构性」把「难办」升级成「做不到」** · **④ 写明新方案为什么取代了旧尝试**（新方案让老问题**不再存在**）⇒ **「换一条路」而不是「继续试」** · **⑤「不再需要」比「不要」更好**（说明约束**已消失**，而非「谁也别动」）
- ⇒ ⭐ **而它与 AGENTS rc.5 ⑥ 说的是同一件事、互相印证** ⇒⇒ **这正是 F-0209-01 的镜像**：**那里只有文档旧；这里代码与文档都写对了** ⇒⇒ **判据不是「文档可不可信」，是「这条事实有没有两边都记」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0284 · ⭐ 「不假装」家族第六例：两个近义否定被分开（0 新发现）
- `main.rs:18-20` 文件级**绝对**契约：「所有可失败步骤都收敛为 `Result<_, String>`……**绝不 panic、绝不静默吞错**」
- ⇒ 而派发面**确实在忽略**：`:505`「**未知 id → 静默忽略（不回执、不动参数）**」· `:508`「**旧渲染面忽略未知 type，故向后兼容**」· `:649` `_ => {} // 未知类型忽略`
- ⇒ ⇒ ⭐⭐ **两者不冲突，因为是两件事**：**「静默吞『错』」= 出了错却没报**（禁止）；**「忽略『未知 type』」= 没出错**，不认识扩展消息是**协议兼容**情形（**必须**）⇒ 且 `:508` 写了理由
- ⇒ ⇒ 而 `:505` **写明它不做什么**（「不回执、不动参数」）⇒ **不会半途生效**；另 `:136` 同规则在**非 iframe 场景**也适用
- ⇒ ⭐⭐ **可提炼**：**「不假装 X」必须精确到「不假装 X 而仍要做 Y」** —— **只写「绝不静默」会逼着实现去「回应」每条未知消息，而那会破坏向前兼容** ⇒ **分开的代价是多写两句，收益是两条规则都不会被误用**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0285 · ⭐ 「忽略未知」第四例、且跨语言（转前端，0 新发现）
- ⭐ `dev_tools_section.dart:338-340`「**未知 key 不隐藏**（回落原始 key）——未来 Mod / 新字段**不能因为前端不认**…」⇒ **理由是向前兼容** ⇒ **本轮第四例**「忽略未知」（`main.rs:505`/`:508`/`:136` + 本例）
- ⇒ ⭐⭐ **而它跨语言**：三例在 **Rust**、一例在 **Dart** ⇒⇒ **更强地说明这是「约定」而非某处的选择**
- ⇒ ⭐ **与「敏感 ⇒ 移除」并存且不冲突**（接 B0284「两个近义动作被分开」）：**未知 ⇒ 显示原始 key** vs **已知但敏感 ⇒ `obj.remove(key)`**（`mods_routes`，B0066）⇒ **「脱敏」针对「已知敏感」，「透传」针对「尚未知道」** ⇒ 后者保证**扩展不会被前端悄悄吃掉**
- ⇒ 与 **B0123**（视图字段对等性）**同处一区** ⇒ **三条同区结论互相不打架**
- ⚠ **本批 grep 打偏**：命中的全是 **Mod 状态视图**函数、**没有** `obscureText`/env 写入 ⇒ **密钥写入面在别的文件** ⇒ **下一批第一件事是定位它**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0286 · ⭐⭐ 本审计读到过最强的密钥处理实现（0 新发现）
- ⭐ `lib/settings/sections/env_key_field.dart:1-17`（B0285 定位到）⇒ 六条各堵一个洞：**不持有**（无法被误渲染）· **不请求**（与 B0120 后端只回键名一致）· **不显示**（只有「已设置/未设置」= **B0123 的 `has_api_key` 形状**）· **只往上走**（**没有路径能把密钥带回来**）· **键名不可随手填**（⇒ 堵掉「写了后端不读的变量名」这个**正确性**洞）
- ⇒ ⭐⭐⭐ **「每次保存后立即清空（不留明文在内存里等 GC）」** ⇒ ⇒ **这一条我在整个审计里没见过别处** ⇒ 它堵的是**残余窗口**：**明文留在 widget 里直到被 GC** 是真实（虽小）暴露，**而他们把它点名了** ⇒⇒ **超出红线 R 文档本身**
- ⇒ 另「它为什么存在」那段**先说用户可见的症状**（「改完配置**还是 401**，且**没有任何地方能改**」）⇒ 而**不是**抽象要求 ⇒ **又一例「先说人话」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0287 · ⭐⭐ 判据与理由是同一条（0 新发现）
- ⭐ `env_key_field.dart:75-76`「**立刻清掉明文：它已经写进 `.env`，没有理由继续留在输入框里**」⇒ ⇒ **清空的判据不是「保存动作完成」，而是「明文已经没有存在的理由」**
- ⇒ ⇒ **理由会随事实变化**：若 `.env` **没写成功，明文确实还有理由留着** ⇒ ⇒ ⭐ **「只在成功时清」不是独立规则，而是那条理由的推论**
- ⇒ ⭐⭐ **可提炼**：**把「什么时候做」绑到「为什么做」，正确的时机就会自己掉出来** ⇒ 比写「成功后就清」**更耐改**（换存储/换传输/换后端语义时**都不用重推时机**）
- 另三处：清空在 `await save` **之后** ⇒ **失败保留输入、用户不必重打** · `if (!mounted) return;` 在 await 之后（`:72`/`:83`）⇒ **B0230 纪律第四处** · 成功文案**区分两态**（「已清除」对齐 AGENTS「空值 = 清除」/「已写入…**立即生效**（不用重启）」对齐 B0120 的 reload）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0288 · ✅ 密钥写入链首尾读完（0 新发现）
- ⭐⭐ `shell_admin.dart:109-111` → `on ApiException`「**写成功了但读状态失败**：**不动现有状态**（`EnvKeyField` 自己的提示仍然准确）」⇒ ⇒ **它知道写入是成功的** ⇒ **不清空、不报错、不回滚 UI** ⇒ **失败被归因到正确的那一半**
- ⇒ ⇒ 而 **UI 的既有提示仍然是真的** ⇒ ⇒ **B0284「必须精确到『不假装 X 而仍要做 Y』」的又一实例**：**不说「失败要回滚」，而说「这一种失败不需要回滚，因为另一半仍然成功」**
- ⭐ `:96-97` **明确拒绝一句多余的话**：「后端写完刷新快照 + 重建 LLM/TTS client ⇒ 这里**不需要（也不该）**提示『重启』或『重跑 ignite.sh』」⇒ **防的是「加一句以防万一」** ⇒ **文档式的诚实**
- ⭐ `:100-101`「值**只在参数里出现**」+ 重读 `GET /api/v1/env`「**只为**刷新那个布尔」⇒ **不是「顺便同步一下」**；两个 `if (!mounted) return;` ⇒ **B0230 纪律第五、六处**
- ⇒ ✅ **整条链（跨 4 文件、两种语言）六步**：控件不持有/不请求/不显示 → 只往上走 → **写成功才清** → **拒绝多余的话** → 值只在参数里、重读**只为那个布尔** → **读失败不动状态** ⇒⇒ **每一处都带理由，且没有一处是「以防万一」加的**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0289 · ⭐⭐ 一个被点名的危险（0 新发现）
- ⭐⭐ `live2d_stage.dart:172`「…**loading，过早 `sync` 只能入队；重建 iframe 后队列已随旧桥销毁。**」⇒ ⇒ **队列的生命周期绑在「桥」上、不是绑在 widget 上** ⇒⇒ **一个比「桥」活得久的队列会静默丢掉全部** ⇒ **他们把这条危险写在回调的文档上**（而不是等它发生）
- ⇒ 自愈机制随之写明：`:131-132` 偏好快照是「**iframe 重建 / 重挂后的自愈快照**」，在 **`_attach`（首帧 / retry 重挂）** 与 `didUpdateWidget` 时应用 ⇒ **设计形态：状态住 widget · 传输是每桥的队列 · 重挂时重放当前状态**
- ⇒ ⭐ **而「重放一切」成立的前提被成对写出**：`:214`「桥会把它们**按序 flush**；渲染面**只认最后一个**」⇒ **过期中间态会被最后一个覆盖** ⇒ **所以 flush 全部是安全的** ⇒ **这两句必须成对，否则不成立**
- ⇒ 「**不要推断、去读真源**」家族又一实例（同 B0238 ack 驱动 · B0247 落盘以免「刷新后还在」变假）；另 `:346`「**ready 前入队，ready 后按序 flush**」两阶段被命名
- ⚠ **声明 ≠ 行为**：`:132` 是**声明**「重放」，**`_attach` 的执行体未读** ⇒ 下一批第一件事
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0290 · ✅ 「声明 vs 行为」结清（0 新发现）
- ⭐ `_attach`（`:330-353`）**确实**在每次挂桥时**全量重发五类值**（model/scale/dark/stageColor/actionScales）⇒ **`:132` 的声明成立** ⇒ **B0289 未核项关闭**
- ⭐ 旧桥**先 `removeListener` 再 `dispose`**（`:337-338`）· 旧的 render 订阅**显式 `cancel()`**（`:340`）⇒ **`:200` 的「随旧桥一起取消」在执行体里兑现了**
- ⭐⭐⭐ **`if (!mounted)` 时「把刚到手的 transport 还回去」**（`:331-334`）⇒ **「widget 已经没了」的回调里最容易漏的就是这个刚到手的资源**，而**它被显式 dispose 了** ⇒ **这不是「清空」或「忽略」，是「还回去」**
- ⇒ ⇒ ⭐ **这件事没有任何注释说明，只有代码做了** ⇒ **代码说出了注释没说的话**；`:123`「与 `stageColor` 同一条纪律」指的是 **B0120/B0237 那条「配置快照必须与实际一致、外部改动要能自愈」的家族**
- ⇒ ⭐⭐ **可提炼（B0287 与本批方向相反、同源）**：**处理资源的两条基本问句是「它现在还需要存在吗」与「谁还持有它」** ⇒ **大多数泄漏与误清都源于只问了其中一条**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0291 · ⭐⭐ 「复用令牌」的理由链 + 项目级禁令（0 新发现）
- ⭐ 复制取 **`widget.text` 原文**（非渲染 span）⇒ **B0261 声明成立**；**两次 await 两次 `if (!mounted)`**（`:553`/`:559`）⇒ **B0230 纪律第七、八处**
- ⇒ ⭐⭐⭐ `:556-559` 解释**为什么复用 `AppRhythms.interruptedHold` 而不是 `AppDurations`**：「那 4 档的语义是**一次过渡有多快**」vs「它是**瞬时状态保持多久**的那一档」+「**再立一个同值令牌是本项目明确不要的**（**见该令牌的注释**）」⇒ ⇒ **不是「没找到合适的」，是「项目明令不许」** ⇒ **第四次「指路而非复述」**
- ⇒ ⭐ **可提炼**：**令牌按「它度量什么」分类，而不是按「谁用它」分类** ⇒ **已有的档位才能被复用** ⇒ **复用的前提是分类对**；`:556-557` 正是**写出这个分类标准**的地方
- ⚠ **依赖未核**：结论依赖「那 4 档 = 过渡时长」这句**注释的正确性** ⇒ **B0292 第一件事**（读 `AppRhythms`/`AppDurations` 定义与其注释）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0292 · ✅ B0291 的「依赖未核」链闭合（0 新发现）
- ⭐ `AppDurations`（`tokens.dart:874+`）四档**全是过渡时长**：`fast`120 hover/pressed · `base`200 内容切换 · `slow`320 模态 · `reveal`600「**仅此一处**长动画：启动揭示」+ `registry`「**只服务测试**」⇒ ⇒ **没有一档是「瞬时状态保持多久」** ⇒ ⇒ **B0291 的推理成立**
- ⇒ ⭐ `registry`（只服务测试）⇒ **token 可机器枚举** ⇒ **B0204 `toValuesMap()` 形状的第二个家**
- ⇒ ⭐⭐ `interruptedHold` 的注释：「「**瞬时状态**」的保持时长：`已打断`、`已复制` 这类**就地确认**共用一档」+「**刻意只有一档**（**2026-09-11，P2-3**）：……两个**同值**令牌只会制造『**改一处忘一处**』」⇒ ⇒ **B0291 引的「见该令牌的注释」指的就是这里** ⇒ **链闭合**、理由是**带日期的变更记录**（B0228 族）
- ⇒ ⭐ `tokens.dart:1-5`「**一律引用这里的令牌**由 `test/design_tokens_lint_test.dart` 扫描守住」⇒ ⇒ **不是靠自觉** ⇒ 同 **B0204 P24**（让违规**必然失败**）
- ⇒ ⭐ `:18-21` **行数豁免理由可核**：「拆成四个文件会让『四套必须同步改』**从一次编辑变成四处编辑** —— 而**漏改一处正是本项目 P4「静默失效」要治的病**」⇒ **结构决策由「失败模式」论证，不由品味论证**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0293 · ⭐⭐「不假装」家族第七例，方向相反（0 新发现）
- ⭐⭐ `settings_models.dart:780-784` `note` 的规则：「**成功但有话要说**……**只有真有解释价值时才非空** —— 服务端**刻意不在成功路径上默认塞一句**，否则**每次自检都多一句噪音，用户很快就不看它了**」⇒ ⇒ **这是「不假装」的反向形态**：前面是「别假装成功/可用/能保存」，**这一条是「别假装有话说」**
- ⇒ ⭐⭐ **提炼出一条新区分**：**「真不真」的规则**（前六例）与**「何时才有价值」的规则**（本例）**需要不同的判据** —— 前者问「**这句话成立吗**」（用**类型/证据**判：B0232 ack 驱动 · B0247 落盘）；**本者问「这句话值得说吗」**（用**注意力**判）⇒ **同一个词在两个方向上都成立，而它们不能共用一条判据**
- 另：① Dart 类型是服务端 `TestOutcome` 的**逐字镜像**（六字段全对）**且点名了那个文件** ⇒ **第五次「指路而非复述」** ② `modelEcho` 的用途写明：「**确认『打到的确实是配置里那个』**」⇒ **回显不是装饰，是证据** ③ **`note` 与 `errorMessage` 是两个字段** ⇒ **「成功但有话要说」与「失败有话说」不共用通道**（B0284 家族又一实例）
- ⏰ 下一批第一件事：**自检的失败分类在前端怎么显示**（尤其 AGENTS 那条「**404 → 算通过**」的裁决）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0294 · ⭐ **F-0294-01（P3）** · 有意提供的解释掉在半路
- ⭐ **F-0294-01（P3）** `main.dart:1046-1050`（与 `:1065` 同构）成功分支 = `'ok · N ms · 模型：X'` ⇒ **`o.note` 未被读** ⇒ ⇒ AGENTS 裁决里的「**+ 一句 `note` 说明**」与 B0293 的字段规则「**只有真有解释价值时才非空**」**在 UI 上无处落地**
- ⇒ ⇒ **用户看不到「为什么这次通过而上次失败」**；⚠ **但这不是「假装成功」**（`ok` 来自服务端、失败分支三层兜底完整）⇒ **是「有意提供的解释掉在半路」** ⇒ **定 P3**（无功能影响；缺的是**解释**不是**判定**）
- ⭐ 三处**做对了**：① `modelEcho` 真被呈现 ⇒ **回显是证据不是装饰** ② 失败三层兜底 `errorMessage ?? errorCode ?? '未知原因'` ⇒ **顺序本身是设计**（先人话后码）③ `ApiException` 单独一支 ⇒ 「**没到服务端**」与「到了但判失败」是两回事（B0284 家族）
- ⇒ 两次 await 后都有 `if (!mounted || **epoch != _resultEpoch**) return;` ⇒ ⭐ **`mounted` 管「控件还在不在」、`epoch` 管「这次结果是不是最新的」**（B0140 的 epoch 闩锁）
- ⇒ 建议：① 成功分支带上 note（一行）② 或**在 Dart 侧删掉该字段**、让「谁负责解释」只有一处（B0284 家族）；⚠ **更强说法（「全局无任何展示」）我未核** ⇒ **只声称「`:1046-1050` 不显示它」** ⇒ 接手者先跑 `grep -rn "\.note" shell/flutter/lib/`
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0295 · 🔻🔻 F-0294-01 被更正（更强说法被推翻）
- `grep -rn "\.note\b" shell/flutter/lib/` ⇒ `main.dart:1073`（**TTS 自检**）**把 `note` 显示了**：`? 'ok · N ms${o.note == null ? '' : ' · ${o.note}'}`
- ⇒ ⇒ **「全局无任何展示」被推翻** ⇒ ⭐⭐ **真正的缺口只在 LLM 自检 `:1046-1050`** ⇒ **F-0294-01 范围收窄到一处**
- ⇒ ⭐⭐⭐ **根因改写为「两个同族功能实现不一致」**：同一条规则（「成功但有话要说」）在 LLM/TTS 两个自检上写法不同 ⇒ **收敛代价 = 零**（两行拼接）⇒ **没有理由不一致** ⇒ ⇒ 与 **B0249**（重复**可接受**、因为收敛会**层间倒挂**）**正相反**
- ⇒ ⇒ ⭐ **判据补齐**：「有理由的重复」要问**收敛的代价**；**代价为零时，不一致本身就是缺陷**；**最佳修法就在同一文件三十行之下**（按 `:1073` 改 `:1046-1050`）
- ⚠ **同名不同型**：`app_shortcuts.dart` / `shortcut_help_dialog.dart` 的 `note` 是**快捷键说明**的字段 ⇒ **grep `\.note` 会同时命中两者** ⇒ 记在案上以免误判
- ⚠ **元收获**：我 B0294 写的「`testTts` 消费点**同构**」**是没核就写的推测**，而**推测恰好是错的** ⇒ ⇒ 它**差点**让这条被写成「两处都缺」从而**低估** ⇒ ⇒ **B0274 规则在此的代价是「结论错一半」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第十九次对账（CONSOLIDATION-20）· ⭐⭐ **发现缩小一半、根因变深**
- **7 条 P1 复核：7/7 仍成立**；【模式 Q + KNOWN-LOSSES】**✔ 通过**（含 B0263 那次**第四次拦截**）
- ⭐⭐ **本轮唯一的发现被它自己下一批更正**：B0294 记 F-0294-01（P3，`main.dart:1046-1050` 丢弃 `o.note`）并写「`testTts` 消费点**同构**」⇒ **B0295 证明那一句是推测、恰好是错的**（`:1073` TTS **显示了 `note`**）⇒ **更强说法被推翻** ⇒ **范围从两处收窄到一处**
- ⇒ ⇒ ⭐ **根因改写**：**不是「字段被丢」，而是「两个同族功能实现不一致，代价为零却没收敛」** ⇒ 与 **B0249**（重复**可接受**、因收敛会**层间倒挂**）**正相反** ⇒ ⭐ **判据补齐**：「有理由的重复」要问**收敛的代价**；**代价为零时，不一致本身就是缺陷**；**最佳修法在同一文件三十行之下**
- ⇒ ⭐⭐⭐ **本轮最可迁移的产出：注释的两个方向都不够用** —— **注释说的「是什么」要靠执行体**（B0289→B0290 兑现）· **注释说的「为什么」要靠它指向的定义**（B0291→B0292 兑现）⇒ **共同成因是同一个词：凭印象补全**
- ⭐ 正面样本（十九批最密集）：**密钥写入链**（跨 4 文件/2 语言，六步）· **「时机绑理由」**（清空是推论非规则）· **资源的两条问句**（需要吗/谁持有）· **「不假装 X」必须精确**（否则破坏向前兼容）· **「不假装」反向形态**（别假装有话说；真不真 vs 何时才有价值）· **失败归因到正确的那半** · **记录失败尝试最佳样本** · **代码说出注释没说的话** · **令牌按度量什么分类** · **不变式写在拆分现场** · **注释是声明的镜像** · **「拒绝未知」跨语言**
- ⇒ 第四种失效形态入册（**本轮新增**）：**凭印象补全**（把「同构」脑补出来）⇒ ⭐ **合成句更新**：**不要在原地下结论，也不要在原地下推测**
- 覆盖率（**机械**：316 条去重路径）：Rust `.rs` **81** · 前端 `.dart` **37/218**（最大面）· CI/脚本 4 ⇒ ⭐ **`l2d-wasm-demo` 余面已清**
- ⭐ 自评：**本轮唯一的新发现是 P3，而真正的产出是「我发现自己在上一批说的话里脑补了一个词」** ⇒ **那个词恰好是错的** ⇒ **审计里最贵的一类错误不是「没看」，是「以为自己看过」**

### BATCH-0297 · ⭐ 承诺成立；而我差点记成一条方向相反的错发现（0 新发现）
- ⭐ AGENTS rc.2 ③「**等渲染面 `loaded` 回执才说『已切换』」**在代码里**成立**：**声明** `stage.dart:159` → **调用** `:317` → ⭐ **消费 `main.dart:1217 onAck: (…) { … }`**
- ⇒ 桥层也写明「**必须等**」的后果：`bridge.dart:122`「`loaded`（换成了），要么回 `error`（没换成）」+ `:166`「**只发不等，界面就会在舞台还…**」⇒ ⇒ **成功与失败各有各的表示**（B0288 同族）
- ⇒ ⚠⚠ **而我差点记成一条方向相反的错发现**：第一次 grep（同词 `onAck`、**排除**了 `live2d_bridge.dart`）**只看到声明与调用、没看到消费** ⇒ 我据此准备写「`onAck` 无消费点 ⇒ AGENTS 那句是旧的」⇒ **第二次 grep（`onAck:`、含构造处）把它找出来了**
- ⇒ ⭐ **具体动作规则**：**从「声明/调用」转向「消费」时，要搜的是「谁传给它」那个模式**（`onAck:`）**而不是继续搜同一个词** ⇒ **同词搜索与「谁传给它」是两个模式**（B0274 规则第四次拦我，第二次专拦「无消费点」这一类）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0298 · 🔻 更正 B0297：认错了对象；两条通道
- `main.dart:1213-1217` → `onAck` 的用途是「**缩放百分比**……**首屏也要有值**：过去只在上一次放大/缩小时才读，于是初始状态一直显示『—』」⇒ ⇒ **它是 `stage-ack` 流（缩放读数）**、**不是**换模型 ⇒ **我 B0297 的「承诺成立」认错了对象**
- ⇒ ⭐ 而这个消费点**本身正是在修一个已记录的缺陷** ⇒ **同 AGENTS rc.5 ⑧ 与 B0238 核的那个** ⇒ **三处（声明 / 消费 / 变更历史）现在都核到了**
- ⇒ ⭐⭐ **两条通道必须分开**：**`stage-ack` 流**（`onAck`）= 渲染面**对已应用参数**的回执 ⇒ **UI 缩放读数**（`stage.dart:159`/`main.dart:1217`）· **桥的 flush 回执** = `loaded`（换成了）/ `error`（没换成）⇒ **换模型是否生效**（`bridge.dart:122`/`:166`）⇒⇒ **两者都成立，但 AGENTS rc.2 ③ 指的是后者**
- ⇒ ⇒ ⏰ **「等」的那一半仍未核**（`live2d_bridge` 余面）⇒ **B0299 第一件事**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0299 · ⭐⭐⭐ 纪律写进了签名（0 新发现）
- ⭐⭐⭐ `live2d_bridge.dart:161-170` `Future<bool> swapModel(String url, {timeout: 15s})` —— 「协议 v1 `sync` 换模型，并**等到渲染面回执**才返回（R2 规矩）」⇒ ⇒ **调用方拿不到「已发出但未确认」的结果** ⇒ ⇒ **「只发不等」在类型上就做不到**
- ⇒ ⇒ 与「恰一次」纪律（B0132/B0156 靠**断言**守住）同族，但**这是更强的一档：靠类型守住**；`:120-122` 说它与 `live2d_stage.dart` 的 **R2 是同一个道理**（**第六次「指路」**）
- ⇒ ⭐⭐ **`true` 要「对上号」**：「回了 `loaded` **且报的正是我们发出去的那个 url**」⇒ **迟到的、属于上一次切换的 `loaded` 不会被误当成这次成功**；**`false` 的三条原因并列**：超时 / 渲染面报错 / **回执 url 对不上**
- ⇒ ⭐⭐ **理由写到机制层**：「activate **只改后端 registry**，**真正换皮发生在这个 iframe 里**」+「**只发不等，界面就会在舞台还没换的时候说「已切换」**」+ **指名缺陷名「激活了但没换皮」**
- ⇒ 另 `_lastSentStageBg`（`:134-138`）**又一条「排过队 ≠ 已送达」**的记录：**点名具体症状**（「重挂 iframe 之后**永远是黑底**」）+ **把自己的历史发现按族引用**（F-0002-1）⇒ ⭐ **记过的事实至今可被查到**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0300 · ⭐ 怀疑被证伪 + 「重复可接受」判据的另一半（0 新发现）
- ⭐ `chat_session.dart:150-154` + `:366` → 存储层**确实有界**（`while (session.messages.length > kMaxMessagesPerSession)`）、且**与视图同名同值**，理由**点名不同步的症状**（「界面上能看到但重开就没了」）
- ⇒ ⭐ **总体积也是闭的**：`browser_io.dart:82`「`maxSessions` × `kMaxMessagesPerSession` **双重夹持**」+ `:147`「**即使每个都到上限也远在配额内**」⇒ ⇒ **最坏情况被算过**，不是「希望不会太大」
- ⇒ ⇒ **「三个长期累积」各用对的手段**：日志列表**虚拟化**（view 成本·B0227）· 观察者缓冲区**上界+上界可见**（内存成本·B0240）· 聊天历史**视图 500 + 数据 500**（两层都管）· **总体积双重夹持** ⇒ **0 findings**，**而本批的价值是把自己的怀疑证伪了**
- ⇒ ⭐⭐ **与 B0249 合起来得到新区分**：| **可以重复** | **不能少** | ⇒ **两处 `const` 声明**（收敛会**层间倒挂** ⇒ 重复更便宜）· **两条裁剪**（数据层与视图层**各管一类成本**）⇒⇒ **声明可以重复、执行不能重复** —— 这是「重复可接受」判据的**另一半**
- ⇒ ⇒ 而 `:151-153` **点名不同步的症状** ⇒ **B0270 那一课的反向应用**（B0270 是「文档叫你手写 ⇒ 重建不许丢」；这里是「文件是程序自己的 ⇒ 裁剪要护住不变量」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0301 · ⭐⭐⭐ 把假设变成可核事实（0 新发现）
- ⭐⭐⭐ `browser_io.dart:79-83`「**配额满不是假设**：localStorage 常见上限 **5 MB**，而 `ChatSessionStore` 用 `maxSessions` × `kMaxMessagesPerSession` **双重夹持**把体积压在配额内。即便如此，**写失败也只丢「这次之后的新记录」，不该影响正在进行的对话**」
- ⇒ ⇒ **未知 → 有数 → 结构兜住 → 残余失败被限定**（本审计见过最强的一组）；⭐ 与 **B0164 家族的反面应用**（B0164 = 保守启发式 + **声明误判代价**；本条 = **把不可验证的配额当成已知数，并用结构把它压下去**）
- ⇒ ⭐⭐ **失败后果被限定到明确范围**（「只丢『这次之后的新记录』，**不该影响正在进行的对话**」）⇒ 与 B0288「**失败归因到正确的那半**」同族、**且更具体**
- ⇒ `:365-369` 裁剪体 `removeAt(0)` ⇒ **与「超了丢最旧的」逐字一致** ⇒ **B0300 关闭**；失败原因也被穷举（「**无痕模式 / 配额满**都会抛」）
- ⚠ **未核（下一批第一件事）**：`catch (_)` **之后具体做什么** —— B0293 核过「**失败不能静默**」，而这里**明确写了「失败静默」** ⇒ **两者是否矛盾，未核**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0302 · ✅ 张力解开：正确的分层，不是矛盾（0 新发现）
- `browser_io.dart:91-92` `catch (_) { // 忽略：**存储不可用不影响本次会话**。 }` ⇒ ⇒ ⭐⭐ **catch 的理由自带边界**：「**存储不可用**」**vs**「**本次会话**」⇒ **两件事被明确切开**
- ⇒ ⇒ **与 B0293「失败不能静默」不矛盾**，因为**对象不同**：| 纪律 | 对象 | 该不该静默 | ⇒ **链路失败**（LLM/TTS/解码）**不该** · **持久化失败**（存档写不下）**该**（对话照常，只少一段存档）
- ⇒ ⇒ ⭐ **分层按「用户能不能继续」划**；且这是 **B0293 那条区分的同族**：**B0284「不假装 X」要精确到「不假装 X 而仍要做 Y」** · **本条「不静默」也要精确到「不静默哪种失败」** ⇒ ⇒ **同一个词在两个方向上都需要限定**；**只用了半句话把边界说完**
- ⇒ ✅ **B0301 未核项关闭**；⚠ **仍不断言**：「本次会话」的**确切含义**未核（这一轮对话？标签页会话？）⇒ **边界也许还要再切一次**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0303 · ✅ 「本次会话」的粒度是「每次保存」（0 新发现）
- `chat_controller._persist()`（`:610-613`）出现在 **8+ 个站点**（`:98/:161/:197/:500/:518/:528…`）⇒ **保存按状态变化发生、不是「每会话一次」**；且每次 `write(sessions)` **写整个 blob** ⇒ ⇒ **失败一次 = 到下一次成功为止的记录没落盘**
- ⇒ ⇒ ⭐ **这正是「只丢『这次之后的新记录』」那句** ⇒ ⇒ **注释精确，两半句互相印证**（`browser_io` 的「**存储不可用不影响本次会话**」= 内存里那场照常）⇒ **B0302 关闭**
- ⇒ ⇒ ⭐ **顺带闭环**：`:606-608` **紧接在「本轮失败」之后**调 `_persist()` ⇒ **失败的轮次也会落盘** ⇒ 与 **B0247 的「未收尾」落盘链**一致 ⇒ ⭐ **失败不是被吞掉的东西，它被写下来了**（与 B0288 同向）
- ⚠ **仍不做穷举**：`_persist()` 的**全部 8+ 站点**未逐一核 ⇒ 「按状态变化」的覆盖面**大体**成立
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0304 · ⭐ 切会话不是纯 UI 动作（0 新发现）
- ⭐⭐ `chat_controller.dart:523-531` → `:526` **`_abandonTurn()` 在切换之前** ⇒ **流式中的那一轮被放弃**、**文字不会落到新会话**
- ⇒ ⇒ ⭐ **接上 B0248 链**：`text_fallback` 是「**覆盖式**整段正文」+ `unfinished` 标记 ⇒⇒ **「放弃」有确切含义**：那一轮**带着「未收尾」的标记留在旧会话里**，**不是被静默搬走、也不是被静默删除** ⇒ 与 B0288「**失败不是被吞掉的东西，它被写下来了**」**同向**
- ⇒ 另：切到当前会话直接 `return`（重复点击无害）· `sessions.select` 返回 `false` 就停（**失败不半途生效**）
- ⇒ ⭐⭐ 而 `:533-537` 说明**为什么**：`/\_syncActiveSession`「只影响**不带会话 id 的注入路径**（external-input 弹幕 / voice-input 转写）：**它们接在当前这段对话上**」⇒ ⇒ **「切会话」同时改变「注入的归属」** ⇒ **B0221/B0231「按会话分桶不串味」链的前端一环**
- ⇒ ⇒ ⭐ 结论：**它在语义上不是纯 UI 动作，而两个副作用写在同一个连续块里、没有藏**（`:539`「失败不抛（见 `ApiClient.setActiveSession` 的说明）」= **第七次「指路」**）；⚠ **`_abandonTurn()` 本体未读** —— B0247/B0248 链上我一直没读的那个
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0305 · ⭐⭐⭐ 「合理的解释」也可能是伪造（0 新发现）
- ⭐⭐ `chat_controller:573-604` → `_abandonTurn`（用户自己切走/重发/stop）vs `_releaseTurn`（**外部原因**）⇒ ⇒ **按「谁造成的中断」分，不是按「要不要提示」分** ⇒ **判据在原因，不在动作**
- ⇒ ⭐⭐ 分开的理由**可观察**：「否则**表现与『切走后的』完全一样（都是没有输出）**，而**两者的原因完全不同**」；`_abandonTurn` 的四个调用点**全是用户动作** ⇒ 「**静默是对的**」有据
- ⇒ ⭐⭐⭐ **「已收到的文字保留」的理由是「那是**真实收到**的内容，不许丢」** ⇒ ⇒ **依据是「真实性」，不是「完整性」**
- ⇒ ⭐⭐⭐⭐ **「这里不能用『模型没有返回文字』那条 —— 这一轮是被中断/断连掉的，写成『模型没说话』就是**另一种伪造**」** ⇒ ⇒ ⭐⭐⭐ **「不假装」家族最深的一次**：**连「合理的解释」都可能是伪造**，判据是「**这个原因对不对得上实际发生的事**」⇒ ⇒ 与 B0293「**别假装有话说**」**合起来才完整**：**说了要对，说得要准**
- ⇒ 另：两种空态各走各的路（**系统行说清原因** vs **空气泡删掉、不产生提示**）⇒ B0284 家族；两条路径的归属**写在一处**（`_finishTurn` 的 `wordless` 分支）
- ⚠ **待核**：**`_releaseTurn` 的调用点未核** ⇒ **判据的另一半**（谁会「外部原因收口」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0306 · ✅ 判据的另一半：两处都排除了另一条路（0 新发现）
- **调用点一**（`:28-39` WS 通道掉了）：「**收口帧在断连期间不会重放，丢了就再也回不来**，**输入框会永久卡在「停止本轮」**而 `send()` 静默 return」⇒ **判据从机制推出**；谓词 `mustReleaseTurnOnWsLoss` 在**另一个文件**，注释**指过去**
- ⭐⭐ **调用点二**（`:276-285` 被别的客户端抢占）：「**用户自己按的停止已经在上面的 `stop()` 里先收口了**，走到这里说明这一轮是**外部**中断的」⇒ ⇒ **同一个动作两条路，各自的前置条件被排除** ⇒ **B0305 判据在调用点的落地**；且**两个 reason 各有自己的 code** ⇒ **两码分立 ⇒ 可搜**
- ⇒ ⇒ ⭐ **判据在两端都成立**：**定义处**按原因分（`_abandonTurn` vs `_releaseTurn`）· **调用处**把「另一条路已处理」写明
- ⇒ ⭐ 两个场景都写出「**不收口会怎样**」⇒ **同一句后果**（「输入框会永久卡在『停止本轮』」）**出现两次** ⇒ **不是重复论证，是同一后果在两条路上的重现**（与 B0284 **方向相反**：**这里重复的是后果，规则被就地排除了**）
- ⚠ **待核**：**那两句 message 文案准不准**（B0305 核了「要准」这条**规则**，**未核这两句准不准**）+ `mustReleaseTurnOnWsLoss` / `stop()` 本体未读
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0307 · ✅ 用规则审实例：三句全过（0 新发现）
- ⭐⭐ **用 B0305 的规则审那三个实例，全过**：「本轮回复未收到（**实时通道断开**）」✅ 说通道断了不是「模型没说话」· 「本轮已被中断（**新的代次开始**）」✅ 点机制不拿模型当借口 · 「本轮失败，但**服务端未给出错误详情**（可打开『设置 → 开发模式 → 诊断日志』…）」✅⭐ **最纯** —— **承认「服务端没告诉我为什么」**
- ⇒ ⭐⭐ **第三处的注释把规则逐字写在遵守它的常量上面**（`turn_liveness.dart:189-191`）：「**禁止「无字无错」**……**必须给一个能拿去搜索的码**，并指向诊断日志（**而不是假装知道原因**）」⇒ **B0305 的规则被同一个文件自己复述了一遍**
- ⇒ ⭐⭐ **第一处给出的是「同一原则的另一半」**（`:129-133`）：「**同一句话同时用于两处**（错误横幅 + 会话里的系统行）：**横幅是暂时的，而历史是长久的** —— 只写横幅的话，**用户刷新之后只会看到『一条没有回复的消息』，又回到了『与坏了无法区分』那个坑**」⇒ ⇒ **这是 B0247「『未收尾』必须落盘」那条要求，套在「错误行」上而不是「正文」上**
- ⇒ ⇒ 三个码**分立且都「能拿去搜索」**（`ws_dropped_mid_turn` / `turn_preempted` / `turn_failed_no_detail`）⇒ **可搜**（B0159 家族）；⇒ **本链第四次递进**：B0305 定义处 → B0306 调用处 → **B0307 用规则审实例，全过**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0308 · ⭐ 判据本身也守同一纪律（0 新发现）
- ⭐ `turn_liveness.dart:100-103` `mustReleaseTurnOnWsLoss` = **纯函数 + 具名 + 只有两项**（`turnInFlight && status.isProblem`）⇒ 无副作用、可单测、可复用（同 B0264 家族）
- ⇒ ⭐⭐ **两个 getter 都写明了「误判会让用户以为什么」**：`isUsable`「`connecting` **不算**可用 —— 把『正在连』当可用会让 UI 显示『已连接』**而消息其实还发不出去**（**用户看到的是「发出去没回应」**）」· `isProblem`「**用户应该看到问题**」（**不是「技术上坏没坏」**）⇒ ⇒ **他们不只定义状态，还定义了「错误解读的后果」**
- ⇒ ⇒ 且 **`connecting` 被显式排除**在「可用」之外 ⇒ **三个状态里没有灰区**；`isProblem` **两处使用**（`connection_badge:46` / `turn_liveness:103`）⇒ **判据共享、表现分离**（P2 形状）
- ⇒ ⚠ **一处未兑现的「指路」**：B0306 引「用户自己按的停止已经在上面的 **`stop()`** 里先收口了」，而 **`grep -n "void stop" chat_controller.dart` 零命中** ⇒ **可能改名了**（`main.dart` 有 `_stopWithCancellation`）⇒ 按 **B0271 的规矩不外推、不删除引用**，只记录「**未兑现**」
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0309 · ⭐ 指路兑现；判据的**前向应用**（0 新发现）
- ✅ **B0308 那条「未兑现」兑现了** —— `stop()` 在 `chat_controller.dart:214`
- ⇒ ⭐ `:207-213`「**先本地收口，再发请求（顺序有讲究）**」由**两个独立机制**支撑：**① 后端 abort（`RootEffect::TurnAborted`）不投影任何 WS 帧** ⇒ **这一轮的收口只能由客户端做**（⇒「先收口」**不是偏好、是唯一可行**；且**点名服务端那个文件** = 又一次「指路」）
- ⇒ ⭐⭐⭐ **② 顺序还避免一个「未来的误读」**：服务端收到 stop 会推进 epoch 并广播 `new_epoch`，而**本文件里 `new_epoch` 的语义是「上一轮被外部中断」** ⇒ **若不先收口，用户自己按的停止会被读成「外来抢占」**
- ⇒ ⇒⇒ ⭐ **这是 B0306 那条判据的「前向应用」**：B0306 核「`_releaseTurn` 怎么把**外部中断**认出来」；**本条说「怎么不去制造那个『外部』信号」** ⇒ **同一判据、两个方向都用上了**；失败两支都写 `_error` + `notifyListeners()` ⇒ **失败不被吞**
- ⇒ ⇒ ⭐ **B0305–B0309 五批五层首尾闭合**：`stop()` → `_finishTurn(stopped:true)` → abort 不投影帧 ⇒ 收口只能客户端做 → 先收口避免制造「外部」信号 → `new_epoch`/`_releaseTurn` 判外部 → **文案要说准**
- ⚠ **枢纽仍未读**：**`_finishTurn` 本体（`:429`）** —— `stopped: true` 从哪进、`wordless` 分支都在它里面 ⇒ **B0310 第一件事**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0310 · ⭐⭐⭐ 最好的一段「弯路记录」（0 新发现）
- ⭐ `chat_controller.dart:442` 音频收尾闸门 `if (!failed && !stopped) audio.flush();` 判据到机制到症状：「引擎允许某句 `final_chunk` **样本数为 0**，而 `build_audio_frames` 对空 PCM 返回空列表 ⇒ 那一句**永远等不到 `end`** ⇒『**话说完了但结尾没了**』」
- ⇒ ⭐ 而「**失败/停止的轮次不收尾**：那半截本来就没合成完，**播出来就是断句**……契约是『**宁可晚开口，不可中途断**』」（`:441`）⇒ ⇒⇒ **「一句一单元」在音频尾巴上的第五层**（提示 B0116 · 装配 B0132 · 上界 B0243 · 界面 B0261 · **音频尾 B0310**）
- ⇒ ⭐⭐⭐ `// ── 历史（**两天的弯路，别再走回去**）：` 记着**两次错法**：① 伪造 `bubble.text = '（本轮没有文字输出——通常是模型只调用了动作工具）'` ⇒「**不是模型的输出**，是**前端凭上下文猜的台词**，却长得和真回复一模一样 → 用户问『为何会有空提示词』」② `sessions.remove(bubble)` ⇒「对『整轮没返回文字』这个**正常完成**的回合变成**什么都不显示** → 用户报『**第一次对话之后没输出了**』（**2026-09-11 实测复现**：`点点头。` → 零文字、零音频）」③ 现在**换一条系统行**：「**事实照说，但一眼就能看出那不是角色说的话**」
- ⇒ ⇒⭐⭐ **两次失败都在、第二次带实测复现** ⇒ **不是「改好了」，是「试过两条路，都不行，理由如下」**；⭐ **连错误假设的退役都被记录**（LLM 已无工具 ⇒ 那句猜测「**连前提都不成立了**」）⇒ **同 F-0040-01 的正确处理**
- ⇒ ⭐⭐ `:468-469`「**用户主动停止也走这条路，但用另一句话 —— 别把用户的意志说成模型的产出**」⇒ ⇒ **B0305–B0309 那条判据的第六层**（用户的意志不得被叙述成模型产出）
- ⚠ **最后一层未读**：**`settleTurn` 本体**（`TurnSettlement` 四个分支各自的判据）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0311 · ✅ 最后一层闭了（0 新发现）
- ⭐ `settleTurn`：「**『只有思考』优先于『什么都没有』**：气泡里有**真内容**，不能当空气泡丢掉」⇒ **判据是「真内容」** ⇒ **同一条判据的第七层**
- ⇒ ⭐⭐⭐ 而 `mustReleaseTurnOnWsLoss` **为什么不用 `!isUsable`** 写了**机制**：「`connecting` 也不可用，但它是『正在建连』—— **发送瞬间通道可能还在连**（`send()` 里刚调过 `ensureConnected`），**那时把这一轮收掉是错的**（**回复随后就到**）」⇒ ⇒ **比 B0308 精细**（那次只核到「被排除」，**这次拿到机制后果**）
- ⇒ ⇒⇒ ⭐⭐⭐ **而这与 B0309 的 `stop()` 是同一陷阱的两次出现**：**`stop()` 先收口** ⇒ 防**制造 `new_epoch`（外部信号）**；**`mustReleaseTurnOnWsLoss` 不用 `!isUsable`** ⇒ 防**在 `connecting` 时误收一轮还在飞的** ⇒ **两处都在防「把一个还没结束的状态当成结束了」**（**一次在排序、一次在判据**）
- ⇒ ⭐⭐ `kWordlessTurnNotice` 的理由给出**分工**：「**只说『本轮没有返回文字』，原因留给诊断日志**」⇒ ⇒ **这是红线「拿界面上的码去日志里搜」的**对偶**：**界面给事实 · 日志给原因 · 两边用同一个码**
- ⇒ ⇒ ⭐ **B0305–B0311 七批七层闭合**（定义分 · 调用排除 · 文案说准 · 判据纯函数 · 排序避免制造信号 · 收口不冒充模型 · **优先级按真内容**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### 第二十次对账（CONSOLIDATION-21）· ⭐⭐ **一条判据在七个位置上被引用**
- **7 条 P1 复核：7/7 仍成立**；【模式 Q + KNOWN-LOSSES】**✔ 通过**；有效 **78** 条 · 撤回 4 · **P0 = 0**（二十次）
- ⭐⭐⭐ **本轮中心产出：判据「谁造成了这一轮」的七层追踪** —— ① **定义分**（两个函数·B0305）② **调用排除**（`selectSession`·B0304）③ **文案说准**（三个码三句话·B0307）④ **判据纯函数**（`turnInFlight && status.isProblem`·B0308）⑤ **排序避免制造信号**（`stop()` 先收口·B0309）⑥ **收口不冒充模型**（系统行·B0310）⑦ **优先级按真内容**（`settleTurn`·B0311）
- ⇒ ⇒ ⭐⭐⭐ **层与层互相引用**：⑤ 的理由是「**若不先收口，用户自己按的停止会被当成外来抢占**」⇒ **那是 ③ 的判据被自己触发**；④ 不用 `!isUsable` 的理由（**发送瞬间通道可能还在连**）⇒ **与 ⑤ 是同一陷阱的两次落地**
- ⇒ ⇒ ⭐⭐⭐ **「模型」不是某一段文档，而是那些被反复引用的判据；审计读到第七次引用时，才算真正看见了它**
- ⭐ **F-0294-01** 及其更正：范围**从两处收窄到一处**（`:1073` TTS 确实显示 `note`）⇒ **根因改写为「两个同族功能实现不一致，代价为零却没收敛」** ⇒ 与 B0249 **正相反** ⇒ ⭐ **判据补齐：代价为零时，不一致本身就是缺陷**
- ⭐⭐ **四条判据精化**：① **注释两个方向都不够用**（是什么→执行体 · 为什么→定义）② **转向「消费」要搜「谁传给它」**（`onAck:` 非 `onAck`）③ **「重复可接受」有另一半：声明可重复、执行不能重复** ④ **代价为零时不一致即缺陷**
- ⇒ **五种失效形态**全部入册：凭记忆 · 凭零命中 · 凭一次 grep · 凭印象补全 · ⭐ **凭印象怀疑** ⇒ ⭐ **合成句：不要在原地下结论，也不要在原地下推测，也不要在原地生怀疑**
- 覆盖率（**机械**：319 条去重路径）：Rust `.rs` **81** · 前端 `.dart` **37/218**（最大面）· CI/脚本 4 ⇒ ⚠ **前端仍 181 未读** · Mod crates 42 未读
- ⭐ 自评：**本轮唯一的新发现是 P3，而真正的产出是「我跟着一条判据走了七层，而它前六层我都已经读过了」** ⇒ **审计的深度不来自读了多少行，来自「对一个已注意到的东西，是否肯再问六次它还在不在别处」**

### 第二十一次对账（CONSOLIDATION-22）· ⭐⭐ **清账这个轴，比读新文件更有产出**
- **7 条 P1 复核：7/7 仍成立**；【模式 Q + KNOWN-LOSSES】**✔ 通过**；有效 **78** · 撤回 4 · **P0 = 0**（二十一次）
- ⭐⭐ **清账 3 批成绩单**：**结清 8 条** · **收窄 1 条**（`api_client._guard()`「无客户端超时」⇒ **自检路径上是刻意委托给服务端** ⇒ **从疑点降为「已定分工」**）· **判据发现 2 条** · ⇒ **未核实项 18 → 7**
- ⇒ ⇒⭐⭐ **清账为什么比读新文件更有产出**（本轮实测三条）：① **未核实项是路标，而路标指向未读之处**（两条「只是来取个值」都读出没预期的东西：`kReasoningOnlyCaption` 给出**可执行的出路** · `_trimSessions` 记着**一次「兜底反被咬」的教训**）② **收窄长期疑点成本极低**（B0229 挂的疑点**一行 `{'timeout_ms': ?timeoutMs}` 就结案**）③ ⭐ **它同时检验「我记下的东西是否仍成立」**（`ActionCue` 不在我以为的文件里 ⇒ 按 B0274 **不据此推断**）
- ⇒ ⭐⭐ 本批两条正面样本：**「吞 vs 丢」**（`setActiveSession`「失败不抛……**真正的失败由下一次发送的 4xx/5xx 暴露**」⇒ **推迟到真正会疼的地方，而非消失**·第五层）· **「兜底反被咬」**（`firstWhere(..., orElse:)` 会回落到**当前那个** ⇒ **兜底反而把要保的删了** ⇒ **宁可超上限也不能删用户正在看的** ⇒ **第三次「弯路记录」**）
- ⇒ ⚠ **本轮最重要的产出在账本上**：⭐ **查出 `STATE.md` 头部长期停在 `BATCH-0038`** ⇒ 我最近若干次 `in_progress` 更新**锚点未命中、`replace()` 静默无效而我没检查** ⇒ ⇒ **同一条纪律第三形态：「我以为我写了」⇒ 写入后必须回读确认锚点命中**（**已修**，本批起每批回读）
- ⇒ ⚠ **另一起工具层事故**：用 shell heredoc 写账本而正文出现**与 shell 分隔符同形**的行 ⇒ **shell 把后续 python 与 echo 吞进文件**（103 行垃圾）⇒ 已用 `write` 重写 ⇒ ⇒ ⭐ **长文本一律用 `write` 工具** + **`write` 硬要求先读**
- ⇒ ⚠⭐ **KL-4「永久不可核的 6 条」单列，并当场回正两处误列**（`secrets.rs` 断言体 / B0120 chmod 别处**其实可核、只是欠账**）⇒ ⇒ **「不可核」这个标签本身也会出错**
- 覆盖率（**机械**：319 条去重路径）：Rust `.rs` **81** · 前端 `.dart` **37/218**（最大面）· CI/脚本 4
- ⭐ 自评：**本轮我把 11 条欠账变成 6 条永久欠账（不可核的）、8 条结清成可核结论**；而**剩下那些我要单列一节、标注「不可核」及原因 —— 让读者知道哪些是我选择不查的、而不是漏了没查**

### 第二十二次对账（CONSOLIDATION-23）· ⭐ **43 批只产出 2 条 P3，而模式 P25→P31**
- **7 条 P1 复核：7/7 仍成立**；【模式 Q + KNOWN-LOSSES】**✔ 通过**；有效 **78**（P1 7 / P2 35 / P3 36）· 撤回 4 · **P0 = 0**（二十二次）
- ⭐⭐⭐ **一条判据的八层追踪**（B0304–B0313）：「谁造成了这一轮」在**定义分 / 调用排除 / 文案说准 / 判据纯函数 / 排序避免制造信号 / 收口不冒充模型 / 优先级按真内容 / 枚举级**八处**各长成一种形态**⇒ **我先看到「两个收口函数」，七批之后才发现它贯穿八层**
- ⇒ ⭐⭐ **换轴清账**（B0313–B0315）：**结清 8 · 收窄 1 · 误列回正 2** ⇒ **未核实项 18→7**、**永久不可核 6→4**；且 **「不可核」这个标签本身也会出错**，而 CONSOLIDATION-22 随手写下的半句**被真的执行了**（B0319）
- ⇒ ⭐⭐ **五条判据精化** + **六种失效形态**全部入册（凭记忆/零命中/一次 grep/印象补全/印象怀疑）⇒ **合成句：不要在原地下结论，也不要下推测，也不要生怀疑**；⚠ **而「换个人问」被应用四次**（B0316/0319/0345/0352）
- ⇒ ⭐ **四条新模式**：**P26-b 加限定**（同形本身不是问题）· **P29**（假参数要删不删）· **P30**（删通道前列清单）· **P31**（错误提示指向用户能操作的东西）
- ⇒ ⭐⭐ **一个此前没单列的类别**（B0348）：**把「我们没修」与「为什么没修、怎么修」写进任何读代码的人都会看到的地方** ⇒ **它比「修掉」更稀缺**
- 覆盖率（**机械**：319 条去重路径）：Rust `.rs` **81** · 前端 `.dart` **37/218**（最大面）· CI/脚本 4
- ⭐ 自评：**审计的价值在「八层判据 + 六种失效形态 + 六条模式 + 欠账 18→7」上，不在两条 P3 上**

### BATCH-0368 · ✅ **接收端找到了，且把 F-0046-01 想要的全做了**
- ⭐⭐⭐ **接收端在 `shell/flutter/lib/live2d/live2d_host_web.dart`**（Flutter 宿主）—— **不是 wasm crate、也不是 `l2d/src`**（B0367 已证 + 本批零命中）
- ⇒ ⭐⭐⭐ **而它把该条想要的东西全做了**：`if (event.origin != web.window.location.origin) return;` + `if (source != _iframe.contentWindow) return;` ⇒ **双重身份校验**（**同源 + 同一窗口**）⇒ **比该条建议的还多一项**；且**规则先写在头注**（`:40-42`）
- ⇒ ⇒⇒ **「任何页面都能往这个 iframe 发消息」在当前树上不成立** ⇒ **该条不重开**（B0275 的撤回**结论正确**，**理由需补这一句**）
- ⇒ ⭐ **B0367/B0368 两批的实际产出**：**把一个我一度想「重开」的 P1，用排除清单定位到它真正的家，并确认它已按最严的方式实现**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0374 · ⭐⭐⭐ **同一判据，路由层对、断言层漏**
- ⭐⭐⭐ `models_routes/handlers.rs:138-166` `handle_import` 是**五道关卡 / 四个码**：`invalid_payload`（JSON 解析）· `invalid_resource_path`（id 非法 · 路径解析失败 · 目录不存在）· **`invalid_model3_json`（`find_model3_json` 返回 None）** + 文案「目录 X 下找不到 `*.model3.json`」⇒ ⇒ **用户会看到**
- ⇒ ⇒ 而 `model_root.rs:111` 的 assert 在 `bai` **不存在**时**空洞通过** ⇒ ⇒ **同一个判据，路由层做对了、断言层做漏了**
- ⇒ ⇒⇒ ⭐⭐ **我第一次能给出一条 P1 的「对/错两处对照」**（此前只记了「错的那处」）⇒ ⇒ **结论不变、证据变强**
- ⇒ ⇒⇒⭐ **修法收窄到一行级**：**路由层已经对**，只需把 `model_root.rs:111` 那条 assert **换成同样的判定、或直接删掉**（职责已被路由层承担）⇒ **不是设计改动，是一行改动**
- ⇒ 另：`:11` 头注记「**写盘失败回滚内存状态**」⇒ 而 **Dart 侧完全看不到这一层**（B0373）⇒ **「换个文件问」第四次印证**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0375 · 🔻🔻 **F-0001-02 由 P1 降为 P3** · 我记错了主体
- ⭐⭐⭐ `model_root.rs:106-120` 那个 assert **在 `#[test] fn model_root_walks_up_to_the_repo_root()` 里** ⇒ **它是测试断言、不是生产校验**
- ⇒ **B0374 已核**：生产路径 `models_routes::handle_import` 用 `find_model3_json` **做了正确的四段判定**（`invalid_model3_json` + 文案）⇒ ⇒ **生产侧根本没有这个判据**
- ⇒ ⇒⇒ **我记的「整个判据形同虚设、导入可能静默通过」** —— **生产侧不存在这条路径**
- ⇒ ⇒ **实际影响 = 这条测试失去了它要证明的能力**（`model_root()` 是否找对了根），**不是**「用户会导入失败」⇒ ⇒ **降 P3**；**修法变简单**：去掉那个 `||`（让 `bai` 不存在时**测试失败并暴露真相**），或该测试随 `bai` 一并删除
- ⇒ ⇒ ⭐⭐⭐ **可提炼**：**在测试里发现的空洞，默认要问「它在守什么」** —— 若它守的判据**在别处已被正确实现**，**那是测试的缺陷、不是系统的缺陷** ⇒⇒ **级别要按「谁受害」定，不是按「代码多丑」定**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0376 · ✅ **F-0001-02 这条线彻底闭合**
- ⭐ 其余 `bai` 引用**都不是**「仓库自带 bai」的依赖：`adapter/mod.rs:285/317` 是**测试名**；`cli/tests.rs:294-296` **自己造 `bai` 目录 + 假 manifest**
- ⇒ ⭐⭐ 而缺省路径解析**三件事都做到**（`path_resolve.rs:34-53`）：**两个候选**（cwd · manifest 编译期目录）· **失败时列出试过的每一条** · ⭐ **给出可执行的下一步**「请用 `--model-smoke <model3.json>` 显式指定路径」
- ⇒ ⇒⇒ **这是 B0352 那条 P31 的第四处应用**（前三次都在 **UI 侧**·**这次在 CLI 侧**）⇒ 同 B0197/B0316「把机器说清」
- ⇒ ⇒⭐⭐⭐ **所以这条线上三处**：路由层 `handle_import` **对**（B0374）· CLI 缺省解析 **对**（本批）· **`model_root.rs:111` 的 `#[test]` 是唯一缺口**（P3）
- ⇒ ⇒⇒ **而 `cli/tests.rs` 那个测试自己造 `bai`，正是 `model_root_walks_up_to_the_repo_root()` 应该做的事** ⇒⇒⇒ **B0375 的降级完全站得住**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0377 · ⭐ **那条 `||` 的来由被完全解释**
- ⭐⭐⭐ `model_root()`（`:46-57`）**从不需要 `bai` 存在就能返回一个根**（第二级判据是「**同时含 `Cargo.toml` 与 `assets/`**」，`:45` 逐字「**避免在 `crates/*/Cargo.toml` 就停下**」）⇒ **而它是对的**
- ⇒ ⇒⇒ **那条 `#[test]` 断言的其实是「这个仓库恰好带了 bai」这条具体事实**；而 `:24` 逐字：**`assets/models/*` 被 `.gitignore` 排除**（仅 `README.md`/`.gitkeep`）⇒⇒⇒ **它断言的是一个被 gitignore 排除掉的事实**
- ⇒ ⇒⇒⭐⭐ **而 `|| !root.join("bai").is_dir()` 正是为了让这个断言在「仓库不带 bai」时不失败而加的** ⇒⇒⇒ **它把「断言的根据不成立」变成了「断言恒真」** ⇒⇒ **B0375 降级判定的最后一层证据到手**
- ⭐ `:57` 兜底是「**返回那个未命中的 direct**」⇒ **不返回 `None`、不 panic** ⇒ **「找不到」也是一种确定的值**（同 B0308「空态被定义」族）
- ⭐ `builtin_model_id()`（`:70-76`）**又是 P31**：「失败 = **空串**：**宁可状态栏空着，也不要报一个猜出**…」⇒ **P31 第五处**（UI 侧四处 · **CLI/Rust 侧第一处**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0378 · ⭐⭐ **等回执才说真话**
- ⭐⭐⭐ `live2d_stage.dart:462-469`「渲染面自己算缩放（±10%，clamp 0.5..2.0），所以**不发数值**；应用后的真值从 `lastAck` 取」⇒⇒ **宿主没有任何地方能算出「缩放后的值」**⇒⇒⇒ **真值只有渲染面一个来源**
- ⇒⇒ **同 B0299 核的 `swapModel` 一族**（`Future<bool>` 等 `loaded` 回执）⇒ 而这里**更进一步**：**协议层就没有本地值**
- ⭐⭐ **clamp 写在头注**（协议文档上、不在代码里）⇒⇒ **宿主不需要知道边界** ⇒ 同 B0364「边界 = 声明的边界」的**渲染面侧**应用
- ⭐⭐ **B0000 ⑧ 那个缺陷（「缩放读数永远是 `—`」）的修法在这里可见**：「**读回执**」⇒ 与第 1 点同源
- ⭐ `swapModel` 头注仍带「调用方拿 `false` 时应**如实说『已登记，但舞台未确认』**」⇒ 又一个 **P31**
- ⇒ ⇒⭐ **本批把三条线接成一条**：**「不自己算」是让「真值只有一个来源」的结构前提** ⇒ B0299 · B0378 · **B0000 ⑧ 的修法**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0379 · ✅ **B0000 ⑧ 的修法有回归**
- ⭐⭐⭐ `test/live2d_bridge_test.dart:119` **group 名就是那条规矩**：「**stage-ack 解析（收条前不得提示已生效）**」⇒⇒ **测试名 = 被守的纪律** ⇒⇒ **同 B0313 核的 B0309 `stop()` 排序注释同款：纪律写在「防它不守」的那一处**
- ⭐⭐ **测试用「真实形状」**（`:127` 逐字「**真实形状：没有 version，字段在根上**」）⇒⇒ **不是造一个比真实更规整的形状** ⇒⇒ **B0274 规则的一次遵守**
- ⭐⭐ **第二条钉住「拒绝」**（`applied=false` 也要能读出来）⇒⇒ **同 B0334/B0335 —— **反向分支也是分支**
- ⭐ 第三条钉住「**不抛**」（缺字段 ⇒ `null`）⇒ 同 B0322「坏 dataURL 永不抛」族 · `:51` 另有「**容忍无 version 的遗留 stage-ack 与未知 type**」⇒ **向后兼容被测**
- ⇒ ⇒⭐ **B0378 的判断被完整兑现**：**「不自己算」⇒「真值只有一个来源」⇒「有读回执的回归」⇒「回归名就是纪律」** ⇒ **四层齐全**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0380 · ✅ **B0000 ⑦ 的修法有回归** · 而头注写着「被守的缺陷」+ 日期
- ⭐⭐⭐ `test/asset_guard_stage_attach_resend_test.dart:3`「# **被守的缺陷**（rc.4 实测修复，**2026-09-14**；**重放最易覆盖掉的就是这几行**）」⇒⇒ **纪律写在「防它不守」的那一处**（同 B0313 核的 `stop()` 排序注释）⇒⇒ **还带日期 + 症状**
- ⇒ ⇒⇒⭐⭐ **「重放最易覆盖掉的就是这几行」是一句我此前没见过的担心** ⇒⇒ **它防的不是「改错」，是「重做时不小心省掉」**
- ⭐⭐⭐ `:10-14`「# **为什么需要「新的」断言（已存在 vs 新增）**」+「`live2d_bridge_test.dart:92` 已有行为断言，但它测的是**桥有没有照发**，它**测不到**……」⇒⇒ **存在断言 ≠ 覆盖到位** ⇒⇒ **说清了缺的是哪一层**
- ⭐ 断言是**结构扫描**（`RegExp(r'stageColor\s*:\s*widget\.stage…')`）⇒ 守的是「重发时用 `widget.stage…` 而不是缓存」⇒ 同 B0315 核的 `chat_notice_test.dart:150-167`（结构测试钉「不许绕过判据自己判」）
- ⇒ ⇒⭐ **而本批最值钱的是那句「重放最易覆盖掉」** ⇒ **「修法有回归」与「重放会省掉」是两种不同的防**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0381 · ⭐⭐⭐ **三层自证在一个门禁里**
- ⭐⭐⭐ `test/visual_language_test.dart:341`「**扫描器能抓到违规（合成样例自证）**」⇒ 内嵌**合成坏样例** ⇒⇒ **先证明它会红**
- ⭐⭐ `:362` **反向对照**「被注释掉的代码里带 `**` **不该**被扫出来」⇒⇒ **它不是简单 grep**
- ⭐⭐ `:366`「**lib/ 里没有违规**」+ 注释「**渲染器自己当然有 `**`**」⇒⇒ **扫描器自身被排除、且排除理由写明**
- ⇒ ⇒⭐⭐⭐ **三层自证**：**① 合成坏样例**（不是空转的）· **② 反向对照**（有上下文判断）· **③ 排除规则 + 理由**（知道自己会撞到自己）
- ⇒ ⇒ **这正是我 P1 判据里「test that structurally cannot fail」被认真对待的样子** ⇒ **B0344 字体门禁**以另一形式同性质
- ⇒ ⇒⇒ ⭐ **一句关于这个仓库的实话**：**少数门禁到了「先证明自己会红」这一层，多数没到** ⇒ **不构成缺陷，只作观察**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0382 · ⭐⭐ **入口有两个、闸门只有一个**
- ⭐⭐⭐ `voice_listen_controller.dart:215`「**同一道预检：PTT 也走 voice-input 端点（手动闸 / 总闸不变）**」+ `_voiceModBlockReason()` ⇒⇒ `pressStart` 与 `toggle` 走**同一个 Mod 闸门**⇒⇒⇒ **同 B0361 persona「绝不偷偷改成全局导入」族：两道门不因为入口不同而不同**
- ⭐⭐ **「支不支持」是真值**（`supported => _recognizer != null`）⇒⇒ **B0342 核的 `_enabled` 用的就是它** ⇒⇒ **门面禁用与服务端能力同一个源**
- ⭐⭐ 不支持时的文案**点名了需要什么**：「**这个构建没有语音识别（需要桌面版 Chrome / Edge）**」⇒ **P31**，且**点名了具体的浏览器**
- ⭐ `pressStart`/`pressRelease` **各有各的守卫**（`_ptt` 正反两面）⇒ **B0342 核过 UI 侧成对，现在控制器侧也成对** · `:217` **await 后查 mounted**
- ⭐ `suspendedForPlayback`（P0-4）⇒ 又一个「**被谁暂停了**」的状态可见 ⇒ 同 B0313「谁造成了这一轮」
- ⇒ ⇒⭐ **而这件事被写在两个入口之一���注释里** ⇒ **读者在 `pressStart` 这条路上就知道 `toggle` 那条也是同一道门**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0383 · ⭐ **一个不对称，被文案中和了**
- ⭐⭐ `voice_listen_controller.dart:311/313` **「空」与「读失败」落到同一个值（缺省）** ⇒⇒ **不可区分** ⇒⚠ **而 `_loadWakePhrase` 的读源我未核 ⇒ 按 B0274 不推断**
- ⭐⭐ `:347` **匹配用的是重载后的值**（不是初始化时抄的）⇒⇒ **改完下一轮就生效**
- ⭐⭐⭐ `:151` **对外文案把当前唤醒词嵌进去了**：「**在听：说「$_wakePhrase ……」**」⇒⇒ **P31 家族**（把内部状态说出来）⇒⇒⇒ **而这把第 1 点从「问题」降为「可观察的取舍」—— 静默回落会立刻被用户看见**
- ⇒ ⇒⭐ **本批最值钱的是第 3 点**：**一个内部的回落之所以不是问题，是因为另有一处把它说出来了** ⇒⇒ **P31 的第二面：正因为说出来了，那个不对称才可以存在**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0384 · ⭐⭐⭐ **「空 vs 读失败」是语义上同一个值**
- ⭐⭐⭐ `lib/main.dart:982-988` **唤醒词是 `voice-input` Mod 的 `config['wake_phrase']`**，**不是 `DisplayPrefs`** ⇒⇒⇒ **B0383 那条「空 vs 读失败」是**同一个有意的形状**：**「Mod 没启用」与「它的配置里没这个词」都意味着「没有自定义唤醒词」** ⇒⇒ **一个值足矣**
- ⭐⭐ `_loadWakePhrase` 是**注入的**（`voice_listen_controller.dart:78`）⇒ **控制器不 import API** ⇒ **可单测** ⇒ 同 B0361 族
- ⭐⭐ **而 `_loadModEnabled` 也在同一处注入**（`:79`）⇒⇒ **B0382 那道「同一道预检」用的就是它** ⇒⇒ **闸门与取值是同一家的**
- ⭐⭐ 读的是 `m.config`（B0355：带 `config`、**secret 脱敏**）⇒ **`wake_phrase` 不是 secret** ⇒ **可见性是契约决定的** ⇒ 又一个「契约决定呈现」（B0358 族）
- ⇒ ⇒⭐ **上一个批我记的是「一个不对称」；这一批知道它是语义上就应该是同一个值** ⇒⇒ **B0383 看起来不对称，是因为我只看到「空 vs 抛异常」，没看到「这两个来自不同的世界」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0385 · ⭐⭐⭐ **同一句话写在两侧**
- ⚠ 开头 `grep wake_phrase crates/live2d-ai-mod-external-input/` **零命中** ⇒ **路径错、不是事实** ⇒ **「换个文件问」第五次**（真身在 **`live2d-ai-mod-voice-input`**）
- ⭐⭐⭐ `voice_routes.rs:52-56` **四个闸门、顺序「钉死在 handler 里」**：`voice_manual_off` · **`voice_gate_closed`（总闸=唤醒短语，**显式空**）** · `wake_phrase_required`（不以它开头）· 命中则**剥掉**
- ⇒ ⇒⇒⭐ **而第 2 条写着「**键缺失**不在此列 —— 走产品缺省『小可爱』」** ⇒⇒⇒ **这解释了 B0383 的「不对称」：服务端区分「显式空」与「键缺失」，客户端把两者都收敛成「缺省那个词」⇒ **有意的**（客户端要可读的词 · 服务端要闸门状态）
- ⭐⭐⭐ **同一句话写在两侧**：B0382 客户端「**同一道预检：PTT 也走 voice-input 端点（手动闸/总闸不变）**」↔ 本批服务端「**手动闸/总闸（`wake_phrase` 显式空）/清洗/归一化/长度/token 全不变**」⇒⇒ **不是靠工具同步，是两边各自写着同一句话**
- ⭐⭐ `wake_phrase_matched` 记着一次**谎报**：「**旧实现恒 `true`，等于对『按住说话』谎报『听见了唤醒词』**」⇒ 又一次「如实报标志」（B0348 `residue` 族）
- ⭐⭐ **判据真源被点名**「`gate::evaluate_mode`（四态），handler **不得内联一份等价判定**」⇒ 又一次「判据集中、禁止走私」⇒ **B0291 的实操版**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0386 · ⭐⭐⭐ **这条链闭合了**
- ⭐⭐⭐ `live2d-ai-mod-voice-input/src/gate.rs` `GateOutcome` **四个变体各自带着前置条件**：`Allow { text }`（**可能为空串、由调用方按空文本拒绝**）· `ManualOff` · ⭐ `GateClosed`（**显式**空/纯空白，**键缺失不算**）· `WakeRequired`（**含空输入**）
- ⇒ ⇒⇒⭐ **同一句纪律出现三处**：**handler 头注**（B0385）· **枚举变体文档**（本批）· **客户端收敛逻辑**（B0383）⇒⇒⇒ **任何一处被删掉，另两处还在**
- ⭐⭐ **`Allow { text }` 可能带空串、明确「由调用方按空文本拒绝」** ⇒ **一个状态不负责下一层的判断** ⇒ 同 B0349 可空性家族 ⇒ 与 `voice_routes` 第 4 步「归一化 → 空文本判定」**对得上**
- ⭐⭐⭐ **这条链闭合**：① 两个入口·同一道预检（B0382）· ② 取值源 Mod config + 注入（B0384）· ③ 服务端四闸·顺序钉死（B0385）· **④ 判据四态·禁内联（本批）**
- ⇒ ⇒⭐ **而本批最值钱的是「一条纪律在三处被独立说出」** ⇒⇒ **这比「一处写清楚 + 别处引用」更抗改** ⇒⇒ **因为引用会随重构断掉，重复不会**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0387 · ✅ **+1 条 P2：spec 的文档表漏了 6 个键，含两个闸门开关**
- ⚠⭐ **`voice-input` 的头注「配置字段（settings_spec v1）」表只列 3 个键**（`backend`/`locale`/`token`），**而 `voice_input_settings_spec()` 实际声明 9 个**
- ⇒ 漏掉的 6 个里含 **`wake_phrase` 与 `manual_enabled`** —— **正是 `voice_routes.rs` 四个闸门读的那两个**
- ⇒ ⇒ **查「唤醒闸在哪配 / 能不能关」的人先读这张表 ⇒ 会得出「闸门不可配」的结论**；而配置面板上**确实有**这两项（B0364 核过 spec 驱表单）⇒ **面板里有、文档里没有**
- ⇒ ⇒ **F-0387-01 (P2)** · 修法：**补两行表格**（或改口径为「常用字段」并指向 `voice_input_settings_spec()` 为准）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **79**（P1 6 / P2 36 / P3 37）

### BATCH-0388 · ⭐⭐⭐ **横扫证伪了我自己的推广**
- 四个 Mod 的机械对照：`external-input` **4:7**（表比代码多）· `voice-input` **9:3**（F-0387-01）· `pet-desktop` 3:3 · `persona` **8:2**
- ⭐⭐⭐ 而 `persona` 那张表**是「命令」表**（`import_card`/`clear_import`）、**不是配置索引**；而那 8 个 key 是 **SillyTavern 卡片字段**、**不是配置旋钮** ⇒⇒ **F-0387-01 是 `voice-input` 独有的**
- ⇒ ⇒⭐⭐⭐ **三种不同情况都表现为「计数不等」⇒ 计数不是判据** ⇒⇒ ⭐ **可提炼：跨仓库横扫时「数字不一致」是线索、不是判据；必须逐项确认那张表自称为是什么**
- ⇒ ⇒ **我若停在计数这一步，就会把 `persona` 的「命令表」当成「漏了 6 个配置项」⇒ 一个假发现** ⇒⇒ **这是 B0369「凭片段」的第七次复发**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **79**（P1 6 / P2 36 / P3 37）

### BATCH-0389 · ⭐⭐⭐ **「修法不是加提示，而是把默认换过来」**
- ⚠ `grep syncShellStageBg dev_tools_section.dart` **零命中** ⇒ **照例不是结论** ⇒ 换文件（`display_prefs.dart`）命中 ⇒ **「零命中不是结论」第七次**
- ⭐⭐⭐ `display_prefs.dart:152-167`「# **为什么换掉 `syncShellStageBg`**（**2026-09-27 修一个真缺陷**）」⇒ 症状分**两层**：「加了 3 张图，界面一点变化都没有」**+**「一个**必然无效的按钮**」⇒⇒ **第二层比第一层严重**（困惑 vs 骗人）⇒⇒ **两层都被修**
- ⭐⭐⭐⭐ **修法是「换默认」而不是「加提示」**，理由逐字写明 ⇒⇒⭐⭐ **本审计最清晰的一次「为什么不给它加个提示」**：**加提示处理「用户可能困惑」· 换默认处理「用户本来就该成功」**⇒⇒ **提示是给不可避免的失败兜底 · 换默认是让失败不该发生**（B0353 又一处）
- ⭐⭐ **升级到项目级禁令**：「那是本项目 **P4 明令禁止的「静默失效」**」⇒ 不是个人判断、是引用已有红线 ⇒ 又一次「指路」
- ⭐⭐ **「渲染参数 ≠ 图片来源」被单独说明**（铺法/位置/透明度/模糊/遮罩/过渡**两种来源下都生效**）⇒ **换默认后先说清哪些不变**
- ⇒ ⇒⭐ **第四个「弯路记录」**，失效类型 = **「默认朝向错了」**⇒⇒ **四类至此**：伪造 · 什么都不显示 · 兜底反被咬 · **默认朝向错**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0390 · ⭐⭐⭐ **B0389 那条链闭合了**
- ⭐⭐⭐ `appearance_background.dart:298-300`「**「来源 = 舞台那张」时的**那两个按钮**（换 / 清）**」+ `onPickStageImage`/`onClearStageImage` **都是可空** ⇒⇒ **B0389 记的「一个必然无效的按钮」在结构上被消除了** ⇒⇒ **「条件成立才给回调」**（同 B0326 可空族）
- ⭐⭐⭐ `:322-325`「**判据只在外壳一处（`AppShell._currentBackground`），这里只消费**」⇒⇒ **同 B0335「相位只被派生一次」**⇒⇒ 而注释记着「**外壳注释里不再是「永远第 0 项」**」⇒⇒⭐ **连注释都被同步修过**（同 B0212 的形状，但那次是「正文改、注释留旧」——**这次两边都改**）
- ⇒ ⇒⭐ **B0389 链现在完整**：**默认朝向换过来**（不是加提示·是换默认）· **按钮按来源条件存在** · **当前项判据集中**⇒⇒ **三处合起来 = 那个缺陷的三种失效面各自被一条规则挡住**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0391 · ⭐⭐ **三个 API 的 `null` 有三种含义**
- ⭐⭐⭐ `appearance_background.dart:109-112` 三个 `null` 语义被分开点名：`clearStyle`=**三项一起清的开关** · `copyWith(opacity: null)`=**「不改」** · 逐图编辑器要表达=**「清掉」**⇒⇒ **同 B0349 可空性家族，但这里更进一步：不同 API 的可空含义不同 ⇒ 三个都得说**
- ⭐⭐ **而 `dataUrl` 被单独点名**（不是样式参数、是**字节**）⇒⇒ 后果被写出来（「**缩略图与渲染一起消失，直到下一次水合**」）⇒ **写后果而非只写「要保留」**
- ⭐⭐ `hydrating` 是**显式 bool** ⇒ 「**还没读回来**」与「**真的空**」被区分（B0325/B0362 族）· `maybeOf` 的「**没有外壳注入时返回 null**」写在文档上
- ⇒ ⇒⭐ **「可空」不是一种设计，是每次都要说清的那种** ⇒⇒ **B0349 记的是「可空 = 有一个可推导的默认」；本批补的是另一半 —— 同一文件里三个可空参数含义各不相同，而每个都被点名了**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0392 · ⭐⭐ **B0391 留的问题结清**
- ⭐⭐⭐ `main.dart:204-205`「**水合这一次读结束了（读回成功 / 超时兜底都算结束）**」⇒⇒ **`hydrating` 不会卡在 true** ⇒⇒ 兜底是「**超时**」不是「错误」⇒⇒ **用户看到「读完了、但库是空的」**
- ⇒ ⇒⇒ **不记发现的三个理由**：① 兜底存在 ② 代价被写明 ③ 用户看到「空」而不是「卡住」⇒⇒ **B0362 判据（文档缺口不算发现）再次适用**
- ⭐⭐⭐ 而那条缺陷**记着编号与两个后果**（`F-0013-1`/`F-0001-3`）：旧实现把水合开始时刻的快照整体盖回去 ⇒ **① 窗口期改动全部回滚** **② `migrated` 时把旧对象写回盘**（**更持久 ⇒ 被点名**）⇒⇒ 修法是「**只回填背景域 + 传入当前值**」
- ⭐⭐ 写回盘判据是 `hydrated.migrated` ⇒ **只搬过才写回** ⇒ **不制造需要清理的状态**（同 B0371「换对象而不是逐个清」）· `if (!mounted) return;` 在最前
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0393 · ⭐⭐⭐ **为未来的误判预先写下后果**
- ⭐⭐⭐ `appearance_section.dart:14-16`「**留这段是因为「下一个人很可能再按旧名误判一次」**……一出现就让人以为**删掉它不影响外观设置，实际会整套删掉主题与口型**」⇒⇒ ⇒ **它防的不是「改错」，是「一个看似合理的判断」** ⇒⇒ **而最坏后果被写到最坏**（不是「有点影响」，是「整套没」）
- ⇒⇒⇒ **同 B0380「重放最易覆盖掉」的思路** ⇒⇒ **两者都不是防「改错」，是防「一个看似合理的判断」**；⇒ 与 B0370「删通道前先列清单」同族，**而那一条的对象是一个通道、这一条的对象是一次删除决定**
- ⭐⭐ **「以名实相符」是改名的理由**（不是「更好听」）
- ⭐⭐ **豁免的**进展**被记着**：`1011 行抽出` · `1489 → 约 700` ·「**仍超 ≤500**」⇒⇒⇒ **「豁免」是进行中的状态、不是永久标记** ⇒⇒ 同 B0329「代价如实记录」（那里记代价、这里记进度）
- ⇒ ⇒⭐ **「为未来的误判预先写下后果」与「为未来的修改写下理由」是两种注释**，**后者更常见** ⇒⇒ **前者只在「误判会不可逆」时才被写下来**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0394 · ⭐⭐ **「只摆一套」也是结构性的**
- ⭐⭐⭐ `appearance_section.dart:241` `if (**!librarySource**)` ⇒ **互斥是结构性的**，不是「都摆出来、让用户自己选」⇒⇒ **同 B0342「禁用在输入层生效」**
- ⭐⭐⭐ 理由是**用户会把两套当成一套**（「**同时摆出来用户只会以为它们是一套**」）+ 引 `rc.5 §9.2`⇒⇒ **又是「指路」**，且**同 B0389 的思路：防的是用户的合理误解**
- ⭐⭐ **「两套列表、两套预算、两条下发通道」逐一点名** ⇒⇒ 三样都不同 ⇒ **摆在一起时无法从界面上区分**
- ⭐⭐⭐ **边界的澄清句用否定式**：「这一块管的是**舞台那张的历史列表，不是背景库**」⇒⇒⇒ **用「不是」划界比用「是」更难被误读**（同 B0324 族）
- ⇒ ⇒⭐ **「来源」的三种失效面现在都有结构守卫**：① 按钮必然无效（可空回调·B0390）· ② 「当前项」算错（判据只在一处·B0390）· **③ 两个入口同时出现（`if (!librarySource)`·本批）**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0395 · ⭐⭐ **「别名不是第二套配额」**
- ⭐⭐⭐ `display_prefs.dart:68/75/81/92` 四个常量：**`kStageImageMaxChars=1500000`** · **`kShellImageMaxChars = kStageImageMaxChars`（别名）** · `kStagePlaylistMaxItems=16` · `kStagePlaylistMaxChars = kStageImageMaxChars`
- ⇒⇒⇒ ⭐⭐⭐ **而「别名」被解释得最清楚**：壳背景与舞台背景写在**同一条 localStorage 记录**里，**给两份独立上限只会让「整份偏好都写不进去」更容易触发**⇒ **只是可读的别名、不是第二套配额**⇒⇒ **一个常量存在两次的理由不是「方便」，是「让读者以为有两套」**
- ⭐⭐⭐ 而 **1500000** 有完整理由链：**为什么是这个上限**（超配额会连带丢主音量/口型 · **代价远大于一张图没记住**）· **超限怎么表现**（「不是错误 · 照常生效 · 只是不写盘 · **UI 会如实说明**」）· ⭐⭐⭐ **以及没做什么**（「**下游裁剪没有做** —— 需要浏览器 canvas · **属于不可测代码** · **刻意不引入**」）
- ⇒⇒⇒⭐ **而给出的拒绝理由是「不可测」** ⇒⇒ **这是本仓的一贯判据**（「这一步会引入不可测的代码」本身是充分的拒绝理由）⇒⇒ **同 B0321/B0361 同一族**
- ⭐⭐ **「16 张」被指向主上限**（「**真正的上限是总字符预算**，这一条只挡住『项数无界』」）⇒ **次级上限被指向主上限、并说清自己是次级的**
- ⭐⭐ 总长上限的行为逐字写明：「**超出时保留前面的、丢掉放不下的**，并在界面上**如实说明「已达上限」**，而不是**悄悄把整份设丢掉**」⇒ 又一个 **P31**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0396 · ⭐⭐⭐ **承诺兑现成什么形状**
- ⚠ `grep 已达上限` 在 UI 侧**零命中** ⇒ **照例不是结论** ⇒ 换问法（裁剪处有没有说话）⇒ **有**
- ⭐⭐⭐ `shell_prefs.dart:352-362` B0395 承诺的「如实说明『已达上限』」**真的存在**，且**带上具体数字**（`$kStagePlaylistMaxItems 张`）⇒⇒ **不是四个字，是「已到上限 16 张」**
- ⭐⭐⭐ **五个分支 = 四个具名 reason + 一个诚实的 `_` 兜底**（「这张图**没能**加入」· **不假装知道原因**）⇒⇒ **同 B0335「穷举 switch 无 `_`」的对照**
- ⭐⭐⭐ **四个具名分支每个都带一个动作**，且**动作名是界面上按钮的原名**（用「」引起来）⇒⇒ **P31 第四次在 UI 侧**⇒⇒ **照着念就能做**
- ⭐⭐ **三种「满」被分开**：`item_too_large`（这张图太大）· `limit_reached`（**张数**满）· `budget_exceeded`（**总长**满）⇒⇒ **三件事、三种处置**（换图/清空/清空）· `'empty'` 也在
- ⇒ ⇒⭐ **而本批最值钱的是第 1 点**：**「承诺是否兑现」的检查必须问到「兑现成什么形状」** —— **兑现成一个词** 与 **兑现成「数字+动作+三种原因分开」** 是两件事 ⇒⇒ **后者才叫兑现**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0397 · ⭐⭐ **成功被建成一个具名状态**
- ⭐⭐ `display_prefs.dart:1085-1116` `appendToStagePlaylist` **四个失败态按固定顺序判定**（空 → 单张太大 → **张数**满 → **总长**满）⇒⇒ **先便宜的判据在前**，而 **B0396 的 UI switch 顺序与之一致** ⇒⇒ **文案与判据同源**
- ⭐⭐⭐⭐ **而 `reason` 有第五个值 `'added'`** ⇒⇒ **类型是「结果」不是「错误」** ⇒⇒⇒ **同 B0335「六个相位全可达（含 `idle`）」家族**——**成功被建成具名状态、而不是「没有错误」**
- ⇒ ⇒⇒⭐ **B0396 的 switch 只写四个失败分支 + `_` 不是遗漏** —— **因为调用方在 `if (!result.added)` 里分了流** ⇒⇒ **穷举完整靠外层分支保证** ⇒⇒ **同 B0342/B0349：靠结构保证、不靠自觉**
- ⭐⭐⭐ **每个失败分支都返回 `playlist: current`（原样不动）** ⇒⇒ 不是「只在末尾统一处理」⇒⇒ **B0395 注释说的正是这个** ⇒⇒ **注释与实现是同一件事的两半**（B0393 的正面例）
- ⭐ `total` 是**现算**的 ⇒⇒ 不依赖某个需要维护的计数字段（同 B0371）
- ⇒ ⇒⭐ **而这比「在 switch 里写五个分支」更好**：**五个分支会诱使人把 `added` 写成空处理**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0398 · ⭐⭐⭐ **「不引入无法回归的代码」是本仓的判据**
- ⭐⭐ `browser_io.dart:80-92`「**配额满不是假设**：localStorage 常见上限 **5 MB**，而 `ChatSessionStore` 用 **`maxSessions` × `kMaxMessagesPerSession` 双重夹持**」⇒⇒ **B0262 核过的「双重夹持」在这里被点名**（与 B0143 裁剪 / B0215 `kChatHistoryLimit` = **三层**）
- ⭐⭐ **「静默」的两条理由被分级**：**无痕模式（预期的）** / **配额满（兜底的）**⇒⇒ 同 B0287「有理由才清」
- ⭐⭐⭐⭐ `:98-101`「**不做缩放（刻意的）**：裁剪要走 canvas · 只能在浏览器里跑 · **`flutter test` 覆盖不到` —— 本轮不引入无法回归的代码**。代价是「**大图不写盘**」，由调用方**如实告知用户**」⇒⇒⇒ **与 B0395 的「下游裁剪……**不可测代码**、刻意不引入」是同一判据的第二次出现**
- ⇒ ⇒⇒⭐ **它是本仓的判据、不是这一次的选择** ⇒⇒ **两次的代价都被逐字写下、且都指给了下游** ⇒⇒⇒ **「拒绝一个改进」与「记下它的代价」成对出现**——**只有后者被反复做时，前者才站得住**
- ⭐⭐⭐ `:102-107` **三态由两个可空字段编码**（取消 = 两者皆 null）⇒⇒ **正是 B0340 那个坑的形状** ⇒⇒ 而注释明说「以前读失败与取消**不可分**、用户看到的就是『**点了选图没反应**』」⇒⇒ **一个已修坑的**症状**被写下来了**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0399 · ⭐⭐ **一次判断被写下来：合并是对的**
- ⭐⭐⭐ `browser_io.dart` `loadChatSessions`：**「空」三个来源**（键不存在 · 空串 · JSON 坏）**收敛到同一个 `empty()`**
- ⇒ ⇒⇒⭐ **而我判断这个合并是**正确的** —— **存档是缓存**（对话仍在内存/服务端）· **坏掉的存档没有可恢复的内容** ⇒⇒ **「降级成空」与「降级成默认」是同一种正确选择**；且 B0398 写侧静默理由也是「**不该影响正在进行的对话**」⇒⇒ **两侧口径一致**
- ⚠ **代价**：**真的 bug 也会被吞成「空存档」** ⇒⇒ **只记为口径观察、不进 FINDINGS**（可核的改进方向：分开 `jsonDecode` 与 `fromJson` 两处错误文案 —— **但用户可察觉的后果都是「没有历史」⇒ 属改进非缺陷**）
- ⭐ `raw.isEmpty` 那个防御判据防的是「**外部编辑 / 旧版本残留**」（写侧 `jsonEncode` 不会产生 `""`）
- ⇒ ⇒⭐ **而本批的收获是「一次判断被写下来」**：**「三种『没有』合并成一个值」既可能是疏忽、也可能是对** ⇒⇒ **区分办法只有一个：问「它们可不可区分、有没有不同的正确处置」**⇒⇒ **B0388 那条判据的第三次应用**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0400 · ⭐⭐⭐ **策略是分层的，而 B0399 的判断只在一个层次上成立**
- ⭐⭐⭐⭐ `chat_session.dart` `ChatSessionStore.fromJson` **版本门禁的第二个判据**：「不认识的版本**整体丢弃**而不是尽力解析 —— **可能把新字段写没了**」⇒⇒⭐ **理由是写回** ⇒⇒⇒ **与「不可恢复」是独立的两个理由**（即使旧格式**完全可解析**也整丢）
- ⭐⭐⭐ **而版本对时逐条容错**（坏的那条**被跳过**）· `activeId` 是**第三种处置**（「指向不存在的会话（**存储被外部改坏**）→ 当作没选」）⇒⇒ **三个坏法、三种处置**
- ⭐⭐ **`_trimSessions()` + `_trimMessages()` 在反序列化后被调用** ⇒⇒ **B0398 核的「双重夹持」不只是写入时** ⇒⇒ **外部塞进来的大档不会撑爆存储**
- ⇒ ⇒⭐⭐⭐ **而这修正了 B0399 的措辞**：**「三种『没有』合并成一个值」在 `loadChatSessions` 那一层成立**；**到了 `fromJson` 那一层被分成三种处置** ⇒⇒⇒ **分层才是真相：「合并」在粗粒度入口、「区分」在细粒度解析** ⇒⇒⇒ **⇒ 我 B0399 那条判断只在一个层次上成立，而我当时没说是哪个**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0401 · ⭐⭐ **「必须丢」与「可以补」的界是 `id`**
- ⭐⭐⭐ `chat_session.dart:90-93`「**坏数据返回 `null`（丢弃这一条），绝不抛**。与 **`DisplayPrefs.fromJson` 同一条纪律**……**丢一条会话的代价远小于丢整个列表**」⇒⇒ **丢弃的粒度由代价的不对称决定** ⇒⇒ **不是「宽容」，是「代价小的那侧」**
- ⭐⭐⭐⭐ **而「必须丢」与「可以补」的界，是 `id`**：`raw` 不是 Map → 整条丢；⭐ **`id` 空/缺失 → 整条丢**（**没有 id 就无法被 `activeId` 引用、无法被 trim 定位**）；`createdAt` 坏 → 补 `epoch 0`；`title` 坏 → 补 `''`；某条 `message` 坏 → 只丢那条
- ⇒ ⇒⇒⭐ **`id` 是这条记录**唯一不可替代**的东西** ⇒⇒ **同 B0347 核的 persona「按 id 定位」是同一条纪律**；⭐ `epoch 0` 的缺省**方向安全**（旧的沉底、不占前面）
- ⭐⭐ **「分层」至此三层**：`loadChatSessions` 入口整丢 · `ChatSession.fromJson` 记录级丢 · `ChatMessage.fromJson` 消息级丢 ⇒ **每层有自己的丢弃判据**
- ⇒ ⇒⭐ **而本批最值钱的是「界 = 唯一不可替代的那一栏」** ⇒⇒ **这个判据可迁移**：**任何「解析失败就丢」的地方，都该先问「它缺的那一栏是不是不可替代的」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0402 · ⭐⭐⭐ **判据在消息层成立、且最锋利**
- ⭐⭐⭐⭐ `chat_message.dart` `ChatMessage.fromJson`：**未知 `role` ⇒ 丢弃整条**，**而不是「退化成 user/assistant」**
- ⇒ ⇒⇒⭐ **而理由在别处已可核**：**B0327** 核过 `Semantics(label: '${ChatRole.system.label}提示：$text')` ⇒ **角色决定它被怎么称呼**；**B0360** 核过 `role == ChatRole.system ⇒ _SystemRow` ⇒ **角色决定它走哪条渲染分支**⇒⇒⇒ **「猜一个角色」= 把内容送到错的渲染分支，而送错比丢掉更糟**
- ⭐⭐⭐ **「不可替代的栏」从一个变成三个，理由各不相同**：`id`（**无法被引用/定位**·B0401）· **`role`（无法被正确渲染**·本批）· `text`（**消息之所以存在的部分**）
- ⭐⭐ **而 `epoch`/`failed`/`unfinished` 三个全部可缺省** ⇒ **它们是「附加状态」、不是「身份」**（同 B0247 的「失败轮标记」）
- ⭐⭐ `for (c in ChatRole.values)` 而**不是** `values.byName(...)` ⇒ **前者不抛、后者抛** ⇒ 与「坏数据不抛」同一条纪律；⭐ `raw['failed'] == true` ⇒ **缺字段与 `false` 同义**
- ⇒ ⇒⭐ **而本批最值钱的是第 1 点**：**「猜一个角色」不会立刻错** —— **用户消息被画成系统提示、反之亦然，两者都还「看起来正常」** ⇒⇒⇒ **这类错误不报错、只会被忽略 ⇒ 所以这里只能靠「丢」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0403 · ⭐⭐⭐ **判据在写侧也成立**
- ⭐⭐⭐⭐ `chat_message.dart` `toJson`：**读侧必需的两个字段（`role`/`text`）必写**；**读侧可缺省的三个（`epoch`/`failed`/`unfinished`）缺省不写** ⇒⇒⭐⭐ **「只存非默认」是为 5 MB 配额服务的** ⇒⇒ **B0398 核的「双重夹持」那条硬约束，在「存」这一侧也按同一预算设计**（我此前只从「夹持」那一侧看过）
- ⭐⭐ `if (failed) 'failed': true` ⇒ **只写 `true`** 而读侧 `raw['failed'] == true` **把缺字段解释为 `false`** ⇒ **两侧是同一个约定**
- ⭐⭐⭐ `epoch` 用 `if (epoch != null)` 而**不是** `if (epoch != 0)` ⇒ **「没有 epoch」与「epoch 是 0」是两种状态**（B0349 族）⇒ 与 B0402 读侧 `rawEpoch is int ? rawEpoch : null` **严格对称**
- ⇒ ⇒⭐ **整条纪律读写两侧都成立**：**不可替代的 ⇔ 必写 · 可替代的 ⇔ 缺省不写** ⇒⇒ **单看读侧只知道「缺了会怎样」；两侧都看才知道「存的时候就已经省了」**
- ⇒ ⇒⭐ **而本批的价值是「反方向用同一条判据」**：**B0401 在读侧提炼 · B0403 在写侧验它成立** ⇒⇒ **一条只在单侧成立的判据是「读得通的借口」，两侧都成立的才是纪律**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0404 · ⭐⭐⭐⭐ **别让判据自己消失**
- ⭐⭐⭐⭐ `ChatSessionStore.toJson`：**`version` 永远写**，而它**就是那个门禁的判据**（读侧 `version != kChatStoreVersion ⇒ 整丢`）⇒⇒⭐⭐⭐⭐ **若它也走「缺省不写」，新存的文件就没有门禁字段 ⇒ 读侧会把「自己刚写的文件」整丢**⇒⇒ **「必需 ⇔ 必写」在最要紧处生效**
- ⭐⭐⭐ **`activeId` 写 `null` 而不是省略**，读侧把「指向不存在的会话」也归成 `null` ⇒⇒ **四种可能的输入、两种可能的输出** ⇒ 与 B0400「三种坏法、三种处置」对齐
- ⭐⭐ **`sessions` 哪怕空也写 `[]`** ⇒ 「没有会话」被显式表达（**同 B0349「要写 `null` 而不是省略」族**）
- ⇒ ⇒⭐⭐⭐ **而本批把 B0401 那条判据推到了它的边界**：**「不可替代 ⇒ 必写」在 `version` 上是最强的形式** —— **一个字段的作用是「让读侧整丢我」，而它自己一旦缺失，读侧就会整丢包括它自己在内的一切**⇒⇒⇒⭐ **所以这条规则不只是「别丢信息」，还是「别让判据自己消失」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0405 · ⭐⭐⭐ **同一预算、不同层不同手段**
- ⭐⭐⭐ `ChatSession.toJson` **五项全写**（`id`/`title`/`createdAt`/`updatedAt`/`messages`）⇒⇒ **「缺省不写」被**限定在消息层**、并有理由**：`role`/`text` 是消息主体、**而 `epoch`/`failed`/`unfinished` 是大多数消息都没有的附加状态**
- ⇒ ⇒⇒⭐ **同一份文件里两层用了两种策略，而判据是统一的**：**可替代 ⇔ 缺省不写 · 不可替代 ⇔ 必写**（消息层三项可替代、会话层五项全不可替代）⇒⇒⇒ **「全部必写」与「缺省不写」不是两种风格，是同一条规则在两层上的两个结论**
- ⭐⭐ `createdAt`/`updatedAt` 写 **`toIso8601String()`**（可读、带时区）而读侧是 `_readTime(...)` ⇒⇒ **格式与类型对称，不是格式对称**
- ⭐⭐ **而 `title` 常为空仍全写** ⇒⇒⇒ **「常为空」不等于「可省略」**
- ⇒ ⇒⭐⭐⭐ **而这修正了 B0404 留的 ⚠「两处夹持机制不同」—— 那是对的、且正是本批结论**：**消息层「不写没用的」· 会话层「丢不下的」**⇒⇒ **同一个预算约束、不同层不同手段** ⇒⇒ **比「全用一种手段」更省、也更难写坏**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0406 · ⭐⭐⭐ **「有界」是两个决定**
- ⭐⭐⭐⭐ `render_events.dart:190-191`「**观测缓冲的默认上限（阶段 5 §2 B 栏冻结：上限 200）**」⇒⇒ **一个数有出处、有栏位、有它被定下来的那次** ⇒⇒⭐ **而「冻结」是一个状态词** ⇒⇒ **它告诉后来者「这个数可以调，但改它要动决策文档」**（同 B0242/B0365 族）
- ⭐⭐⭐ 「**观测缓冲只住内存，不写任何持久化（本栏的红线）**」⇒⇒ **留存边界被提升到红线级**（同 B0307「什么不进产物」族）
- ⭐⭐⭐⭐ **两种「有界」，可接受性完全不同**：**观测缓冲 = 有界环形（覆盖最旧，丢了没人察觉 ⇒ ✅）** vs **会话列表 = 夹持（丢了用户会说「我的历史呢」⇒ 必须如实说明「已达上限」·B0395）**
- ⇒ ⇒⇒⭐ **判据是「丢了之后用户会不会察觉」** ⇒⇒ **察觉不到 ⇒ 环形覆盖（最省、零打扰）**；**察觉得到 ⇒ 夹持 + 一句「已达上限」** ⇒⇒⇒ **⇒ 这解释了 B0405 为什么两层手段不同**
- ⭐⭐ 「纯逻辑（不 import Flutter）⇒ 可单测」· 单例 hub 位置被点名（又一次「指路」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0407 · ⭐⭐⭐ **「有界」的第一问是「谁有资格留在环里」**
- ⭐⭐⭐⭐ `render_events.dart:265-275`「**阶段 5 维护者补丁（D44）**：stage-clock 是 **30ms 的连续信号**，不是事件。**逐条入环会让 200 条容量在约 6 秒内被时钟灌满**，**action_cue / ack 全被挤掉**、**B 栏实际不可用**。这里按 `(type, seq)` **折叠**」
- ⇒ ⇒⇒⭐ **「有界」的第二种手段**：**挤掉最旧**（记录同等重要时）· ⭐ **先按 `(type, seq)` 折叠**（**有一类是连续信号、会淹掉其它**时）⇒⇒⇒ **「有界」不是「丢得多」，是「先决定谁有资格留在环里」**
- ⭐⭐⭐ **理由被量化**：「**200 条在约 6 秒内被灌满**」⇒ **30 ms × 200 = 6 秒 · 这个数是算出来的**（同 B0329 族）
- ⭐⭐⭐ **而「B 栏**实际不可用**」被写出来** ⇒⇒ **不是「难看」、是「这一栏的功能没了」** ⇒ **「不可用」比「难用」更能推动一次修补**（同 B0348 的写法）
- ⭐⭐ **折叠的键是 `(type, seq)`** ⇒⇒ **「同一段」可判定**（`seq` 由渲染面给）⇒ **不是靠猜**；⭐ 消费侧 `:136-139` 对另一组列表**复用同一个上限**
- ⇒ ⇒⭐ **而本批把 B0406 的判据反过来用**：**stage-clock 之所以特殊，是因为它挤掉的那 200 条是**被察觉的**（ack 消失了）**
- ⚠ **按 B0375 判据检查后不记发现**：`:254-255` `assert(capacity > 0)` 在 release 下失效、而 `capacity` 是构造参数 ⇒ **但生产侧唯一构造用缺省 200 ⇒ 「capacity=0」不可达**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0408 · ⭐⭐⭐ **「有没有在跑」与「是谁造成的」被分在两处**
- ⭐⭐⭐ `ui_phase.dart:53-79` `UiSignals` **每个信号的定义都是线路级的**：`WsStatus == connected` · POST **已受理 ∧ 未收口** · `voice_started ∧ ¬voice_ended` · `new_epoch` 后的短窗口 · 有未关闭的错误⇒⇒⇒ **五个没有一个是「界面该显示什么」** ⇒⇒ **`deriveUiPhase` 才做那一步**
- ⭐⭐⭐⭐⭐ **而这个类刻意**不包含「谁造成了这一轮」** —— 它只记「有没有在跑」，**「是谁」由 `settleTurn(stopped:)` 那条链回答**（B0304–B0313 核过的八层判据）⇒⇒⇒ **⇒ 两者**正交**、不是同一件事的两种写法 —— 而这个正交是我此前一直默认、没核过的**
- ⭐⭐⭐ **「打断窗口」被逐字推到调用方**，理由「**定时器是副作用，不属于纯函数**」⇒⇒⇒ **这一句同时做两件事：① 保住纯度 ② 点名谁该负责那个定时器** ⇒⇒ **而「责任在调用方」写在**被推出去的那一方**的注释里**
- ⭐⭐ **全是布尔/枚举 ⇒ 可 `const` 构造** ⇒⇒ **「状态是否变了」可以按值比较**（B0326 族）· 三条纯度约束同时写明（无 `BuildContext` · 无 `Stream` · 无时间 · 同 B0332）
- ⇒ ⇒⭐ **「有没有在跑」是状态（可比较、可派生、可测试）；「是谁造成的」是收口时的判据（一次性、需要上下文）** ⇒⇒ **而「打断窗口」被推到调用方，正是这个分离能成立的技术前提**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0409 · ⭐⭐⭐⭐ **纯度与可测性是同一件事**
- ⭐⭐⭐⭐⭐ `ui_state_tracker.dart:5-9`「`ui_phase.dart` 是纯函数，但『**信号从哪来、什么时候清**』**同样是逻辑**、**而且是更容易错的一半**：`voice_started` 之后**谁负责清掉**？**`new_epoch` 的打断窗口由谁计时**？留在 `main.dart` 里就**永远测不到** —— 而它**必须 `import 'package:web'`**」
- ⇒ ⇒⭐⭐ **⇒ 这纠正了「逻辑 = 计算」的直觉**：**逻辑的一半是「时序与归属」** ⇒⇒⇒⭐⭐ **⇒ 而那一半**不 import 任何东西** ⇒⇒⇒⇒ **⇒ 「纯度」与「可测性」在这里是统一的一件事**
- ⭐⭐⭐ **定时器是注入的**（`TimerFactory` 可传可省）⇒⇒ **这正是 B0408「定时器是副作用、不属于纯函数」的落点** ⇒⇒ **纯度被保住、副作用被参数化**（同 B0179 注入时钟）
- ⭐⭐⭐⭐ **`:223` 一句**自我更正**（「刚设上的 `_interrupted = true` **立刻被自己抹掉**」）⇒⇒ **同 B0290 核过的那处** ⇒⇒ **第三次出现「自我更正型注释」**⇒⇒⇒ **⇒ 而它在「谁负责清」这个**被专门命名过**的问题上 ⇒⇒⇒⇒⇒ **命名过的难点，恰好就是留痕最多的地方**
- ⇒ ⇒⭐⭐⭐ **本批把三次观察统一了**：**B0361（面板不 import browser_io）· B0328（气泡只依赖纯模型）· B0179（把时钟做成参数）**⇒⇒⇒ **三次不同的观察、一个共同的根据：依赖方向即测试性**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0410 · ⭐⭐⭐⭐ **我的笔记少了一项**
- ⭐⭐⭐⭐ `live_region.dart:11` 说限速「**只在正文真的变长时**才更新」⇒ 实现 `:57` `if (fullText == _text) return false;`⇒⇒⭐⭐⭐ **⇒ 我 B0350 记的「两级放行」其实是**三项**：**① 文本没变（直接丢）② 有句末标点（立刻）③ 距上次 ≥ 1.5 s（兜底）**⇒⇒⇒ **⇒ 而漏掉的那项恰恰是**最省**的那个**
- ⭐⭐⭐ 「淹没」的代价写成用户可感知的形态（「一串**互相打断的音节，比不说话更糟**」）⇒ 同 B0389 的写法（不是「有点影响」，是「更糟」）
- ⭐⭐⭐ 「整句到达」被称作「**有意义的语义边界**」⇒ **「一句一单元」红线在读屏侧的落点**；⭐ 句末标点声明为「与 `live2d-ai-runtime` 的分句标点**对齐**」⇒ **跨语言对齐又一次**
- ⭐⭐⭐ `finish` 的两个理由（不清空 → **下一轮先念上一轮残留** · 不补播 → **最后一句读屏用户永远听不到**）⇒ 同 B0350 核的 `settleTurn`「最后没句号的那句」⇒ **同一形态在音频与读屏两侧各有一个出口**
- ⇒ ⇒⭐⭐⭐⭐ **而本批最重要的产出是「我自己的笔记少了一项」**：「两级放行」是我**从注释里读的**，而第三个条件**写在注释的另一个位置**（`:11` vs `:57`）⇒⇒⇒⭐ **⇒ 我核了那个注释、没核那个函数 ⇒ 又是「读过两侧、没并排」**——**B0369 形态第七次复发（这次的两侧是**同一段注释的两处**）**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过**

### BATCH-0411 · ✅ **+1 条 P2：「对齐」不成立，两侧差四个字符**
- ⚠⭐ **`live_region.dart:19-22`「**与 `live2d-ai-runtime` 的分句标点对齐**」是注释里的断言** ⇒⇒ **逐字比之后不成立**
- | 字符 | Dart `'。！？…；\n'` | Rust `.?!。！？…\n` | ⇒ **Dart 缺 `.` `?` `!`，多 `；`** —— **两个方向都不同**
- ⇒ ① **英文句尾不触发「立刻播报」** ⇒ 退化为 1.5 s 兜底（**降级、不丢信息**）② **全角分号处提前播报**（**提前、不丢内容**）⇒ **故 P2 非 P1**
- ⇒ ② ⭐ **并修正我自己 B0350 的说法**：「读屏播报与切句**共用**判据」当时**依据是注释** ⇒ **逐字比后只对「判据的形状」成立、对「标点集合」不成立**
- ⇒ ⇒ **F-0411-01 (P2)** · 修法**一行**：让集合逐字相同 + 注释改准；**或**改注释为「只认全角（英文句尾走 1.5 s 兜底）」
- ⇒ ⇒⭐ **可提炼**：「**跨语言对齐**」的断言**必须被一条跨语言断言守住** —— 否则它只是一句会过期的注释（同 B0344「防的是过期合规」的应用）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0412 · ⭐⭐⭐⭐ **反证条款被用上了、结果是「反证不成立」**
- ⭐⭐⭐ `sentence.rs:23-24` `is_terminator(c) = matches!(c, '.' | '?' | '!' | '。' | '！' | '？' | '…' | '\n')` ⇒⇒ **代码逐字证实 `mod.rs` 的记载** ⇒⇒ **F-0411-01 差异表的 Rust 侧那半完全正确**
- ⇒ ⇒⭐⭐⭐⭐ **而 `；` 那一半更锋利**：`；` 被显式归入 **`is_soft_break`（弱标点）**，且 `is_terminator` 文档**逐字写着「不含分号/冒号等弱标点」**⇒⇒⇒ **⇒ Dart 把 `；` 放进 `kSentenceEnders` = 在「协议明确规定不是句界」的位置立即播报**⇒⇒ **不是「多了一个无害字符」，是「与一条显式协议约定相反」**
- ⭐⭐⭐ **Rust 侧的理由是「**协议固定**」** ⇒⇒ **集合是契约、不是实现细节** ⇒⇒ 「让 Dart 引用同一集合」有了更强依据；⭐ **根因**：`is_soft_break`（停顿点）这个概念**在 Dart 侧不存在** ⇒ **缺了那一档**
- ⇒ ⇒⭐⭐⭐⭐ **而本批最值钱的是这一点**：**我第一次在一批之内用自己写的反证条款反过来**加强**了发现**⇒⇒ **「写出反证条款」与「下一批真的用它」是两件事**⇒⇒ **同 B0319「不可核标签也会出错」的对称面**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0413 · ⭐⭐⭐⭐ **不是「集合不同」，是「档位不同」**
- ⭐⭐⭐⭐ `sentence.rs:123-136` **安全阀是三层降级**：**句界**（`.?!。！？…\n`）· **降级①弱标点**（`，、；：`·听感是自然停顿）· **降级②字符位置硬切**（一句话被劈开）⇒⇒ **排序判据 = 「这一层切出来的东西，用户会不会觉得是坏的」** ⇒⇒ **硬切是最后一层、且条件是「确实一个弱标点都没有」**
- ⭐⭐⭐ **「正常路径**绝不**走到这里」被明说** ⇒ **安全阀的标准写法：先说它不该被走到**（同 B0407 写法）
- ⭐⭐⭐ **触发条件是「**模型吐出一大段没有标点的文字**」** ⇒ **安全阀防的是「模型不配合」，不是「文件坏了」**
- ⭐⭐⭐ **旧值 `48` 的后果被逐字写下**（**按字符位置硬切** · TTS 侧表现）⇒ **一次调参事故** ⇒ 解释了那个数为何从「切句目标」改名为「安全阀」
- ⇒ ⇒⭐⭐⭐⭐ **而本批把 B0412 的根因说完了、并**改变了 F-0411-01 的性质**：**Dart 只有「句界」一档、这一侧有三档** ⇒⇒⭐⭐ **⇒ `；` 在 Rust 侧不是句界、它是「降级①」的切点，而 Dart 把降级①的点当成了正常档** ⇒⇒⇒⇒⇒ **⇒ 修法更准：不是把 `；` 拉进那一档，而是让 Dart 明确「我只有句界这一档」或补上「停顿点」档但写明它不构成立即播报的理由**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0414 · ⭐⭐⭐⭐ **一条比值 + 一个 clamp**
- ⭐⭐⭐⭐ `sentence.rs:63-67`「现在默认 **200**：**正常口语句长（中文约 10–40 字）永远碰不到这个上限**，因此**一句话恰好对应一次 TTS 请求**」⇒⇒ **200 ÷ 40 ≈ 5 倍余量** ⇒⇒⇒⭐⭐ **⇒「一句一单元」红线在**参数层**的落地是一个**被算出来的比值**
- ⭐⭐⭐⭐ **旧值的失败被追到用户可感知的形态（四步）**：一句被拆成多个请求 ⇒ 每个请求各有一段合成延迟 ⇒ 播放时逐段空档 ⇒ 听感即「…」（同 B0329 写法）
- ⭐⭐⭐⭐⭐ **而 `max_chars: max_chars.max(1)`** ⇒⇒ **⇒ 这正是 B0407「`assert(capacity > 0)` 在 release 下失效」的**正确做法** —— **不是断言、是一个 clamp** ⇒⇒⇒⭐⭐ **⇒ 同一关注、两种语言、两种可靠性（Dart 用 `assert` · Rust 用 `max(1)`）⇒⇒⇒⇒⇒ **⇒ 又一次看到：断言不是钳制**
- ⭐⭐ `200` 与 `kStageImageMaxChars = 1500000` 量纲不同，而**两者都在各自文件的「预算」注释下** ⇒ **「预算」是个类别**
- ⇒ ⇒⭐⭐ **本批把一条五批的线收了尾**：B0410 读屏判据（**我笔记少一项**）· B0411 集合不一致（**F-0411-01 P2**）· B0412 Rust 代码证实（**反证不成立、发现被加强**）· B0413 **三档降级 ⇒ 「档位不同」** · **本批 档位的参数 + clamp 不用 assert**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0415 · ⭐⭐⭐⭐ **补上那条原则的另一半**
- ⭐⭐⭐⭐ `shell_admin.dart` `_loadAdmin` **两次嵌套降级**：`/env` 失败 ⇒ **沿用旧值**（「**失败不连坐**：`/env` 挂了不该让**整个管理面板**报错」）· `/api/v1/logs` 403 ⇒ **它自己处理**（「**分开取**，403 不该让整个面板失败」·B0352 已核）⇒⇒ **⇒ 而「整块失败」只留给前三个调用 ⇒ 四个调用、两种失败语义**
- ⭐⭐⭐⭐⭐ **而「不连坐」补上了 B0392 那条原则的另一半**：**B0392**「**降级不能压过错误**」（错误不能被盖住）· **本批**「**降级不能把无关的也一起盖住**」⇒⇒⇒⇒⭐⭐⭐ **⇒ 两半合起来：「降级要只降该降的那一处，且降级的痕迹要看得见」**
- ⭐⭐ `_adminError = e.toString()` ⇒ **错误原文上屏**（同 B0352）· ⭐⭐ 两次 `_refresh()`（开始/结束）而**降级①在两次之间、不额外刷新** ⇒ **中间态不触发界面动作**（同 B0346「成功才清」）
- ⇒ ⇒⭐⭐⭐ **而本批的收获是「补上另一半」**：**我在 B0392 记「降级不能压过错误」时以为自己已掌握那条原则**⇒⇒⭐ **⇒ 而那只说了「不许盖住错误」、没说「不许盖住别的」**⇒⇒⇒⭐⭐ **⇒ 两次都做对、我只看见一次 ⇒ 同 B0369「凭片段」的形状：把一条原则的**一半**当成了全部**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0416 · ⭐⭐⭐⭐ **「唯一出口」是结构级的**
- ⭐⭐⭐⭐ `grep -c "setState(" lib/app/shell_admin.dart` = **0**（全文件零处），而 `_refresh()` 被调十几次 ⇒⇒ **⇒ 刷新只有一个出口** ⇒⇒⇒ **「唯一真源」在这里是结构性的**（B0335 核的相位派生同形）
- ⭐⭐⭐⭐ 而 `:4` 头注把文件自述写成「**没有新逻辑**」⇒⇒ **可核对的方式正是「没有 `setState`、也没有第二套派生」**⇒⇒⇒ **⇒ 一句自述若无法被机械核对就只是姿态；这一句可以**
- ⭐⭐ 文件开头是「**哪个回调接哪个 API**」的对照表 ⇒ **形态 = 接线表 + 落地** ⇒ **B0414「依赖方向即测试性」在这里也成立**
- ⇒ ⇒⭐⭐ **而本批是 B0415 那条教训的**第一次成功应用**：**按「别只看一半」去查「刷新是不是唯一出口」⇒ 答案是「是、而且强到不需要 `setState`」** ⇒⇒ **⇒ 若不查，账上会继续写着「刷新入口未核」**
- ⇒ ⇒⭐⭐⭐⭐ **可提炼**：「**唯一出口**」有两种强度 —— **① 约定级**（大家都记得调）· **② 结构级**（**根本没有别的出口**）⇒⇒ **零个 `setState` = ②** ⇒⇒⇒⭐⭐ **⇒ 这解释了 B0415 那两次降级为什么能被静态读出来：全部状态变化都经由那一个函数、而它只做「通知」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0417 · ⭐⭐⭐ **「为什么不拆」的两类理由**
- ⭐⭐⭐ `shell_admin.dart:1-7`「**状态字段仍留在 `main.dart` 的 `_ShellRootState` 里**：**Dart 私有是库级的**，**且其中几个被 `test/transient_results_test.dart` 的源码扫描钉住**」
- ⇒ ⇒⭐⭐⭐⭐ **两条理由分属两个世界**：**① 语言约束**（`private` 是库级 ⇒ 拆成独立协作者就够不到）· **② 工具链约束**（**测试逐字扫字段名 ⇒ 拆走字段会让那条测试变红**）⇒⇒ **② 的形状是「这个字段名被一条测试的 `contains` 钉住了」**
- ⇒⇒⇒⭐ **⇒ 与 B0297「转向消费要搜 `onAck:` 而不是 `onAck`」同源：结构测试会在代码之外创造约束** ⇒⇒⇒⇒ **⇒ 而这类约束不会出现在任何设计文档里**
- ⭐⭐⭐ 而 `part of` 组合根 ⇒ **`setState` 只能是 `_ShellRootState.setState`** ⇒⇒ **⇒ B0416 的「零个 `setState`」现在有了**机制解释**（不是纪律、是结构）**
- ⭐⭐⭐ 「用 `part` + 扩展方法而不是独立协作者」被指到 `main.dart` 头注（又一次「指路」）· ⭐⭐ 「**原样搬出**」被写明（2026-09-13 rc.3 N1）⇒ **搬移是一次声明过的动作、不是渐进的漂移**
- ⇒ ⚠ **本仓有两条结构测试、两种钉法**：**方法调用**（`chat_notice_test.dart:150-167` · `contains('settleTurn(')`）· ⭐ **字段名**（`transient_results_test.dart` · **断言体未读**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0418 · ⭐⭐⭐⭐ **一条判据管当下、一条管将来**
- ⭐⭐⭐⭐⭐ `test/transient_results_test.dart:4-12`「# **被修掉的是什么**」被写成一条**用户走过的具体路径**：「测出『**失败：401**』→ 修好配置 → 切走 → 切回 ⇒ **那句失效的结论还在**，而它描述的**已经是上一套配置**了」⇒⇒ **可复现的脚本**，不是「结果可能不新鲜」这样的形容（同 B0328 的写法）
- ⭐⭐⭐⭐ **而它点名「**还有一层更细的**」**（连点后立刻切走 ⇒ **结果落在用户已离开的分区**、**看起来像是刚测的**）⇒⇒ **⇒ 「还有一层」这个措辞把「不是全部」说清了**
- ⭐⭐⭐⭐⭐ `:18-22`「# **为什么这里用源码扫描而不是 widget 测试**」单独立节：理由 ① State 私有 ② 依赖真实 HTTP ③⭐ **「成本远大于收益」**（一条判据）④⭐⭐⭐ **「还会把测试绑死在…」**（**将来会疼**）⇒⇒⇒⭐⭐ **⇒ ③ 管当下、④ 管将来**
- ⭐⭐⭐ 而 `_adminMessage` 的改名理由「**旧名字会让人误以为它属于那条已删除的链路**」⇒⇒ **⇒ 与 B0393「留这段是因为下一个人很可能再按旧名误判一次」同族**⇒⇒⇒⭐ **⇒ 而 B0393 我以为是个例，现在看是**本仓第三次**（文件名 · 字段名 · **测试手段**）⇒⇒⇒⭐⭐ **三次都是「防一个看似合理的误判」**
- ⭐⭐ 而四个字段被逐个列出 ⇒⇒ **⇒ 这就是 B0417 说的「被逐字扫的那些字段」** ⇒⇒⇒ **B0417 那条观察落地**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0419 · ⭐⭐⭐⭐ **只有「在途」才需要世代号**
- ⭐⭐⭐⭐ `test/transient_results_test.dart:81/96`「`_section =` 在 main.dart 里**只出现在 `_gotoSection` 一处**」+ `expect(writers.single, contains('_section = next'))` ⇒⇒ **「只有一条路」被机械核过** ⇒⇒⇒ **⇒ 同 B0416「零个 `setState`」的手法：让「唯一」**可被 grep 验证**
- ⭐⭐⭐ `:99`「`_gotoSection` **真的清了结果（不是只改分区）**」⇒ **测试名 = 被守的纪律**（B0379 族）· ⭐⭐⭐ `:118/141` 逐字段核 + 每字段带 `reason`（`'$flag 没有复位'`）⇒ 同 B0369 的 `reason`
- ⭐⭐⭐⭐⭐ **而第四个场景我没预期**：`:145`「**在途的**异步结果也会作废（`_resultEpoch` 对账）」⇒⇒ **不只是「切走时清空」，还要让**飞行中的响应回来时作废** ⇒⇒⇒⭐⭐ **机制是 `_resultEpoch`（世代号）⇒ 与 B0310 核的 `epoch` 是同一条机制，而「跨代作废」是它的新用途**
- ⇒ ⇒⭐⭐⭐⭐ **而本批最值钱的是第 4 点**：**「清空已有状态」与「作废在途结果」是两个不同的失效** —— **前者是显示问题、后者是「写入时机」问题**⇒⇒ **⇒ 而只有后者需要世代号**⇒⇒⇒⭐ **⇒ 于是这条测试的存在，证明了一个设计选择：清空不足以解决竞态**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0420 · ⭐⭐⭐⭐ **一个动作里做了三件事**
- ⭐⭐⭐⭐⭐ **一个真问题被提出、被就地关掉**：`if (epoch != _resultEpoch) return;` **发生在** `setState(_llmTesting = false)` **之前** ⇒⇒ 我担心「标志卡在 true」⇒⇒⇒ **但 `_gotoSection` 自己复位了它**（`:1150`）⇒⇒⇒⭐ **⇒ 不是「侥幸无害」，是同一个动作里做了两件事：作废在途 + 复位标志**
- ⭐⭐⭐⭐ 而「**静默丢弃**」被写成**目的**：「那正是 `_resultEpoch` 想要的效果」⇒⇒ **「静默」在这里是判据、不是副作用**（同 B0407「0 行入环是有意的」族）
- ⭐⭐⭐ 三件事**相邻、同一个 `setState`**：① 换分区（**显示错位**）② 推进世代（**在途写入**）③ 复位标志（**进度条卡住**）⇒⇒⇒⭐⭐⭐ **⇒ 而我今天差一点只看见 ① ⇒ 若不是追问「它会不会卡住」，我会记一条假缺陷** ⇒⇒ **「会不会卡住」是廉价的、必须问**
- ⭐⭐⭐ **自增点唯一**（`:1149`）且**就在「切分区只有一条路」那个函数里**（B0419 已核）⇒⇒ **世代推进与清空不会脱节**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0421 · ⭐⭐⭐⭐⭐ **禁令自带它的成立条件**
- ⭐⭐⭐⭐⭐ `main.dart:1145-1148`「**调用方：[_gotoSection]。不要在别处随手调** —— 它会让**进行中的**连通性自检结果被**静默丢弃**（那正是 `_resultEpoch` 想要的效果，**但只有在「用户已经离开那个分区」时才成立**）」
- ⇒ ⇒⭐⭐⭐ **⇒ 这是一条「同一动作在别的场景下就是 bug」的判据** ⇒⇒⇒ **而它不是写「别乱调」、是写「在什么条件下它才对」** ⇒⇒⇒ **⇒ 禁令的形状是「判据」不是「规矩」**
- ⭐⭐⭐⭐ 而「调用方：[_gotoSection]`」被逐字写出 ⇒ **同 B0379「唯一调用点」的写法、而这一处更强**（那处写「这里被调用」· 这处写「不要在别处调」）⇒⇒ **⇒ 「不要在别处调」是**双向**的约束**
- ⭐⭐⭐ **九个字段一次清完**（含 `epoch`）⇒⇒ **B0419 核的逐字段断言（`'$flag 没有复位'`）现在有了对象清单**；⭐ 四个 `…Failed` 标志也清 ⇒ **一次清完「结果 + 标志 + 失败态」**
- ⇒ ⇒⭐⭐⭐⭐⭐ **与 B0305「谁造成了这一轮」同家族**：**一个动作的对错取决于它发生在谁造成的场合** ⇒⇒⇒ **⇒ 而把这句话写在函数头上，下一个想复用它的人就会先问「我这里也是那个场合吗」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0422 · ⭐⭐⭐ **那个论证是三对一**
- ⭐⭐⭐⭐ `dev_tools_section.dart:1-7`「合在一个文件里：**它们共享同一套「列表 + 动作 + 内联结果」的骨架**，**拆成四个文件只会让同一段列表渲染代码出现四遍**。**每个类都短、职责单一，找起来靠类名即可**」
- ⇒ ⇒⭐⭐⭐ **⇒ 代价那一侧（「重复四遍」）对上**三处反驳**（类短 · 职责单一 · 靠类名找）⇒⇒⇒⭐⭐ **⇒ 合起来是「我知道这样不好，但三处抵消了两处」**⇒⇒⇒⇒⇒ **⇒ 同 B0393「≤1000 豁免理由」的同一动作，而这一条**连反驳那一侧都写了****
- ⭐⭐⭐ 「**列表 + 动作 + 内联结果**」这个三段骨架**被抽名了** ⇒⇒ **同 B0230 核的记忆面板** ⇒⇒⇒⭐ **⇒ 而「抽出一个名字」让它可被复用 ⇒⇒⇒ **⇒ 我第一次在本仓看到三段骨架被命名并跨文件复用**
- ⭐ `show PresetStatus, kDefault…` ⇒ **具名导入、只为了不让整个 `live2d_stage.dart` 进依赖图**（B0409 族）
- ⇒ ⇒⭐⭐ **而大多数「不拆文件」的注释只写代价那一侧** ⇒⇒⇒⭐ **⇒ 写上反驳那一侧，这个决定才可以被重新评估** ⇒⇒⇒⭐⭐ **⇒ （B0393 的「豁免理由」只写了理由那一侧 ⇒ 两条合起来才是完整形状）**
- ⚠ **待核**：import 区**顺序是乱的** ⇒ **Dart 有 `directives_ordering` lint** ⇒ **但我未核 lint 配置 ⇒ 按「无证据不记发现」只记为观察**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0423 · ⭐ **一次观察被结清为「不适用」**
- ⭐⭐⭐⭐ `analysis_options.yaml`：**唯一规则来源是 `include: package:flutter_lints/flutter.yaml`**，`linter: rules:` 下**两条都是模板自带、且都被注释掉**
- ⭐⭐⭐⭐ **而其中一条指向一个不存在的包**：`prefer_single_quotes` 属 **package:http** 的历史 lint、**已从 `flutter_lints` 移除** ⇒⇒⇒⭐⭐ **⇒ 「注释会过期」在**生成物**上同样成立**
- ⇒ ⇒⭐⭐⭐ **⇒ B0422 那条「import 顺序是乱的」因此**不适用**（`directives_ordering` 不在推荐集里）⇒⇒⇒ **⇒ 按规矩不记发现** ⇒ **这条观察到此结清**
- ⭐⭐ 排除项只有 `build/**` 与 `web/**` ⇒⇒ **⇒ 与 AGENTS「前端产物不入库」一致**
- ⭐⭐⭐⭐ **而这份配置本身零解释** ⇒⇒⇒⭐⭐ **⇒ 与 B0344 字体门禁对照：那道门禁**记得自己的来历**（生成文件 + 哈希 + 「换字体必须重新生成」），而「采用哪一套 lint」**不留痕** ⇒⇒⇒⇒ **⇒ 这不是缺陷**（推荐集是合理默认）**，但两者不对称**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0424 · ⭐⭐⭐⭐ **还没发生的码被提前给出**
- ⭐⭐⭐⭐ `dev_tools_section.dart:249-256` `if (devMode)` 包一段**开发者提示**：「导入只接受 `assets/models/` 下**已存在的目录名**；删除激活中的模型会被服务端拒绝（**409 model_active**）」
- ⭐⭐⭐⭐ **「关掉 `dev_mode` ⇒ 这一块整块不出现」**（不是 disabled、不是折叠）⇒⇒ **同 B0326 可空回调族 / B0394 `if (!librarySource)` 族**；而 B0341 核的 `_ListenButton` 在 unsupported 时是「**禁用并说明**」⇒⇒⇒ **⇒ 两种做法、两种理由**（这一块只对开发者有意义 ⇒ 不给非开发者看一条看不懂的约束）
- ⭐⭐⭐⭐⭐ **而提示本身是两条 P31**：①「已存在的目录名」= **说清「你不需要上传、需先把它放好」** ⇒ **B0372 那句的 UI 侧说法**；②「会被服务端拒绝（**409 model_active**）」⇒ **B0252 核过 409 确实会发生** ⇒⇒⇒⭐⭐ **⇒ 连还没发生的码都先给**
- ⭐⭐ **`const Padding`** ⇒ **一段没有交互的说明** ⇒ **P28 的对应形态是「不假装这段提示是按钮」**
- ⭐⭐⭐⭐ **而这一批是 B0000 ⑭ 那个「开关被自己所在分区藏起来」的**镜像** —— **这一块的存在被 `dev_mode` 正确地门控着** ⇒⇒⇒⭐⭐ **⇒⇒ 「正确门控」与「错误隐藏」的区别在于：门控的那一块**本来就该只对开发者可见****
- ⇒ ⇒⭐⭐⭐⭐ **可提炼**：**B0159 那条是「错误发生后拿界面上的码去日志里搜」；这一处是加强版——码在用户撞上它之前就给了**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0425 · ⭐⭐⭐⭐ **两个降级，只有半个理由**
- ⭐⭐⭐⭐ `shell_admin.dart:20-25` `if (mounted && **dev != _devMode**) { _devMode = dev; _refresh(); }` + `catch (_) { // **忽略：保持 false。** }`
- ⇒ ⭐⭐⭐⭐ **「读状态」与「推送变化」被分开**（`dev != _devMode` 去抖 · 每次轮询不刷新）⇒ **B0342「条件成立才给回调」的读侧**
- ⭐⭐⭐⭐⭐ **而两个降级选了不同方向、只有一半写了理由**：`/env` 失败 ⇒ **沿用旧值**（「**不该让整个面板报错**」= 讲**后果**）· `fetchStatus` 失败 ⇒ **保持 false**（「**忽略：保持 false**」= **只复述了结果**）⇒⇒⇒⭐⭐⭐ **⇒ ⇒ 后者没有回答「为什么这样选是对的」**
- ⭐⭐⭐⭐ 而「**status**」在本仓至少**三个意思**、分处三处、无统一命名：`_api.fetchStatus()`（应用状态）· `_diagApi.status()`（渲染面/模型状态）· `runtime_status`（WS 运行时）⇒⇒ **同形不同源** ⇒⇒ **分属三层架构、混淆风险低 ⇒ 按判据不记 FINDINGS，记为观察**
- ⇒ ⇒⭐⭐⭐⭐ **而这一条是 B0348「把『我们没修』与『为什么』写下来」的最小对照面** ⇒⇒⇒ **「为什么」的最低形态不是解释，是「这样选是安全的」那一句**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0426 · ⭐⭐⭐⭐ **值归服务端、文案归客户端**
- ⭐⭐⭐⭐⭐ `mods_api.dart:250-256`「**两个都认**——**只认一个的话，坏 Mod 会在界面上露出裸英文 `failed`，「出错」提示永远不出现**（**persona 轨实测抓到**）」⇒⇒ **理由不是「兼容性」、是「用户会看到裸英文」** ⇒⇒ **B0245「状态用文字」的延续**
- ⭐⭐⭐ **而它指名「服务端序列化的是 `ModStatus::as_str()`」** ⇒⇒ **差异来源被追到 Rust 侧那个函数** ⇒⇒ 又一次「指路」⇒ **同 B0411「跨语言差异应被断言守住」的族**
- ⭐⭐⭐⭐ **而括号里写着「**persona 轨实测抓到**」** ⇒⇒ **「什么时候发现的」被记下并归因到具体的轨** ⇒⇒ **同 B0418「正常路径**绝不**走到这里」那一族**
- ⭐⭐⭐⭐⭐ **而 `switch` 在客户端** ⇒⇒ **值归服务端所有、文案归客户端所有** ⇒⇒ **同 B0355「`restarted`/`enabled` 标志由服务端给、文案由前端写」完全同型** ⇒⇒⇒ **而 B0355 那时我只看成「标志+文案」，现在看清是「**值/文案分属两侧**」**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 这意味着每加一个 `ModStatus` 变体，客户端要同步改** ⇒⇒ **⇒ 而这条「同步」有测试吗 —— 未核** ⇒⇒⇒⭐⭐ **⇒ 同 B0242 核的字体门禁用 `fieldCount` 守住「新增成员必然失败」** ⇒⇒⇒⭐⭐⭐ **⇒ 同一原则在两处的落实程度：字体侧有 · Mod 状态侧未核 ⇒ 一条值得核的线索**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0427 · ⭐⭐⭐⭐ **「未知」是被测试固定的行为**
- ⭐⭐⭐⭐⭐ `test/admin_api_test.dart:186` `expect(m.statusLabel, **'weird_state'**)`（**原样透出**）· `:197` `expect(…single.statusLabel, **'未知'**)`（**映射**）⇒⇒ **线索结清：有测试，而且它固定的是「降级行为」本身**
- ⇒ ⇒⭐⭐⭐⭐ **而这是 B0242「新增成员必然失败」的**反面、且是更好的一种**：**① 编译期红**（字体 `fieldCount`）· **② 运行期显示「未知」**（Mod 状态）⇒⇒⇒⭐⭐⭐ **⇒⇒ 而 ② 的适用范围更广**（**跨语言、跨版本时不可能编译失败**）⇒⇒⇒ **⇒⇒ 而 ② 之所以能成立，前提是「**降级路径本身被断言了**」**
- ⇒⇒⇒⭐⭐⭐⭐ **⇒ 而这也修正了我 B0426 的担心**（「新增变体 ⇒ 客户端要同步改」）⇒⇒⇒⭐ **⇒⇒ 而实际上客户端**不需要**同步改也能工作、只是显示得保守**⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 而这正是 B0411「跨语言对齐应被断言守住」的**更实际的一版**：**跨语言时不可能「编译失败」，所以正确做法是「断言降级行为」**
- ⭐⭐⭐⭐ 而 `:186` 与 `:197` 的**两种兜底分工理由未读**（为什么一处透出、一处映射）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0428 · ⭐⭐⭐⭐ **判据是「谁在看」**
- ⭐⭐⭐⭐⭐ `test/admin_api_test.dart:186` `status=**weird_state**`（**有值、不认识**）⇒ `'weird_state'` + `expect(m.isRunning, **isFalse**)`；`:189`「**status 为空**」⇒ `'未知'`（**缺字段**）⇒⇒ **这不是两种风格、是两个情况**⇒⇒⇒⭐⭐⭐ **而方向恰好相反：「变了」要**看得见**（透出）· 「缺了」要**说得清**（「未知」）**
- ⇒ ⇒⭐⭐⭐⭐ **而这修正了我对 B0426 那条注释的读法**（看似矛盾、实则不矛盾）：**那条说「**已知的坏值**不该露英文」（用户看到的是 bug）**；**这里说「**未知的值**就该露原样」（开发者看到的是变化）**⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ 判据是「谁在看」**
- ⇒ ⭐⭐⭐⭐ **可迁移**：「**未知值该透出还是该映射」取决于读者** —— **给用户 ⇒ 映射成可读的话**（且映射本身要有测试钉住）· **给开发者 ⇒ 原样透出**（让变化可见）⇒⇒⇒⭐⭐ **⇒ 本仓两边都做了、且各自有测试**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0429 · ⭐⭐⭐⭐ **「为什么可以静默」是最少被写、却最重要的一句**
- ⭐⭐⭐⭐ `shell_admin.dart:31-32`「取不到 → **空表，调试面板回落显示稳定 id**」+ `:35` `if (table.length == 0) return;`⇒⇒ **降级是「换一种显示」**⇒⇒⇒⭐⭐ **「稳定 id」是承重的**（给开发者看、标签表挂掉时仍可用）⇒⇒⇒⭐⭐⭐ **⇒ 同 B0428 判据：「给谁看」决定「怎么降级」**；⭐⭐ **而主体是「调试面板」⇒ 降级到 id 是对的**
- ⭐⭐⭐⭐⭐ `:40-41`「**失败静默：保持 `null`，不误报红字；端点自己的 403 `mod_disabled` 仍是兜底真源**」⇒⇒ **「不误报」= 不把不知道说成知道**；⇒⇒⇒⭐⭐⭐⭐ **而后半句更值钱：「读不到」不是唯一真源 · 端点自己会回 403** ⇒⇒ **两个来源分工明确**（启动读一次 best-effort · 点按钮由端点现判）⇒⇒⇒⭐⭐⭐⭐ **⇒⇒ 「静默」有依据、不是省事**（B0425 那条「一半写了理由」的对照）
- ⭐⭐⭐ **而 `table.length == 0` 也当失败**（避免用空表覆盖旧表）⇒⇒ **同 B0415「`/env` 失败沿用旧值」的形状**⇒⇒⇒⭐ **⇒⇒ 第三个降级判据：空结果也算失败**
- ⇒ ⇒⭐⭐⭐⭐⭐ **可提炼**：**「为什么可以静默」是降级注释里最少被写、却最重要的一句** —— **它把「我漏了信息」与「信息在别处」区分开** ⇒⇒⇒⭐⭐ **⇒ 而「信息在别处」必须点名那个别处**（本例 `mod_disabled`）⇒⇒⇒⭐⭐⭐ **⇒⇒ 否则「静默」就与「放弃」无法区分**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0430 · ⭐⭐⭐⭐ **「翻译失败」与「判空值」是一对**
- ⭐⭐⭐⭐⭐ `preset_labels.dart:113-123` `fetchPresetLabels`：**HTTP 非 200 · 网络抛 · 解析抛** ⇒ **三种失败全部 `return PresetLabelTable.empty`** ⇒⇒ ⇒ **调用方（`_loadPresetLabels`）因此不需要 `try`、只需判 `length == 0`** ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒⇒ 这是一个「把失败翻译成值」的完整实现**（B0392 那一族、且在同一个函数里做完）
- ⭐⭐⭐⭐⭐ **而 B0429 我记的 `if (table.length == 0) return;` 不是多余的** —— 它挡的是**第三种（解析失败 ⇒ 空表）**⇒⇒⇒⭐⭐ **⇒⇒ 「空结果也算失败」在这里是有用的、不是冗余**
- ⭐⭐⭐⭐ `Uri.base.resolve('/actions/preset_labels.json')` ⇒⇒ **路径相对、base 由宿主决定** ⇒⇒ **同 B0321「主键不用 `*`」族** ⇒⇒⇒⭐⭐ **⇒ 「跟着部署走」而不是「写死一个 host」**
- ⇒ ⇒⭐⭐⭐⭐⭐ **结论**：「**把失败翻译成值**」与「**在调用方判空值**」是**一对** ⇒⇒ **只有两者都在，「失败」才不需要在每一处被处理** ⇒⇒⇒⭐⭐⭐ **⇒⇒ 而本仓两者都在** ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒ 而 B0429 我看到 `length == 0` 时只以为它防空表 ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒ 现在知道它接的是**三层失败漏下来的最后一层****
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0431 · ⭐⭐⭐⭐ **降级形态由数据形状决定**
- ⭐⭐⭐⭐⭐ `preset_labels.dart:86-98` `parse` **自己有四级保护**：`decoded is! Map`→empty · `raw is! Map`→empty · ⭐ **逐条**形状不对→跳过该条 · ⭐ **逐条**`zh` 非空否则跳过该条⇒⇒⇒⭐⭐ **「粗粒度整丢 / 细粒度跳过」的两层结构第三次出现**（①② 同 B0400 · ③④ 同 B0402）
- ⭐⭐⭐⭐⭐ **而 ④ 的判据是「`zh` 是非空字符串」** ⇒⇒ **中文标签必需、英文不是** ⇒⇒⇒⭐ **⇒⇒ 同 B0401「**不可替代的那一栏**」的判据** ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ 而 B0430 核的「调试面板回落显示稳定 id」在这里有了机制解释：「没有 `zh`」正是「只能显示 id」的那种情况 ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ 降级形态与数据缺口是**同一个判据****
- ⇒ ⇒⭐⭐⭐⭐⭐ **最值钱的是这句**：**「没有它就降级成 id」不是一条 UI 决定，而是数据层的一个判据** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 「降级形态由数据形状决定、不由界面决定」** ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ 它在解析时就决定了「这条能不能被显示成中文」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0432 · ⭐⭐⭐⭐ **禁止走私的文案版**
- ⭐⭐⭐⭐⭐ `dev_tools_section.dart:1244-1245`「**缺省空表 = 显示稳定 id（不手写第二套中文标签）**」⇒⇒⭐⭐⭐ **两套中文标签 = 两份要同步维护的真相**⇒⇒⇒⭐⭐⭐⭐ **⇒⇒ 而同步两份的失败模式是「界面显示了一个渲染面不认识的名字」**⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ 「判据集中、禁止走私」的**文案版** —— 走私的是**文案**、不是判据**
- ⭐⭐⭐⭐ **而「缺省空表」是显式的合法输入**（`:1210`）⇒⇒ **空表不是「还没加载」而是「永久没有」**⇒⇒⇒⭐⭐ **⇒⇒ 而两种在界面上**长得一样**（都是 id）⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ 「还没加载」与「表里没有」在这个控件上**不需要区分**，因为对用户的意义一样** ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒⇒ 与 B0415「降级要只降该降的一处」同族**
- ⭐⭐⭐ `clock` 也被注入（`:1248-1250`「**不在 widget 里写死 `DateTime.now()`**」）⇒⇒ **「把不可测的东西变成参数」的第三处**（timer · throttle clock · 面板时钟）
- ⭐⭐⭐⭐ `:1296-1298`「**只在 devMode 下渲染** —— off 时**整块不在语义树**」⇒⇒⭐⭐ **⇒⇒ 「不在语义树」是**可被测试断言**的说法** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 而这比「不显示」精确**（`opacity: 0` 仍在树里）
- ⚠ **留一条**：**「不在语义树」那条断言是否存在 —— 未找**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **80**（P1 6 / P2 37 / P3 37）

### BATCH-0433 · ✅ **+1 条 P3：一句没有断言支撑的自述**
- ⚠ `dev_tools_section.dart:1296-1298`「**只在 devMode 下渲染** —— off 时**整块不在语义树**」—— **今天为真**（`if (devMode)` 不构建），**但全仓没有任何断言支撑它**
- ⇒ **全仓唯一断言语义树的是** `test/audio_bar_test.dart:173-183`**（`ensureSemantics()` + `bySemanticsLabel('主音量')`·主音量滑杆）
- ⇒⇒ 按 P24「让变更必然失败」：**把 `if (devMode)` 改成 `Opacity(opacity: devMode ? 1 : 0)`（一种常见的「可见性重构」）⇒ 没有任何东西变红** ⇒⇒ **而后果是读屏用户读到整块导演观测面板（A/B/C/D 四栏 + 事件流 + TTS 文本）** ⇒⇒ **回归防护缺口、非当前缺陷 ⇒ P3**
- ⇒ ⇒⭐ **F-0433-01 (P3)** · 修法：**一条测试**（注释已把该断言的形状写好了）⇒ 与 `audio_bar_test.dart` 同一手法
- ⇒ ⇒⭐ **并与 B0418 的「可核对的自我描述」标准对照**：**这一句恰好不是那种** —— 它声称了一个可断言的事实、却没有断言
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **81**（P1 6 / P2 37 / P3 38）

### BATCH-0434 · ✅ **反证结清，范围收窄到一句话**
- ⭐⭐⭐⭐⭐ **F-0433-01 不撤回**：**没有等价断言**；而 **`devMode` 门控这个模式本身是有测试的**（`wiring_test.dart:117`「历史轮数只在 devMode 出现」）⇒⇒⇒ **修法因此是「照抄一条已存在的测试形状」** ⇒⇒ **成本比原先写的更低**
- ⭐⭐⭐⭐ `director_observer_test.dart:135/171/230/276` **直接构造那个 section、绕过门控**；该文件 `devMode: false` **零命中**⇒⇒ **这不是疏忽、是测试分层的正常结果**⇒⇒⇒⭐ **真正的缺口是「分层之后没人补一条**跨层**断言」**
- ⇒ ⇒⭐⭐⭐ **范围精确成一句话**：**「`devMode` 门控有测试（对聊天基建那块），但没有一条跨层断言覆盖『从面板进去时导演观测整块不在语义树』」** ⇒⇒ **缺口不是「忘了测」，是「分层之后没有一条跨层的断言」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **81**（P1 6 / P2 37 / P3 38）

### BATCH-0435 · ⭐⭐⭐⭐ **清单存在 · 我推翻了自己的一般化**
- ⭐⭐⭐⭐⭐ `test/wiring_test.dart:51` `group('**必须被接线的构造（点名，每个都写清后果）**')`（146 行、已守四条：`StageHost` / `shortcutHelp` / `LiveRegionThrottle` / `EpochGate`）⇒⇒ ⇒ **这一层是有系统覆盖的**⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒ B0434 那句「跨层断言缺失」不成立为普遍问题** ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ 这让 F-0433-01 的性质是「**清单少一项**」而非「缺一层测试」** ⇒⇒ **而加一行的成本极低**
- ⭐⭐⭐⭐ **而 group 名是一条标准** ⇒⇒⭐⭐⭐⭐ **四条里三条把后果写进**测试名本身**（不是另写注释）**；第四条用「**钉子 13 真的生效**」另一种理由 ⇒⇒⭐ **⇒⇒ 「每个都写清后果」在实践里执行成了「每个都写清**为什么值得测**」**
- ⭐⭐ `:89-117` 同文件里还有**行为测试**（`PersonaSection`）⇒⇒ **两层在同一处都有人写**（与 B0434 方向相反、这是好的一面）
- ⇒ ⇒⭐⭐⭐⭐ **可提炼**：**「跨层断言缺失」不是普遍问题、是漏了一个条目** ⇒⇒⇒⭐⭐⭐ **⇒⇒ 而清单式测试的失败模式是「**清单会过期**」** ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒ 同 B0344「门禁防的是过期合规」**同一形状** ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ 而那份清单恰好把自己写成了「点名 + 每个写清后果」⇒⇒⇒⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ 「漏一个」在这种清单里是**可见的****
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **81**（P1 6 / P2 37 / P3 38）

### BATCH-0436 · ✅ **+1 条 P3：清单里多了一个不存在的名字**
- ⭐⭐⭐⭐ `test/wiring_test.dart:77` `test('**`EpochGate` 的判据被 `ChatController` 用上**（钉子 13 真的生效）')` + 两条 `referencedOutside('mustInterruptAudio(' / 'decideAudioFrame(', 'lib/audio/epoch_gate.dart')`
- ⭐⭐⭐ 而 **`EpochGate` 全仓只出现在这一行测试名里**（`grep -rn EpochGate --include=*.dart .`）⇒⇒ **它不是一个存在的类型名**；而 `lib/audio/epoch_gate.dart` 存在
- ⭐⭐⭐⭐⭐ **⇒ 测试名承诺的强于断言给出的**：名字说「**被 `ChatController` 用上**」· 断言查的是「**被文件外引用**」⇒⇒ **任何别的文件引用它们都通过**⇒⇒ **而这条测试不是废的**（它守「判据不被内联回调用方」）⇒⇒ **问题是名字与断言的落差 ⇒ P3**
- ⇒ ⇒⭐⭐⭐ **另一层（B0435 说的「清单会过期」在同一条清单上的第一次实证）**：一个承诺「**每个都写清后果**」的清单里，它的第一个条目用了一个**仓库里没有的名字** ⇒⇒ **这不是「漏一项」，是「多了一个不存在的项」**
- ⇒ ⇒⭐ **F-0436-01 (P3)** · 修法**各一行**：**改名**成它真正断言的那句 · **或**补第三条断言把 `ChatController` 点名；并把 `EpochGate` 换成文件/函数名
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **82**（P1 6 / P2 37 / P3 39）

### BATCH-0437 · ✅ **反证结清**（F-0436-01）
- ⭐⭐⭐⭐⭐ `test/wiring_test.dart:38-49` `bool referencedOutside(String symbol, String ownFile)` —— **只有两个参数、不接受「调用方」** ⇒⇒⇒⭐⭐⭐ **⇒⇒ 「被 `ChatController` 用上」确实没有断言支撑** ⇒⇒ 「补第三条断言」成立、「删多余参数」**不成立**
- ⭐⭐⭐⭐ **而它是 `contains(字符串)`、不解析符号 ⇒ 注释里提到也算命中** ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ 同仓另一条扫源码字符串的测试（`visual_language_test`·B0381 已核）**刻意排除注释** ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 「同一种手法在同一个仓里被用成两种强度」，而弱的那一种恰好写在一条承诺更强的测试名下** ⇒⇒ **一行注释就能让 `:77` 变绿**
- ⭐⭐⭐⭐ **而 `!f.path.endsWith(ownFile)` 正确排除了自己那个文件 ⇒ 「在别处被引用」被正确检查** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒ 「意图对、强度弱」—— 这就是 F-0436-01 的准确性质**（不是「测试无效」）· ⭐ 扫 `lib/` 不含 `test/` ⇒ 与意图一致
- ⇒ ⇒ **F-0436-01 仍 P3、不升级**（当前无缺陷）**但新增一层落差**：「被文件外引用」**可被一行注释满足**、而名字说的是「被 `ChatController` 用上」⇒⇒ **修法相应地是两处：排除注释 + 改名或点名调用方**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **82**（P1 6 / P2 37 / P3 39）

### BATCH-0438 · ⭐⭐⭐ **30 个落点 · 只记分母、不下结论**
- ⭐⭐⭐⭐ `test/` 里**扫源码字符串的测试有 30 个文件**（`wiring_test.dart` 146 行 … `display_prefs_test.dart` 892 行）⇒⇒ **⇒ 「结构测试」在这个仓是一个有规模的层**
- ⇒ ⇒⭐⭐⭐ **我已核注释处理强度的只有 2 处**、**其余 28 未核** ⇒⇒⇒⭐ **⇒⇒ 「两处强度不同」是一个观察、而「30 处强度相同」不是结论** ⇒⇒ **按我自己的规矩：不出结论、只记分母**
- ⇒ ⇒⭐⭐⭐ **而「是否排除注释 / 是否指名调用方」这两条差异、在任何文件里都没有被声明** ⇒⇒⇒⭐⭐ **⇒⇒ `visual_language_test` 是唯一一处把它做成「反向对照」的**（B0381）⇒⇒⇒⭐⭐ **⇒⇒ 而 `wiring_test:77` 本可以照抄 B0381 的做法、而没有** ⇒⇒ **F-0436-01 的修法因此有同层先例可循**
- ⇒ ⇒⭐⭐ **⇒⇒ 换句话说：这一层的**默认强度是「不排除注释、不指名调用方」，唯一的例外没有被复用到同层****
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **82**（P1 6 / P2 37 / P3 39）

### BATCH-0439 · ⭐⭐⭐ **自我证明有两种形态，两处都有**
- ⭐⭐⭐⭐⭐ `test/stage_overlay_single_test.dart`（135 行）**也有自我证明**、且**形态不同**：**真 widget + 存在/不存在成对**（`:109-110` `find.textContaining('加载中') findsNothing`）vs `visual_language_test` 的**扫源码 + 合成样例**⇒⇒⇒⭐⭐⭐⭐ **⇒ 「这条纪律在本仓有两种形态、都存在」** ⇒⇒⇒ **⇒ 这比 B0438 的「唯一一处」更准确 —— 那一句在此被修正**
- ⭐⭐⭐⭐ 而 `:109-110` 是「唯一性」的**正反面**（「恰好一个」+「另一个一个都没有」）⇒⇒ **比只数正面更严**
- ⭐⭐⭐⭐⭐ **而 `:124` 的 `reason` 承认了这条测试的脆弱性**：「找不到 `_StageBadge`，**测试需要跟着改**」⇒⇒⇒⭐⭐⭐ **⇒ 这是「自我证明」的另一种写法** ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ 它不说「我不会坏」、它说「我坏了会这样说」** ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ 「测试会随重构一起坏」是结构测试的固有属性、而本仓把它写进 `reason`**
- ⭐⭐⭐ `:35`「把**真的** `Live2DStage` 放进 `StageHost`（**这正是 `main.dart` 的接法**）」⇒⇒ **「被谁用」被显式指出** ⇒⇒ **同 B0417 族**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **82**（P1 6 / P2 37 / P3 39）

### BATCH-0440 · ⭐⭐⭐ **第三种形态最强** · 而本批**挑错了一次文件**
- ⚠⚠ **过程注记（挑错文件）**：按文件名挑了 `session_baseline_test.dart` ⇒ 得到的是 **`ActionCue` 相关断言**、与 B0243 核的「历史裁剪」无关 ⇒⇒⭐⭐⭐ **⇒ 而这次不是「零命中」、是「有命中但主题不同」⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒ 「文件名可以指向另一个主题」** ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ 「找到了 ≠ 是它」**（B0316 那条规矩要**扩写**）
- ⭐⭐⭐⭐⭐ **第三种自我证明形态、而它最强**：`chat_session_test.dart:207-211` + `keyboard_test.dart:158` 两个文件**各钉一个数值**（都是 **500**）⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ **它防的不是「测试本身失效」、是「两处各自都对、但对不上」**
- ⭐⭐⭐⭐ 而测试名把**不一致的后果**写出来了：「**两处不一致会出现「看得见但重开就没了」**」⇒⇒ **用户可感知的具体症状**，不是「参数不一致」这样的抽象说法
- ⭐⭐⭐⭐⭐ **而「不 import、改为钉住数值」把**理由**（**会把 Flutter 拖进纯逻辑测试**）与**代价**（**逼着人同时看两处**）都写明了** ⇒⇒⇒⭐⭐ **「知道代价是什么、还是选它」** ⇒⇒ **同 B0307 那个自觉**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 这是 B0411「跨语言对齐必须被断言守住」的**仓内版** ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 「同一个原则，跨语言与跨文件各有一个实例」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **82**（P1 6 / P2 37 / P3 39）

### BATCH-0441 · ⭐⭐⭐⭐ **「排除注释」是 helper 级 · 但没成为默认**
- ⭐⭐⭐⭐⭐ `test/background_hydration_window_test.dart:228-252`：注释先说**为什么只能扫源码**（`main.dart` 依赖 `package:web`、VM 加载不了）· 再说**怎么扫**（「**调用形状**、**先剥掉注释与字符串**」）⇒ `stripCommentsAndStrings(src)` + `expect(src.contains('MemoryBackgroundStore('), **isFalse**, reason: '**代码路径，不是注释里提一句**')`
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 「排除注释」在这个仓是一个**具名 helper**、不是每个测试各写一遍** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ B0437 那句「弱的那种写在承诺更强的测试名下」有了正面解法：⇒⇒⇒⭐⭐⭐⭐⭐ **「强的那种不是没被写，是**没有成为默认**」**
- ⭐⭐⭐⭐⭐ **而「因为它依赖 `package:web`」这个理由，同时是「它只能被扫」的原因和「它扫不到行为」的原因** ⇒⇒ **一条完整的因果链**（同 B0409「依赖方向即测试性」）
- ⭐⭐⭐⭐ **⇒ 「不留占位」那条裁决（B0300 / AGENTS v0.5.0）有守卫** = 「**反向断言（`isFalse`）+ `reason`**」⇒⇒ **⇒ 同 B0342「禁用在输入层生效」的测试侧形态**
- ⇒ ⇒⭐⭐⭐⭐ **四个实例的自我证明形态至此**：①扫源码+合成样例 ②真 widget+`findsNothing` ③跨文件数值 pin ④⭐**扫源码+`stripCommentsAndStrings`+`isFalse`+`reason`（唯一一个把「排除注释」做成 helper 级默认的）** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒⇒⇒⇒ ⇒ **而 `referencedOutside` 与它同层却没用那个 helper ⇒ 这就是「有正解、但没推广」的可核形状**
- ⇒ ⇒⭐⭐ **F-0436-01 的**首选修法**因此是「让 `referencedOutside` 也走那个 helper」**（而不是新造反向对照）⇒ **同层两个先例可抄**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **82**（P1 6 / P2 37 / P3 39）

### BATCH-0442 · ⭐⭐⭐⭐ **「没推广」是错的 · 正确做法以复制的形式推广了**
- ⚠ **工具层事故（第二次「引号吃掉管道」）**：嵌套 `bash -c` 里正则用双引号包住 ⇒ `DENOM: 0` ⇒⇒ **按规矩那个 0 不用**、分母沿用 B0438 已核的 **30**
- ⭐⭐⭐⭐⭐ **`stripCommentsAndStrings` 的机械计数**：**8 份本地定义**（其中 `design_tokens_test.dart` 是它的家）+ **3 处从它 import** ⇒⇒ **11 个文件在用** / 分母 30 ⇒⇒ **19 个既没定义也没 import** ⇒⇒ **`wiring_test.dart` 属于那 19 个**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 「没推广」是错的** ⇒⇒ **正确做法在本仓推广开了 —— 11 个文件在用** ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒ 「强的那种**不是没被写、是**以复制的形式推广**」** ⇒⇒⭐⭐ **⇒⇒ 而 8 个是各自定义、只有 3 个从同一个家 import** ⇒⇒ **⇒⇒⇒ 「同一个 helper 被复制了 8 份」**
- ⭐⭐⭐⭐⭐ **⇒ 而它的家是 `design_tokens_test.dart`** ⇒⇒⭐⭐⭐ **⇒⇒ 一个 11 个文件在用的 helper，住在名叫 `design_tokens_test` 的文件里** ⇒⇒ **同 B0393「名实相符」/ B0421「一个动作放一个文件」同族 ⇒⇒ **「归入某处、供 11 个文件用」是一个落点**
- ⇒ ⇒⭐⭐⭐⭐ **⇒⇒ ⇒ 而 B0441「helper 级」要修正为：「helper 的实现是既有事实（8 份副本 + 3 处共享），而 `wiring_test` 属于那 19 个」** ⇒⇒ **⇒⇒⇒ 这比「没推广」更有用：不是「没推广」，是「**11 比 19**」**
- ⇒ ⇒⭐⭐⭐⭐ **⇒⇒⇒ 而这正好是 B0435「计数不是判据」的**反面**：**这里计数恰好是对的** —— 因为要回答的是「有几个在用」，而那**就是一个计数问题****
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **82**（P1 6 / P2 37 / P3 39）

### BATCH-0443 · ✅ **+1 条 P3：8 份逐字节相同的副本**
- ⭐⭐⭐⭐⭐ 逐个取定义起 15 行、去空行、`md5sum` ⇒ **8 个文件的 `stripCommentsAndStrings` 长度均 467、md5 均 `96c4bad6`** ⇒⇒⇒ **⇒ 八份逐字节相同**；而**另有 3 个文件已从 `design_tokens_test.dart` import 它**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 「8 份副本」+「3 处共享」= 同一层里共享这条路已被走过 3 次** ⇒⇒⇒⭐⭐⭐ **⇒⇒ 准确的判断是「它不是不能共享，是没有成为默认」**（收窄成一句话的事实）
- ⇒ ⇒⭐⭐ **⇒ 而 P25（「收敛若引入层间倒挂则重复更便宜」）在这里不成立**：**同层共享已被做出来 3 次、不存在「层间倒挂」这一免责理由**
- ⇒ ⇒⭐ **F-0443-01 (P3)** · 修法：8 份本地定义换成**一次 import**（与那 3 处同款）+ 把 helper 归到**与职责相符**的位置（顺带解决「11 个文件在用的东西住在 `design_tokens_test.dart` 里」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **83**（P1 6 / P2 37 / P3 40）

### BATCH-0444 · ⭐⭐⭐⭐ **「不落盘」是关于本栏持久化的判断**
- ⭐⭐⭐⭐⭐ `director_observer_section.dart:21`「D 送 TTS 文本 | 左 = 前端已收 `text_delta`；右 = **日志端点** `sentence_ready`」+ `:23`「**观测缓冲只住内存**：不写本机存储、不落盘（**本栏红线**）」+ `:159` 右列数据口 = `DiagnosticsApi().logs()`
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 「不记录请求体」那条没有被违反** —— `sentence_ready` 记的是**送进 TTS 的文本**、而那**不是请求体** ⇒⇒⇒ **⇒ 而 D 栏两个数据口内容类别不同**：**左 = 前端自己收的帧（内存）· 右 = 服务端日志**
- ⭐⭐⭐⭐⭐ **⇒ 「不落盘」这条红线管的是「本栏自己的持久化」** ⇒⇒⇒⭐⭐⭐⭐ **⇒ 「不落盘」与「服务端已经记了」是两件事** ⇒⇒ **⇒⇒ 「本栏红线」这四个字的作用就是把这两件事分开**
- ⭐⭐⭐ 而 `:72`「进程内唯一实例（**前端不落盘，故不需要持久层**）」把两者连了起来 ⇒⇒ **⇒⇒ 「不需要持久层」是「不落盘」的**推论**、不是重复**
- ⭐⭐⭐ **同一条红线在三处被说**（`:23` 文件头 · `:72` 单例 · `:287` 栏副标）⇒⇒ **同 B0386「一条纪律在三处被独立说出」同族**；而**落点最好认：每一层的人只需看自己那层**
- ⇒ ⇒ **本批的收获**：**「不落盘」是关于本栏持久化的判断、不是关于「内容是否离开过浏览器」的判断** ⇒⇒⇒ **⇒ 这正是本仓「两处各管一层」的常见写法**（B0425「值/文案分属两侧」· B0412「两级跳过/两级整丢」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **83**（P1 6 / P2 37 / P3 40）

### BATCH-0445 · ⭐⭐ **声明写在使用点上**
- ⭐⭐⭐ **⇒ 「两个数据口」成立**：**左列 = 前端自己收的 WS 帧**（经 `ObserverBuffer`、**环形上限 200**·`:19`）· **右列 = `DiagnosticsApi().logs()`**（**服务端日志**·`:159`）⇒⇒ **⇒ 不是同一个数据源、内容类别也不同** ⇒⇒ **B0444 的「内容类别不同」有代码支撑**
- ⭐⭐⭐⭐⭐ **⇒ 而「只取 `sentence_ready`」这个声明就写在数据口上面**：`:634` 的小标题 = 「**原始段（日志端点 sentence_ready）**」⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 「这句话说的过滤」与「这句话被显示的地方」相邻** ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒ 「一句容易被误会的话，紧挨着它被兑现的位置」**
- ⭐⭐⭐ **⇒ 而三态也在数据口下面**（`:635-637` **读取中 · 错误 · 内容**）⇒⇒ **同 B0430「三个态由同一处表达」** ⇒⇒ **「三个态、三个控件」是最容易漏一个的形状、而这里没有漏**
- ⇒ ⇒⭐⭐⭐⭐ **本批最值钱的是第 2 点**：**把声明写在使用点上**的收益不是「多写一遍」而是「**多一个不误读的入口**」⇒⇒⇒ **⇒ 同 B0321「把没用的参数删掉而不是留着」同一个道理：留着的理由 = 少一个不误读的入口**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 而本批把 B0444 那条收获补上了代码层**：**「不落盘」是关于本栏持久化的判断、不是关于「内容是否离开过浏览器」的判断** ⇒⇒ **而右列恰好证明它：本栏不落盘、而内容确实从服务端日志来过 —— 两句话同时成立**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **83**（P1 6 / P2 37 / P3 40）

### BATCH-0446 · ✅ **+1 条 P2：标题在替一个不存在的过滤背书**
- ⭐⭐⭐⭐⭐ `director_observer_section.dart` **三处说法与实现不一致**：表头 `:21`「右 = **日志端点 sentence_ready**」· 小标题 `:634`「**原始段（日志端点 sentence_ready）**」· 而**代码 `:210-213` 是 `lines.map((l) => l.asText)` —— 全部、零过滤**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ ⇒ 标题在替一个不存在的过滤背书** ⇒⇒ 用户与后来的读代码者都会以为右列只有 `sentence_ready`，而它显示**全部日志行** ⇒⇒ **而这一列是 dev_mode 下唯一的「D 送 TTS 文本」对照面**（B0433 核过这一块整块只在 devMode 下渲染）
- ⭐⭐⭐⭐ **⇒ 而过滤只能做在客户端**：**B0321 已核 `?limit=` 服务端不读** ⇒⇒ **⇒⇒ 「按 `sentence_ready` 过滤」这条路存在、只是没写** ⇒⇒ **缺口是**实现**缺口、不是能力缺口**
- ⇒ ⇒ **F-0446-01 (P2)** · 修法**同一处二选一**：**按标题兑现**（客户端 `.where`）**或按代码改标题**（「服务端日志（全部行）」）⇒⇒ **且建议把「这条列是不是全量」写进 `DiagnosticsApi.logs()` 的文档** —— **那个方法刚被 B0321 改过、而它的文档没记这件事**
- ⚠ **软肋（已写进反证条款）**：**未读 `DiagnosticsApi.logs()` 与 `LogLine` 的定义** ⇒⇒ **若 `logs()` 本身只含 `sentence_ready`，本条不成立** ⇒⇒ **而这是可一读定论的** ⇒⇒ **下一批第一件事**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0447 · ✅ **F-0446-01 定论成立 · 根更靠下**
- ⭐⭐⭐⭐⭐ `diagnostics_api.dart` `logs()` 取 `['lines'] ?? ['logs']` ⇒ **逐条 `fromJson`、零 `where`**；`LogLine`（`:58-70`）只有 **`level`/`target`/`message`** + `asText` ⇒⇒ **零过滤已定论**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 「`LogLine` 没有 `type` 字段」比「`logs()` 没过滤」更根本** ⇒⇒⭐⭐⭐ **⇒⇒ ⇒⇒ 要按 `sentence_ready` 过滤，**必须先让 `LogLine` 带上一个区分日志种类的字段** ⇒⇒ **⇒⇒⇒ ⇒⇒ 「这不是加一个 `where` 就完的事」**（而服务端 `MAX_LINES` 是按行数、不是按类型）
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **⇒ 本条建议改写**：**首选（零协议成本）把两处标题改成它实际显示的东西**；若真要过滤则**三步**（服务端加字段 + 客户端带上 + `where`）⇒⇒ **⇒⇒ 「一行文案」是唯一不需要动协议的落点**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0448 · ✅ **一个观察、不是发现** · 而方法论值得留
- ⭐⭐⭐⭐ `logs()` 的 `['lines'] ?? ['logs']` **第二个分支从未被走到**（全 `test/` 零命中造出 `logs` 键的 payload）⇒⇒ **「两种键名都认」没有测试在守**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 判它是观察、不是发现的理由**：「用户可察觉的后果」**没有**（两键名都工作时界面上无差别）·「两条真相会分叉」**可能**、而分叉方向正是 `??` 要挡的 · 修法**一行**⇒⇒ **⇒ 「一行成本」不足以抬到 P3** ⇒⇒ **⇒ 而它就在 F-0446-01 的同一段代码里 ⇒⇒ 记为「顺手可做」**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 方法论（本批最值钱）**：**「一个分支没被测到」不是缺陷** ⇒⇒ **它是否缺陷取决于「走到它与走不到它，界面上有没有差别」** ⇒⇒⇒⭐⭐ **「分支覆盖率」是指标、而「两条路径是否等价」才是判据** ⇒⇒ **本仓的 `??` 兜底是等价的两条路、所以覆盖率低没有害**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0449 · ⭐⭐⭐⭐ **「空」是四个条件的合取**
- ⭐⭐⭐⭐⭐ `message_bubble.dart:78-86`「三个**例外**都照旧显示（各有 UI 语义，**别跟着一起删**）：① 流式中（要有「…」占位，**表示还在等**）② 失败轮（危险色 + 「重试」，**用户唯一的恢复入口**）③ 只有思考（**思考就是本轮唯一内容**）」+ `if (text.trim().isEmpty && !streaming && !failed && !onlyReasoning) return SizedBox.shrink();`
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 「别跟着一起删」的形状 = 给每一项配一句「它凭什么在」** ⇒⇒ **改起来是「加一行说明」而不是「加一个判断」** ⇒⇒ **它防的是「有人认为空就该不显示、于是把三个一起删」**
- ⭐⭐⭐⭐ **⇒ 而「空 ⇒ 不显示」这个默认值是对的**（B0300 的「清掉没有功能的占位」）⇒⇒ **而三个例外是「不空」的三种真实情况**
- ⭐⭐⭐⭐⭐⭐⭐ **⇒ 「空」不是一个状态、是四个条件的合取** ⇒⇒ **⇒ 「空轮有**四种表现**（不显示 / … / 失败重试 / 思考），由**四个条件正交**地分出来」**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ B0448 那条判据给出了正面用法**：**先确认「不空」的四条路径在界面上是否等价** ⇒⇒ **答案是「不等价且各有语义」**⇒⇒ **⇒⇒ 「不等价」是这三行注释的理由、而 B0448 的判据是问这句话的方法**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0450 · ⭐⭐⭐⭐ **「发送不上报 → 上报不猜 → 宿主拿真值」三段齐**
- ⭐⭐⭐⭐⭐ `live2d_stage.dart:313-318`「新回执 → 上报宿主（**缩放百分比要从渲染面的真值来，不能猜**）」+ `if (ack != null && !identical(ack, _lastAck)) { … widget.onAck?.call(ack); }`
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ B0000 ⑧ 那个缺陷（「缩放读数永远是 `—`」·**没人读首帧 `stage-ack`**）在这一侧被**同一个理由**修掉** ⇒⇒⇒⭐⭐ **⇒⇒ 「B0000 ⑧ 的**因果**与**修法**被同一句话连起来了」** ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒ 「发送不上报（B0378）→ 上报不猜（本批）→ 宿主拿真值显示」：三段都在同一个文件里**
- ⭐⭐⭐⭐ **⇒ 而去重是 `!identical(...)`** ⇒⇒ **「同一对象」判等** ⇒⇒ **「新回执才上报」靠比较引用、不靠比较内容**（同 B0263 `presetStatusUpdateFor` 族）
- ⭐⭐⭐⭐ **⇒ 而 `onReady` 的 `_readyNotified` 闩写着「离开 ready 后允许再次通知，**覆盖 retry/重建**」** ⇒⇒⭐⭐⭐ **⇒⇒ 与 B0322「渲染面错误/加载态都会在重建后再发一次」是同一现象的两次记录** ⇒⇒ **两处各说一次、而它们两处独立**
- ⭐⭐ **⇒ 而 `setState(() {})` 在最后** ⇒⇒ **本方法每次都触发一次重建**（同 B0290 `_attach` 每次重放族）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0451 · ⭐⭐⭐ **F-0387-01 的前提证成、性质收紧**
- ⭐⭐⭐⭐⭐ `lib/settings/mods/voice_input_panel.dart:77` 有 `'wake_phrase_set': '唤醒词已自定义'`；`dev_tools_section.dart:706/712/715` 的 spec 驱动表单**同时处理 `string` 与 `bool`** ⇒⇒ **两个闸门字段各有一个输入框** ⇒⇒⭐⭐⭐ **「面板里有、文档表里没有」成立**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 性质收紧：不是「功能缺失」、是「索引与实现不同步」** ⇒⇒ **「功能在、索引缺」比「功能缺」轻得多** ⇒⇒ **而修法（补两行表格）正好对应这个性质**
- ⭐⭐⭐⭐ **⇒ 而 `voice_input_panel.dart:77` 是 B0426「值归服务端、文案归客户端」的**第五次出现**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 而这一批是「反证条款用上、结果为真」的第二例**（第一例 B0412：**反证不成立、发现被加强**）⇒⇒ **两种结果都算「条款被用了」** ⇒⇒⭐⭐⭐ **⇒⇒ 「被用了」比「结论对我有利」更重要**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0452 · ⭐⭐⭐⭐ **可空的三个条件，三条都在**
- ⭐⭐⭐⭐⭐ `mods_api.dart:156-170` `ModConfigResult`：`ok` · `restarted = false`（**有缺省**）· `enabled`（**`bool?`、无缺省**）+ 文档「**`null` = 服务端没回这个键**」⇒⇒ **三个字段 = 界面需要的三个答案**（与 B0355 的 `_okMessage` 逐条对上）
- ⭐⭐⭐⭐⭐ **⇒ 「可空」说清了的三个条件，本仓三条都在**：① 真的是 `bool?` ② **文档写出 `null` 的含义** ③ **调用方按那个含义分支**（**B0355 的 `enabled == false` 显式比 `false`**）
- ⭐⭐⭐⭐⭐ **⇒ 而 `!enabled` 与 `enabled == false` 的差别在这里是实质的**：写成 `!enabled` ⇒ `null` 被当成 `true`，**而写它的人看不出自己错在哪** ⇒⇒ **「显式比 `false`」是三值语义逼出来的、不是风格偏好**
- ⭐⭐⭐ **⇒ 而 `restarted = false` 有缺省、`enabled` 没有** ⇒⇒ **「默认 false 是常态」+ 「默认无（要问服务端）」两种缺省被区分开了**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 本批最值钱的是第 2 点**：**一个可空字段要同时满足三个条件才「说清了」** ⇒⇒ **B0349 我只记了「可空 = 有一个可推导的默认」—— 这一批补的是「可空的三个条件」** ⇒⇒ **「`null` 的含义」被写下来不是注释的礼貌，而是调用方分支的前提**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0453 · ⭐⭐⭐⭐⭐ **「缺失」不能落到一个有意义的值上**
- ⭐⭐⭐⭐⭐ `settings_models.dart:134-145`：`isMaxTokensUnlimited => maxTokens == 0`（**0 是有意义的值**）· `fromJson(Map<String,Object?>? json)`（**参数可空**）+ `json ?? const {}`（**归一**）· 注释「**服务端缺失该字段（旧服务端）→ 回落默认；回落成 0 语义正好相反**」· `j.**containsKey**('max_tokens') ? _int(...) : defaultMaxTokens`
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 「键在不在」与「值是什么」被分开问了** ⇒⇒ **而这在 `bool?` 那处是做不到的**（只能问「是 null 吗」）⇒⇒ **「`containsKey` 比 `== null` 更能表达『键在不在』—— 而这是表结构与对象的差别**
- ⭐⭐⭐⭐⭐⭐ **⇒ 「0 在这一栏是一个有意义的值」⇒「缺失」不能落到 0 上** ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒⇒⇒ ⇒ 「0 是一个有意义的值」⇒「缺失」就不能落到 0 上 —— 一条可推广的判据**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ B0452 那三个条件升级成四种落法**：① `bool?` + 文档写 `null` 含义 ② 参数可空 + 归一 ③ **`containsKey` 先问「键在不在」** ④ 回落值与真实值语义相反时必须用 ③
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 判据**：**给一栏选回落值之前，先问「它的取值集合里有没有哪个值本身就有意义」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0454 · ⭐⭐⭐⭐ **同一条纪律的两面**
- ⭐⭐⭐⭐⭐ `settings_models.dart:1-21` 头注：「**契约来源（读源码确认，不是猜的）**」+ 点名 `settings/view.rs` · `settings/patch.rs` · 「**是 PATCH 不是 PUT**（**实测 `PUT→404` / `PATCH→200`**）」
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 「是 PATCH 不是 PUT」是极易猜错的细节**（REST 直觉会说 PUT）⇒⇒ **它被写下来的理由是「猜错的人看不出来」—— 而猜错的后果被写出来了（404）**
- ⭐⭐⭐⭐⭐ **⇒ 而「三态」的服务端对应物被点名**：`Option<Option<T>>`（`patch.rs` 的 `LlmPatch`）⇒⇒ **前后端用同一个模型表达三态**
- ⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「显式清空」= `"field": null`** ⇒ **这恰恰是 B0453 那条判据要保护的那一格** ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒ 同一条纪律的两面：「一个值不能同时表示『没有』与『某状态』」** —— **B0453**：`0` 不能表示「缺失」与「无限」· **本批**：`null` 表示「显式清空」而「不修改」是**键不出现**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 这就是 B0349「可空 = 有一个可推导的默认」的**前提**：**可空要三态、是因为两种「没有」必须分开**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0455 · ⭐⭐⭐⭐ **「键不出现」被两个来源共用，而这是对的**
- ⭐⭐⭐⭐⭐ `settings_models.dart:75-85` `putTri<T>`：`case null:` 与 `case TriKeep<T>():` **共享 `break`（键不出现）** · `TriClear` ⇒ `out[key] = null` · `TriSet` ⇒ `out[key] = value`
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 从服务端看，「不修改」与「保持」本来就**无法也不必**区分** ⇒⇒⇒ **⇒ 它的正确性依赖服务端把「键不出现」也当作保持** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 而这澄清了 B0454 那句话的适用范围：「两种『没有』必须分开」是针对 wire 形状的、而 `null`/不出现那一侧是**第三态**、不是第四态**
- ⭐⭐⭐⭐⭐ **⇒ 而 `putTri` 是顶层泛型函数、`Tri<T>` 是封闭类型 ⇒ 三态的编码只有一处**；⭐⭐⭐⭐ 每一类 patch 只调 `putTri` ⇒⇒ **「三态怎么上线路」不在任何一个 patch 类里** ⇒⇒ **同 B0335「判据只被派生一次」在序列化那一侧** ⇒⇒ 「每个 patch 各写一遍 switch ⇒ N 份」
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0456 · ⭐⭐⭐⭐ **「构造方式」与「wire 形状」不可能对不上**
- ⭐⭐⭐⭐⭐ `settings_models.dart` `sealed class Tri<T>`：三个静态工厂（`keep` / `clear` / `set`）**各自的文档直接写出它在 wire 上是什么**（`键不出现` / `序列化为 null` / `设为该值`）
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 三层各一张表、而第三层被逐项标注**：① **wire 形状**（`:18-21` 头注表）② **状态**（同表左列）③ **Dart 构造**（`putTri` 的 `switch`·B0455）⇒⇒ **一张跨语言、跨层的表被分三处各写一遍** ⇒⇒⭐⭐⭐ **三处形状相同、粒度不同**
- ⭐⭐⭐⭐⭐ **⇒ 而「构造方式」与「wire 形状」不可能对不上** —— **它们都被同一个 `sealed` 类型穷举** ⇒⇒ **而 B0455 的 `switch` 有「显式三态 + `default`」四支** ⇒⇒⇒⭐⭐⭐⭐⭐
- ⭐⭐⭐⭐ **⇒ 而这套「两张表 + 一道穷举」补全了 B0453 那条判据**：**当一个语义有多种编码时，要**把编码穷举出来**、并**让编译器替你数**** ⇒⇒⇒⭐⭐⭐⭐⭐
- ⭐⭐⭐ **⇒ 而工厂名直接用中文语义词**（`keep`/`clear`/`set`，**不用** `unchanged`/`reset`/`assign`）⇒⇒⭐⭐⭐ **三处一致**：类型名 · 工厂名 · 注释（`不修改`/`显式清空`/`设为`）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0457 · ⭐⭐⭐⭐ **一个怕「没话说」、一个怕「说废话」**
- ⭐⭐⭐⭐⭐ `settings_models.dart:781-785`「`note`：**成功但有话要说**（例如『上游可达但未提供 /models』）。**只有真有解释价值时才非空**——服务端**刻意不在成功路径上默认塞一个**，**否则每次自检都多一句噪音，用户很快就不看它了**」
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 「只有真有解释价值时才非空」是一条判据、不是一个描述** ⇒⇒ **判据的形状是「这个字段的**填充条件**是什么」** ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒ 而它同时给了反面后果** ⇒⇒ **字段留空被当成一个「会被用户读到的界面元素」来权衡**
- ⭐⭐⭐⭐⭐⭐⭐ **⇒ 而这与 B0310「绝不做『什么都没显示』」互为镜像** ⇒⇒ **⇒ 一个怕「没话说」· 一个怕「说废话」**
- ⭐⭐⭐⭐ **⇒ 而 `note` 与 `errorCode`/`errorMessage` 是两组**（成功/失败语义不同）· ⭐⭐⭐⭐ 整个类型 `?` 密集（五个可空字段、**每个各有一句「什么时候非空」**）
- ⭐⭐ **⇒ 而 `latencyMs` 的文档记了一个类型宽度差异**（「服务端用 `u128`，这里收成 `int`」）⇒⇒ **而这也让 B0316 那条 `latencyMs` 可以为 `null` 而不歧义**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0458 · ✅ **兑现成立 · B0457 的「互为镜像」收窄**
- ⭐⭐⭐⭐⭐ `main.dart:1046-1050` 成功分支**三个可空字段都被读**（`latencyMs` · `modelEcho` · `note`）⇒⇒ **每个都有独立的 `??`/`isEmpty` 判据** ⇒⇒⭐⭐⭐⭐ **⇒ 「只有真有解释价值时才非空」兑现为「只有非空才显示」**
- ⭐⭐⭐⭐⭐⭐ **⇒ B0457 那个「互为镜像」是成对写出来的**：**B0310 怕「没话说」**（不许 `SizedBox.shrink()`）· **本批怕「说废话」**（`o.note` 空就不显示）⇒⇒ **两句都实现了**
- ⭐⭐⭐⭐ **⇒ 而 `o.errorMessage ?? o.errorCode ?? '未知原因'` 是三层回落、顺序是**可读性优先**（消息优先于码、而码不会丢）
- ⭐⭐ **⇒ 而 `on ApiException` 分支只写 `e.message`、不写 `e.code`** ⇒⇒ **⇒ 本批唯一的「不对称」**；按 B0295 核过的 `_guard` 语义（**只有 HTTP 层失败才抛**）这**可能是合理解释**（传输层失败没有码）⇒⇒ **⚠ 但我未核 `ApiException` 有没有 `code` ⇒ 按 B0271 记为待核、不记发现**
- ⭐⭐⭐⭐⭐ **⇒ 而 B0294 那条「成功分支丢 `o.note`」在本仓**已经是现状** ⇒⇒ **⇒ 我 20 多批前记的那条已被修掉、而我本批才核**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0459 · ✅ **`code` 是必填的 · 「不写码」是选择不是缺码**
- ⭐⭐⭐⭐⭐ `api_client.dart:30-40` `ApiException(code, message, {status})`：**`code` 必填非空** · `message` 必填非空 · **可空的那个是 `status`**（含义 =「传输层有没有 HTTP 状态码」）
- ⭐⭐⭐⭐⭐⭐ **⇒ 而 `toString()` 已经把两者合成了**（「`$code：$message`」/「`HTTP $status $code：$message`」）⇒⇒⭐⭐⭐⭐⭐ **⇒ 「传输层失败不写码」这个「合理解释」**不成立** —— `code` 一直都在、`toString()` 一直带着它**
- ⭐⭐⭐⭐⭐ **⇒ 而同一个「`$code：$message`」拼法在同一行代码里出现过两次**：`main.dart:1049`（只写 `e.message`）⇄ `api_client.dart:38-39`（**两个字段都写**）⇒⇒⭐⭐⭐⭐⭐ **⇒ 「有一处现成的合成」是「要不要自己再拼一次」这个问题的正确答案**
- ⭐⭐⭐⭐ **⇒ 而 B0453 那条判据在这里也成立**：`status` 可空、其值域里**没有一个值本身有意义** ⇒⇒ **回落成 `0` 会与「传输层失败」混同** ⇒⇒ **而它没有回落到 `0`**
- ⇒ ⇒ **B0458 那条「不对称」现在可以写清了**：**它是一个选择**（只展示用户读得懂的那一段），**而 `e.code` 仍在 `toString()` 里、也仍可被别处取用** ⇒⇒ **按判据不记发现**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0460 · ⭐⭐ **「手感」类判据要落到一个具体场景才可用**
- ⚠ **两处过程注记**：① `dev_tools_section.dart` grep「主题/配色」**零命中** ⇒ **换文件**（`appearance_section.dart:3` 自述含「配色主题」）⇒ **B0316 规矩又一次应用**；② ⭐⭐ **`dev_tools_section.dart` 已是 1822 行、而我账上记 ~1780** ⇒⇒⇒ **「还剩 N 行」是**移动靶** ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒ 「它在长大」而不是「还剩多少」**
- ⭐⭐⭐⭐⭐ `appearance_section.dart:160-163`「这些参数**实时下发给渲染面：没有「保存」按钮**。理由：**拖滑杆时就想看到舞台反应，多一步保存会毁掉这个手感**。**主题同理**——点一下立刻整套换掉，正是它该有的手感」⇒⇒ **同一句理由管两处**
- ⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「手感」是判据、可复用是因为那句理由**含一个具体场景** ⇒⇒⭐⭐⭐⭐⭐⭐ **「拖滑杆时就想看到舞台反应」不是「我喜欢实时」—— 后者无法被反驳、前者可以被追问「哪个场景」** ⇒⇒⇒⭐⭐⭐⭐⭐⭐ **「手感」类判据要落到一个具体场景才可用**
- ⭐⭐⭐⭐⭐⭐ **⇒ 而「**可访问性测试依赖它们**」把「不能改」的理由从「我们决定不改」换成了「**有东西依赖**」** ⇒⇒⇒⭐⭐⭐⭐⭐ **「不能改」的三种理由**：①与外部对齐（B0411·B0454）②成本高于收益（B0307·B0414）③**有东西依赖**（本批补齐）
- ⭐⭐⭐⭐ **⇒ 而「本轮**只换外层容器**」是一句**范围声明****（同 B0440「文件一行未动」· B0393「代码改了、注释也改」）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0461 · ⭐⭐ **两个挂了几十批的未核实项都结了**
- ⭐⭐⭐⭐⭐ `app_shell.dart:613-620` `_currentBackground` 三分支与 **B0390 核的「按来源互斥」完全对上**；`if (items.isEmpty) return null;` + `items[widget.backgroundIndex.**clamp(0, items.length - 1)**]`
- ⇒ ⇒⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `clamp` 那一行正是我 B0437 提的「索引会变陈旧」问题的答案** ⇒⇒ **列表变短而 `backgroundIndex` 还指着旧位置** ⇒⇒⭐⭐⭐⭐⭐ **「库空」与「索引越界」被同一个表达式覆盖**
- ⭐⭐⭐⭐⭐ **⇒ 而 `_pane` 缓存注释把**失效条件逐条列出、每条给一个理由**（都真的与内容有关 ⇒ 排除「顺手也清一下」那类多余失效）
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「别改成 tick 那一层的」= 禁令 + 为什么必要**（`context` 的继承位置）+ 「**约定与回归见 `test/app_shell_background_test.dart`**」⇒⇒ **又一次指路 + 回归文件名**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 两处共同的一层：「**跨对象 / 跨帧的取值都要有边界**」** —— 一处 `clamp`（值的边界）· 一处三条失效条件（缓存的边界）⇒⇒⇒⭐⭐⭐⭐⭐ **「边界」有两种**
- ⇒ ⇒⭐⭐⭐⭐⭐⭐⭐ **⇒ 而它在**第一次读这个函数**时就该被看到 —— 而我当时是从**面板**那一侧读的** ⇒⇒⇒ **我 B0437 记的是「面板上那三个失败面」，而本批记的是「其中两个早已在宿主侧解决」**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0462 · ⭐⭐ **B0448 那条判据的第一次「算」**
- `test/app_shell_background_test.dart`：`_scaffoldBackground` 两态 + 「**`null` 确实是不透明的主题底（说明上面那条为什么致命）**」+ 「**AppShell 把 index 与 hydrating 原样注入设置面板**」（`probe:0/false`）；⭐ **`clamp` 边界零命中**
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 而它注入的是 `index: 0` + 非空库 ⇒ `clamp` 在 `index=0` 时**永远不生效** ⇒ 「越界」这条路径没被走到**
- ⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「分支覆盖率是指标、两条路径是否等价才是判据」（B0448）** —— **本条 `clamp` 与不 `clamp` 的两条路**不等价**（不 clamp 时：越界 ⇒ 背景消失）⇒⇒⇒⭐⭐⭐⭐⭐⭐⭐ **「越界索引 ⇒ 背景整块消失」是用户可察觉的后果**
- ⭐⭐⭐⭐⭐ **⇒ 而这与 B0448 判「不算」的那条**正好成对**：B0448 `['lines'] ?? ['logs']` 两路**等价** ⇒ 不算 · **本条两路**不等价** ⇒ 算 ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒ 「一条判据第一次判『不算』、第二次判『算』—— 而它两次都对」**
- ⭐⭐⭐ **⇒ 而「`null` 确实是不透明的主题底（**说明上面那条为什么致命**）」又是本仓的一个形状**：**一条断言顺带说明自己为什么重要**（同 B0322 / B0440）
- ⚠ **而我为什么仍不记 P3**：**关键未核项 = `backgroundIndex` 自己是否已被 `clamp` 过** ⇒⇒ **若是，则这条缺口不成立**（B0437 曾核 `:217` 附近有类似的 `clamp` ⇒ **二者相乘，可能完全不可达**）⇒⇒ **按 B0271 记为待核、不记发现** ⇒⇒ **「不记发现」的第三条理由（另两条：无用户可察觉后果 / 修法一行太轻）**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0463 · ✅ **B0462 的缺口不成立 · 权威在 `_slideshow`、宿主只跟随**
- ⭐⭐⭐⭐⭐ `main.dart:515-525`「**现在索引的合法范围由 `_slideshow` 说了算（setLibrary 里已经夹过），宿主这一份只跟随**」+ `_backgroundIndex` 由 `_slideshow.index` 驱动 ⇒⇒ **「只跟随」是**单向**的 ⇒⇒⇒ **权威在一处、其余只投影**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而它消掉的两个症状被**逐字**写出**：「**库从 5 项清成 1 项之后 `_backgroundIndex` 还是 4**」+「**下一轮 advance 又从 4 起算**，与『从头开始』的直觉不符」
- ⭐⭐⭐⭐⭐ **⇒ 而「**只让渲染层在取值时 clamp**」被记成一个**曾经合理、后来不够**的设计**（DEC-6 把它移到权威侧）⇒⇒ **同 B0437「读时处理 vs 源头处理」** ⇒⇒⭐⭐⭐⭐⭐⭐ **「读时 clamp」与「源头 clamp」的区别不是风格、是**下一次 advance 从哪起算****
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ B0462 那条缺口按规矩不记**（证据不足·B0271）⇒⇒ **「我提的问题，下一批自己答掉」的第四例**（另三例：B0300 · B0375 · B0412）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0466 · ⭐⭐ **一套独立码表 · 记为观察**
- ⭐⭐⭐⭐ `test_endpoints.rs`：`:142` **`network_failed`** · `:276` `auth_failed` · `:283` `protocol_error` ⇒ **三个都**没有 AGENTS 那套 `<stage>_<suffix>` 的 stage 前缀**
- ⇒ ⇒⭐⭐⭐⭐ **「另一套词表」本身是合理选择**（自检是探针端点、不是对话链）· ⭐⭐⭐⭐⭐ **而「要不要写下来」是可核的**：**对话链的码有自述（AGENTS + `ErrorKind::code()`）· 自检的码只有用法、没有自述**
- ⇒ ⇒⭐⭐ **而用户可见性是另一条轴**（`ok=false` 显示 `errorMessage ?? errorCode` · 传输异常分支只显示 `e.message`）—— **已在 B0459 结清**
- ⇒ ⇒⭐⭐⭐⭐ **为什么记为观察而不是发现**：**①「另一套码表」不违反任何被写下的约定**（AGENTS 说的是「**对话链**的错误码」，没要求探针端点也用）⇒ **「该不该统一」是裁决问题、不是判据问题**；**②** 用户可见性已结清 ⇒⇒ **⇒ 记为一条待核的自述缺口**
- ⚠⚠ **本批第六次工具层事故，触发器已定位**：**长装饰箭头链（`⇒` 重复几十次）在生成中坏掉**。可复用规则新增最后一条：⭐⭐ **装饰性重复要封顶 —— 信息量在文字里，不在符号数量里**（前五次：漏 import · 相对路径 · 引号吃管道 · heredoc 长正文 · 消息截断）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0467 · ✅ **B0466 的第 3 点撤回** · 判据是「可达否」
- ⭐⭐⭐⭐⭐ `test_endpoints.rs:262-270`「`GET {base}/models` 探针的结果分类（**纯函数**，可单测）。**口径（每条都有理由，别随手改）：**」+ 四条逐条带理由 ⇒⇒ **那份自述存在** ⇒⇒⇒ **B0466 写的「只有用法、没有自述」不成立，撤回**
- ⭐⭐⭐⭐⭐ **⇒ 而它的判据不是 `<stage>_<suffix>`、是「可达否」**：`auth_failed`=**可达但 key 不对** · `404`=**可达且未提供 `/models`** · `protocol_error`=**可达但坏了** ⇒⇒ **三个非 2xx 分支里有两个是「可达」** ⇒⇒ **「可达」是这套码表的判据、而对话链的码按环节分**
- ⭐⭐⭐⭐⭐ **⇒ 而段首「**每条都有理由，别随手改**」把判据与禁令写在一行**（同 B0330「段级禁令 + 理由」· B0421「禁令自带成立条件」）· ⭐⭐⭐⭐ 而 `probe_outcome` 自称「**纯函数，可单测**」（同 B0384 族）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0468 · ✅ **裁决守住 · 3/16 落在被禁用的 crate 里**
- ⭐⭐⭐⭐⭐ 全仓 `__apply_settings` 命中 **16** · **非注释（活代码路径）0** ⇒⇒ **走私真的删净了**；`services.rs:153`「**取代旧的 `__apply_settings` 事件走私**」
- ⭐⭐⭐⭐ **⇒ 而「取代了谁」写在**新 API 的头注里**（同 B0305 `calling:` / B0302 族）
- ⭐⭐⭐⭐⭐ **⇒ 而 16 条里 3 条在 `crates/live2d-ai-mod-local-llm/`**（AGENTS：**已废除启动、禁止挂回**）⇒⇒ **而那条禁令的判据是「不在 `AVAILABLE_MOD_FACTORIES`」**（`mod_count_is_three` 守着）⇒⇒⇒ **⇒⇒⇒ 那 3 条注释恰好在判据之外的地方**
- ⇒ ⇒⭐⭐⭐⭐⭐ **可提炼：「删除」有两种 —— 从**可达路径**上删 / 连注释一起删。而这条裁决属于**第一种** ⇒⇒⇒ 那 3 条是**迁���说明**、而它们依附的代码是死的 ⇒⇒⇒⭐⭐⭐ **⇒「注释会指向死代码」是删除类裁决的第四种残留形态**（另三种：类型 / 参数 / 分支）
- ⚠ **本批工具层小事故**：`write` 落下后第 35 行有一个 `·` 被写成了替换字符（`\ufffd`）⇒⇒ **已用 python 定点修复并复查（`仍含替换字符: False`）** ⇒⇒⭐⭐ **⇒ 新增一条可复用规则：写完要扫一遍 `\ufffd`**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **84**（P1 6 / P2 38 / P3 40）

### BATCH-0469 · ✅ **+1 条 P3：弃用信息放在了读者看不到的那一侧**
- ⭐⭐⭐⭐⭐ `lib.rs:3`「//! # ⚠️ 已废除启动（0.2.0-rc.1，2026-09-14）— **DEPRECATED，勿在新代码引用**」**存在且显著**；而 `Cargo.toml:3` 的 `description` **只有**「本地大模型进程管理 Mod（节点 E E5，P1-1）：spawn…」**、零 DEPRECATED**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 两个读者看到的东西不一样**：读 `lib.rs` 的人**第一屏第二行**就看到；读 `Cargo.toml` / `cargo search` / IDE 依赖树的人**看不到** ⇒⇒ **而后者恰好是最可能做错选型的那类读者**（「我想加一个本地 LLM Mod ⇒ 这个 crate 现成」）
- ⇒ ⇒⭐⭐⭐⭐⭐ **判据（可迁移）**：「**一个 crate 的弃用信息放在哪」的判据是「谁会先看到它」** —— `Cargo.toml` 的 `description` 是**工具读**的、`lib.rs:3` 的 `//!` 是**人读源码**时看到的 ⇒⇒ **只放后者 ⇒ 对前者不可见**
- ⇒ ⇒⭐⭐ **F-0469-01 (P3)** · 修法**一行**：description 末尾补 `（0.2.0-rc.1 已废除启动 · DEPRECATED）`；若担心重复维护 ⇒⇒ **两处服务的读者不同、所以重复是对的而不是冗余**（同 B0386 的取舍）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0470 · ✅ **顺序即语义 · 可核**
- ⚠ **换语言（B0440 规矩又一次应用）**：Dart 侧只在 `voice_listen_controller.dart:13` **文档注释**里提到 `evaluate_mode` ⇒⇒ **函数在 Rust 侧**（`gate.rs:124-141`）
- ⭐⭐⭐⭐⭐ **四个终点的顺序本身携带理由**：① `manual_enabled == false`（**用户显式关了 ⇒ 其它都不再相关**）② `wake_phrase` 为空 ③ `ptt` ④ 含唤醒词？
- ⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 ② 排在 ③ 之前，是因为「词空」与「按住说话」是**正交**的事** ⇒⇒⭐⭐⭐⭐⭐ **而 ③ 的实现印证了它**：`phrase` 空 ⇒ `strip_wake_phrase` 必 `None` ⇒ `.unwrap_or_else` 取原样 ⇒⇒⭐⭐⭐⭐⭐ **⇒ 换成之后，词空 + PTT 会变成 `GateClosed`** ⇒⇒⭐⭐⭐⭐⭐
- ⭐⭐⭐⭐⭐ **⇒ 「顺序即语义」在本仓是**可核**的：把两个分支对调、行为就变**
- ⭐⭐⭐⭐ **③ 的「不要求 + 若仍说了顺手剥掉」是「宽松读取」的一个实例**（同 B0430「不把不知道说成知道」的镜像：这里是把「多说的」也收下）· ⭐⭐⭐⭐ `.unwrap_or_else(|| cleaned.to_string())` 把「没有」表达成「原样」—— 与 B0453 方向相反、**而这次是对的**（「没剥到」与「就该原样」在 PTT 这条路上本来就是同一件事）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0471 · ✅ **推论被确认 · 判据第四次出现**
- ⭐⭐⭐⭐⭐ `gate.rs:158-168` `strip_wake_phrase`：`find_wake_span(...)?`（**找不到 ⇒ None**）· `text.chars().collect()`（⭐ **按 char 不按 byte**）· `Some(clean_transcript(&body).unwrap_or_default())`（⭐ **专门空串**）
- ⇒ ⇒⭐⭐⭐⭐⭐ **① 确认了 B0470 那个推论** —— 且是**唯一的路径**（「`unwrap_or_else` 在词空下取原样」成立）
- ⭐⭐⭐⭐⭐ **⇒ 而 `chars()` 是一处 Unicode 安全处理**（按字节切会切在多字节序列中间）⇒⇒ **与 AGENTS「界面文案里不得出现子集外的字符」互为表里**：**那条防输出端的字符、这条防处理端的切分**
- ⭐⭐⭐⭐⭐ **⇒ 而 ③ 那一行是本批最值钱的**：`clean_transcript(&body).unwrap_or_default()` **明确区分了「剥完是空的」与「剥完有内容」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐ **而 B0470 那个 `.unwrap_or_else(|| cleaned.to_string())` 若用在这里，就会把「剥完是空的」说成「原样」**
- ⭐⭐⭐⭐⭐ **⇒ 「回落不与真实值同义」第四次出现**（`toValuesMap()` 键不出现 · Dart `segments: None` · `max_tokens` 用 `containsKey` · **Rust `strip_wake_phrase` 专门空串**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0472 · ✅ **两批推论成立 · 文档两件事各有一处落点**
- ⭐⭐⭐⭐⭐ `gate.rs:176-203` `find_wake_span` **五条 `None` 路径**：① 词里没有非空白（`needle.is_empty()`）② 全空白（`position(...)?`）③ 匹配中跳过文本空白 ④ 不匹配 `break` ⑤ 没走完
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ B0470/B0471 两批的推论全部成立**：`phrase` 空 ⇒ ① ⇒⇒ `strip_wake_phrase` 必 `None`、`.unwrap_or_else(|| cleaned.to_string())` 是**唯一路径**；⇒⇒ **而「把 ② ③ 对调会改变行为」也成立**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而文档说的两件事各有各的代码落点**：「**短语自身的空白**先被丢掉」→ `:178` 的 `filter` ·「**文本里的空白**在匹配过程中被跳过」→ `:188-190` 的 `continue` ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 这两句长得极像、极容易被当成同一件事** ⇒⇒ **「把两处都写成『忽略空白』会让人以为只有一处、于是改动时只改一处」**
- ⇒ ⇒⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而这与 B0443「同一句话在同一文件里出现三次」是同一族的反面**：**那一族是「重复得不够」，这一族是「像得过头」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐
- ⭐⭐⭐⭐⭐ **⇒ 而 `lower_eq`（大小写不敏感）也是文档承诺 ⇒ 三条承诺、三个落点、一一对上**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0473 · ✅ **一个闩 · 一份不可见字符清单 · 一个有论证价值的默认值**
- ⭐⭐⭐⭐⭐ `lib.rs:256-266` `clean_transcript`：`pending_space` 是一个**闩**（延后写空格）⇒⇒ **两个边界用同一个机制处理**（「前导空白落不下、尾随空白自然丢」）⇒⇒ **⇒ 「边界不是两处特判、而是一个状态」**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而四种零宽/方向控制字符被显式点名**（`\u{200B}` · `\u{FEFF}` · `\u{200C}` · `\u{200D}`）⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 关键：它们 `is_whitespace()` 为 `false` ⇒ 上面那个分支抓不到** ⇒⇒ **⇒ 「所以这两行不能合成一行」· 判据：「`is_whitespace()` 抓不到零宽」** ⇒⇒ **同 B0000 ⑫ / B0142 族**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `manual_enabled_from_config` 的 `unwrap_or(true)` = 「读不到 ⇒ 闸门默认开」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 这是 B0453 那条判据的一个反例、而这次是对的**：`max_tokens` 值域里有**有意义的值**（`0`=无限）⇒ 不能回落到 `0`；而 `true`/`false` **都不是「无意义值」** ⇒ 回落到 `true` 是对的 ⇒⇒ **「回落值不能与真实值同义」在有意义的值存在时才成为问题**
- ⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「读不到 ⇒ 默认允许」是一个需要论证的默认值**：论证方向是「用户装了 Mod 就是想用、而闸门关着会让他以为坏了」；**而它的对立面（回落到 `false`、更保守）也可辩护** ⇒⇒⭐⭐⭐⭐⭐ **⇒ 而这个默认值没有注释论证** ⇒⇒ **⇒ 「可辩护的两种默认值里选了其中一个、而不写为什么」**
- ⇒ ⇒⭐⭐⭐⭐ **0 findings**（记为**观察**）：**「没有论证」不足以记缺陷** —— 这里**有理由、只是没写下来**，而**修法是一行注释**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0474 · ✅ **一份跨语言常量 · 后果写成用户可见的对句**
- ⭐⭐⭐⭐⭐ Rust `gate.rs:70` `DEFAULT_WAKE_PHRASE = "小可爱"` · Dart `voice_listen_controller.dart:33` `kDefaultWakePhrase = '小可爱'` ⇒⇒ **逐字相同**；⭐ 而兜底也同（`Some(raw) => raw.trim()` / `None => DEFAULT.to_string()` ⇒ **键缺失与键非字符串走同一条兜底**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而漂移的后果被写成一个具体的、用户可观察的对句**：「**按钮说小可爱、服务端拒小可爱**」⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 那个后果是两侧各说各话、而用户是唯一知道两边不一致的人**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「逐字一致」被写成**要求**、不是巧合** + 「**回归在 `test/voice_wake_default_consistency_test.dart`**」⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **又一次指路 + 回归文件名**（而它是 B0438 核过那 30 个一致性测试里**最短的一个**·41 行）
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而这与 B0440「两个文件各钉一个数值」同族、但更硬**：**关键差别在「发现机制」** —— 同一语言的两次 `expect` ⇒ **编译就红**；跨语言 ⇒ **只能靠一条测试** ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 「跨语言的那一对**必须**有一条跨语言断言」**（B0411/B0412 的**第三个实例**）
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0475 · ✅ **断言是真跨语言的 · 疑问结清**
- ⭐⭐⭐⭐⭐ `test/voice_wake_default_consistency_test.dart`（41 行）**用正则从 Rust 源码抓常量**（`pub const DEFAULT_WAKE_PHRASE:\s*&str\s*=\s*"([^"]+)"`）**再与 Dart 常量比** ⇒⇒ **断言是真跨语言的、不是硬编码对拍** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而若硬编码 `'小可爱'` 与 Dart 比、两侧同改时仍会绿**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而头注比 B0474 那句更具体**：点名了**状态码**（「**服务端回 400 wake_phrase_require**」）⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「断网 / 无日志时完全看不出来」是本仓第 4 次出现的「看不见的失败」表述**：字体子集「有网时看不出来」· CanvasKit CDN「断网即白屏」· `/logs?limit=200` 回 501「诊断日志从未成功过」· **本处** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「明确 skip、**不伪造通过**」是 AGENTS 那条纪律在**测试体例**上的同款实现** ⇒⇒ **⇒「不伪造绿灯」从 CI 纪律变成了测试体例、而这两处是同一句话** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「找不到文件 ⇒ print SKIP ⇒ return」是**最弱的一种 skip**（不失败、也不标记 `skip:`）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0476 · ✅ **1 / 102 · 孤例而非习惯**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `test/*.dart` 共 **102** 个 · 用 `print('SKIP: …') + return` 的只有 **1** 个 · 用 `skip:` / `markTestSkipped` 的只有 `no_backdrop_filter_test.dart`（3 处）⇒⇒ **「最弱的 skip」是孤例、不是习惯**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而那一个文件恰好是**跨语言**那个**（要读仓外的 Rust 源码）⇒⇒ **⇒ 「**只有需要越过包边界的那一个**才需要这种 skip」** ⇒⇒ **⇒ 「其余 101 个都在包内、所以它们不需要 skip」**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 两种跳过的强弱可分**：`skip:`/`markTestSkipped` **正式**（计入统计、reporter 打出）vs `print+return` **最弱**（**什么也不记** —— 一次「通过」）
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `print` 那行**不落在读者看的那张表上**（测试的读者是**报表**）⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0477 · ✅ **「例外」的四个要件全有**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `test/no_backdrop_filter_test.dart`：**例外被点名**（`blurExceptionFile`）· **判据逐路径**（`skip: (String path) => path == …`，不是整块）· **例外本身被守住**（`:99`「必须真的套了 `RepaintBoundary`（否则例外不成立）」）· **扫描器自己被验**（`:41`「防路径写错导致『零命中=通过』」）⇒⇒ **四个要件本仓这一处全有**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒「例外要具名」**同 B0439「§2 B 栏冻结」族**（例外被点名、而不是「除了某类」）** · ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ `:99` 是「例外本身也要被守住」**（同 B0441「那个测试要能先红」族）· ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ `:41` 是「先证明自己会红」（B0381）的又一处** —— **它防的不是「违例没抓到」、是「扫描器压根没扫到东西」**
- ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ 本批修正 B0476 第 3 点**：「`skip:` 出现 3 处」是**机械 `grep -c` 数行、不是数处** ⇒⇒ 而无论哪一种，它都是「逐路径、带名字的例外」⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「grep -c 数行不是数处」是本审计第二次栽在同一个工具特性上**（第一次：B0274 的 `head -3` 截断）⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0478 · ✅ **「例外」是三条并列的约束 · 自证被写成了断言**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `test/no_backdrop_filter_test.dart:80-85`「例外被限定在**三件事**上：① 只允许出现在**这一个文件**；② **sigma 由偏好上界（8 px）压着**；③ **必须有 `RepaintBoundary`**。**任何一条被破坏，下面两条测试都会红。**」+ `blurExceptionFile = 'lib/ui/shell_backdrop.dart'` ⇒⇒ **「例外」不是一句「这里可以有」、而是三条并列的约束**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「任何一条被破坏、下面两条都会红」是自证** —— **它把「这三条各自有人守」写成了断言** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「例外有多条约束」与「每条都有守卫」是两句话、而第二句把自己钉住了**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ B0477 核过 ① 与 ③**；而 **②「sigma 由偏好上界（8 px）压着」**不在这个文件里**、而在**偏好层** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0479 · ✅ **② 成立 · 守卫方式是「不可达」**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `display_prefs.dart:352` `static const double maxBackgroundBlur = 8.0;` + `shell_backdrop.dart:288-290`「sigma 用半径换算（σ ≈ r/2），再乘 0.5」⇒⇒ **「sigma 由偏好上界压着」成立的方式是「偏好里没有超过它的可能」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 守卫不是「每帧检查 sigma ≤ 8」—— 而是滑杆的取值范围本身就是 `0.0 … 8.0`**
- ⭐⭐⭐ **⇒ 而它是一个 `class static const`（不是顶层 const）** ⇒⇒ **⇒ 「上界住在拥有它的那个类里」** ⇒⇒ **同 B0443 核的「`stripCommentsAndStrings` 住在 `design_tokens_test.dart` 里」是同一个形状的两面**
- ⭐⭐⭐⭐⭐ **⇒ 而「σ ≈ r/2」这个近似有没有被测 —— 未核**（B0439「§2 B 栏冻结」是这类近似的先例）
- ⇒ ⇒⭐⭐⭐⭐ **⇒ 本批把 B0478 ② 从「未核」变成「成立」⇒⇒ 三条约束全部核完**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0480 · ✅ **三处引用一个常量 · 我上一批的措辞被自己收窄**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `maxBackgroundBlur` 的**三处引用 + 一处测试**（`:352` 常量 · `:815` 写入口 `clamp` · `appearance_background.dart:540` 滑杆 `max` · `display_prefs_test.dart:645`）**读的都是同一个常量**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而我 B0479 说的「守卫不是检查、是**不可达**」要收窄** —— **它同时是一次 `clamp`（`:815`）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「守卫不是**每帧**检查」仍成立**（读到的 `blur` 一定在界内）· **「不是不可达」要改** ⇒⇒ **⇒ 「措辞被自己下一批收窄」的又一处**（B0428 / B0412 / B0457）
- ⭐⭐⭐⭐⭐ **⇒ 而 `:812` 用的是「**夹到** `[0, maxBackgroundBlur]`」** —— 「夹」是动作、「区间」是集合 ⇒⇒ **注释同时说了两者（做什么 + 做到哪）**
- ⭐⭐⭐⭐⭐⭐ **⇒ 而 `display_prefs_test.dart:645` **引用了这个常量**、不是写死 `8.0`** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 这与 B0474「读 Rust 源码」是同一原则的轻量版：改常量时测试跟着变、不会因两边都写死同一个数而假绿**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0481 · ✅ **两种相反的策略 · 分界是「值域里有没有语义」**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `test/display_prefs_test.dart` 两条相邻测试明说相反策略：`:641`「一律 **clamp 到区间**（模糊不许变负）」· `:656`「枚举字段越界**回落默认**而不是夹到端点」⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 分界正是「这个值域里有没有哪个值本身带着语义」**（同 B0453；B0473 的 `manual_enabled` 是它的**反例**）
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `:656` 把那层语义说成了**用户可见的后果**：「**把坏值夹到 1 会让背景不可读**」⇒⇒ **⇒ 「端点有语义」不只被断言、还被翻译成一个界面上的坏结果**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `:656` 还写下了这一条规则的**边界**：「**不含 `slideInterval`：DEC-1（2026-09-28）把它改成了端点夹持，回归在 `display_prefs_background_fit_test.dart` 的 DEC-1 组**」⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「例外被点名 + 回归被指路」两个动作在这里同时发生**
- ⭐⭐⭐⭐⭐ **⇒ 而 `:641` 里「非有限数（含 ±Infinity）一律回落默认，**与 `clampVolume` 等同一条纪律**」** ⇒⇒ **⇒ 「同一条纪律在另一个字段上也这么用」被写成了一句话**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0482 · ✅ **「夹持」是一张对照表 · 表里含历史 bug 本身**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `display_prefs_background_fit_test.dart:70` `test('夹持对照表：**0→0 / 1→5 / 5→5 / 300→300 / 301→300 / 3600→300 / 99999→…**')` + `:72` `const Map<int,int> table` ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 改动前的行为被当成「要记住的缺陷」、而不是「要继承的行为」**（`1→5` 就是**历史 bug 本身**）
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而头注把「改之前是什么样」写下来了**（`slideInterval` 由「**越界回落 0（= 静默关掉轮播）**」改成端点夹持）⇒⇒ **⇒ 而「静默关掉轮播」正是 B0435 核过的那类后果：一个没有报错、只是**少做了一件事**的行为**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而头注里还列了 DEC-5**（`isRenderable` 从「非空串」收紧到真 dataURL 形态——「**坏图不再让界面说「有背景」**」）⇒⇒ **⇒ 而那句话是 F-0446-01 的**正解方向**（「标题在替一个不存在的过滤背书」的反面）
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而三个 DEC 的呈现方式完全同形**：「**改前是什么 → 改成什么 → 后果是什么**」⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0483 · ✅ **那句注释就是 B0328 那条缺陷的成因**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `live2d_stage.dart:533-550` `retry()`：置 `_bridge=null` · `unawaited(_renderSubscription?.cancel())` · `removeListener` · `unawaited(previous.destroy())` · `_hostError = null` · ⭐「**重建后要重新上报阶段（否则宿主停在旧的 error 覆盖层上）**」· `_lastReportedPhase = null` · `_generation++`
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 那句注释就是 B0328 核过的那条缺陷的**成因，一句话** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「重置」不是一件事、是两件**（`_hostError` + `_lastReportedPhase`）—— **只重置前者就会复现那条缺陷**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `retry()` 的**契约写在头注里**：「**宿主只需要 `currentState?.retry()`、不必知道舞台内部是怎么重挂的**」⇒⇒ **同 B0305 `calling: [_saveAndRefetch]` 族：对外一句、内部不外泄**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而拆桥是**有界的四步** · ⭐⭐⭐ `unawaited(...)` 出现两次（`cancel`/`destroy`）⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「不等」被显式写出来了**（不是忘了 await、而是标明了「不等」）**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0484 · ✅ **落点是 `build` 里的 `ValueKey`** · 「凭印象补全」第六次
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐ `live2d_stage.dart` `_generation` 的**三处**：`:189` 声明 · `:549` `setState(() => _generation++)`（**唯一自增**、在 `retry()` 里）· **`:570` `key: ValueKey<int>(_generation)`（唯一读、在 `build` 里）**
- ⇒ ⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ B0483 我写的「`didUpdateWidget` 侧」是**错的** —— 我猜了它所在的位置而没核** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「凭印象补全」第六次** ⇒⇒ **而这一次的形状最典型：**我说了一个**具体的函数名**、而事实是另一个**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而「机制」这个说法要改** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 它与 B0416「第 12 条：读宿主在 `didUpdateWidget` 里」是**同一条纪律**：**决定「要不要换节点」的那一栏由读放在 `build` 里** ⇒⇒ **⇒「同一条纪律」在同一个文件里以两个函数各做了一次**
- ⭐⭐⭐ **⇒ 而 `setState(() => _generation++)` 的写法** ⇒⇒⭐⭐⭐ **⇒ 计数器的自增被包在 `setState` 里 = 自增与「这一次会重建」是**同一个动作** ⇒⇒ 同 B0420「一个 `setState` 里做了三件事」—— 这里做的是**两件**、而第二件是它的**必然前提**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0488 · ✅ **「面板自己不做网络」是一族约定（三处同形）**
- ⭐⭐⭐⭐⭐⭐⭐⭐ 「面板**自己不做网络**」出现在三处头注：`memory_panel.dart:10-11`（生产）· `test/director_panel_test.dart:8` · `test/external_input_panel_test.dart:9`（测试）⇒⇒⭐⭐⭐⭐⭐ **⇒ 而「注入 fake `ModPanelContext`」是它的**实现方式** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「可测性」先于「不许发网络」被实现、而后者是它的**副产品**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 B0487 我说的「守卫**没被断言**」要收窄一半**：准确的说法是「**没有一条断言它**」⇒⇒ **⇒ 而「注入 fake」这件事**本身**就是一条结构上的保证**（发真网络要有一个 client 参数、而面板没有）⇒⇒ **⇒⇒ 而这与 F-0436-01 是**同一个形状**（名字强于断言、而那里也有一条同层先例）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- ⭐⭐⭐⭐⭐ **⇒ 而「注入 fake」在**测试**里说、而「不做网络」在**生产**里说** ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒** ⭐⭐⭐⭐⭐** ⇒⇒⇒⇒†
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0485 · ✅ 补落盘（B0485/0486/0487 曾在对话里说、未落盘）

### BATCH-0486 · ✅ 补落盘（B0485/0486/0487 曾在对话里说、未落盘）

### BATCH-0487 · ✅ 补落盘（B0485/0486/0487 曾在对话里说、未落盘）

### BATCH-0489 · ✅ **「面板自己不做网络」是类型层面的事**
- ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ `mod_panel.dart:27-40` `ModPanelContext` = 六个 `required`（`mod` / `state` / `stateLoading` / `stateError` / `onRefreshState` / `onCommand`）+ 两个可选（`activeSessionId` / `onModChanged`）⇒⇒ **没有一个是 `http` / `client` / `baseUrl`** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 要违反「面板自己不做网络」，得先给这个类加一个字段**
- ⭐⭐⭐⭐⭐ **⇒ 而唯一的动作出口 `onCommand` 是回调、而回调的实现方持有网络** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「依赖方向」在这里是单向的：面板 → 回调 →（宿主）→ 网络**（同 B0409「依赖方向即测试性」）
- ⭐⭐⭐⭐⭐ **⇒ 而「什么时候刷新」这个决定也在宿主侧**（`onRefreshState` 是不带判断的被动回调）⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- ⚠⚠ **本批第七次工具层事故（新形状）**：**输出里混入了非我发起的工具调用片段** ⇒⇒⇒ **停手、只保留可核的实质**。⇒⇒ ⭐⭐ **七种形状的共同解法：每批实质只由引号包起来的短行承载；长正文交给 `write` 工具** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- ⇒ ⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **B0487 那条「没有一条断言它」因此几乎不算缺口** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **85**（P1 6 / P2 38 / P3 41）

### BATCH-0498 · 三个小节；「没有回显」拆成三个否定
- env_key_field.dart 头注 = 三个小节：它为什么存在 / 与其它字段的区别（安全边界）/ 怎么做
- 三个否定：不持有 · 不请求 · 不显示当前值；另有「值只往上走（响应只回 {key, set}）」与「obscureText + 每次保存后立即清空」
- 可提炼：「不泄露」拆成三个否定，而「只往上走」是**方向**的表述（不是「不返回」）
- 【模式 Q】✔ · 有效 85（P1 6 / P2 38 / P3 41）· 撤回 4 · P0 = 0

### BATCH-0567 · C 栏核对完 · 一处 P3 观察
- C 栏两列异源（左=前端 preset / 右=渲染面 ack）· 右列三值（applied/dropped/空）· clear 同时清两栏
- **一个常量服务两个用途**：`kObserverBufferCapacity` 在 render_events.dart:192，注释只写「阶段5 §2 **B 栏冻结**」；而 C 栏左列经 _trim 复用它
- ⇒ 后果具体：给 C 栏加历史会改到**被裁决冻结**的 B 栏 ⇒ 记为 P3 观察（修法一行注释；不拆常量，按 P30 两处不够）
- 【模式 Q】✔ · 有效 85（P1 6 / P2 38 / P3 41）· 撤回 4 · P0 = 0

### BATCH-0568 · 回填 B0499–B0566（68 批）
- 第二次「对话为唯一真相」故障已修：BATCH-0499-0566-BACKFILL.md 逐条回填，**每条带 file:line**、**未核项照原样保留**
- 回填统计：**0 findings 61 批 · 含 P3 观察 7 批 · 进入 FINDINGS.md 0 条**
- 涵盖四条链：背景预览五跳（B0514–B0517）· 密钥写入 13 处（B0518–B0561）· 发送链（B0539–B0543）· 播报链（B0544–B0549）· 静音观测 11 处（B0528–B0538）
- 【模式 Q】✔ · 有效 85（P1 6 / P2 38 / P3 41）· 撤回 4 · P0 = 0

### BATCH-0595 · ✅ **+1 条 P2：一份描述「已拆除工具层」的文件只剩文档在引**
- ⭐ `shared/mcp_tools.json` 全仓检索**只命中 `.md`**（CHANGELOG + 四份 docs）；**crates / shell/flutter / tests / scripts / .github 零命中**
- ⇒ 而 AGENTS：「**LLM 工具层与动作系统已整体拆除**——**不要**再以…为前提写代码或文档」+「`plugin-sdk.md` **与 mod-product-chain.md 冲突时以它为准**」
- ⇒ **F-0595-01 (P2 · doc-vs-impl)**：读 `plugin-sdk.md` 的人会找到一份「看起来是当前契约、实际描述不存在的东西」的 JSON，而读那份 JSON 的人**没有任何机制知道它已不适用**
- 【模式 Q 双向检查 + KNOWN-LOSSES 扣减】**✔ 通过** · 有效 **86**（P1 6 / P2 39 / P3 41）

# BATCH-0691 落盘（极简）· ⭐ `common/mod.rs` 有 **16 个公开项**；「`use` 清单」与「定义清单」的差集**正好是 `{contains}`**

## 编号
`BATCH-0691.md` **不存在**（首次占用）⇒ 直接落盘。

## 命令（只读，**带前缀**）
```
cd /home/skystar/Live2D-Ai-fe/AUDIT-REPO
echo "LS0691: $(ls BATCH-0691.md 2>&1|head -1)"   -> ls: cannot access（空号）
cd /home/skystar/Live2D-Ai-fe/crates/live2d-ai-runtime/tests
grep -n "^pub fn|^pub struct|^pub type|^pub const" common/mod.rs
```

## 16 个公开项（`common/mod.rs`，**315 行**）
```
── 常量 2 ──
:25  pub const READ_TIMEOUT: Duration = Duration::from_secs(5);
:28  pub const STEP_TIMEOUT: Duration = Duration::from_secs(10);
── 类型别名 2 ──
:33  pub type BoxFut = Pin<Box<dyn Future<Output = ()> + Send>>;
:35  pub type SharedHandler = Arc<dyn Fn(TcpStream) -> BoxFut + …
── 结构 2 ──
:39  pub struct Handler(pub SharedHandler);
:51  pub struct CapturedRequest { … }
── 函数 10 ──
:55   pub fn contains(haystack: &[u8], needle: &[u8; 4]) -> Optio…
:138  pub fn respond_pieces(…)
:171  pub fn sse_content(text: &str) -> Vec<u8>
:175  pub fn sse_done() -> Vec<u8>
:183  pub fn assert_epoch(events: &[EngineEvent], epoch: u64)
:202  pub fn assert_single_terminal_last(events: &[EngineEvent], …
:220  pub fn collect_audio(events: &[EngineEvent]) -> Vec<(u64, …
:269  pub fn find_error_kind(events: &[EngineEvent]) -> Option<&…
:276  pub fn engine(llm_base: &str, tts_base: &str, config: Conv…
:42   （impl Handler 的 pub fn new）        ← 不在上表（B0687 核过它属 impl）
```
⇒⇒ 与 `conversation_engine_tts_flow.rs:12-16` 的 **`use` 清单（14 个）**对账：
`Handler, STEP_TIMEOUT, assert_epoch, assert_single_terminal_last, close_sock(…), engine, find_error_kind, read_speech_input, respond_pieces, spawn_multi_server(…), sse_content, sse_done, write_head, write_pieces`

## 三个可核点
1. **⭐⭐⭐⭐⭐ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ **B0688 第 ② 点那个「一个模块的清单最可靠的来源是
   它的 `use` 语句」被完整验证** ⇒⇒ 「`use` 清单」∩「定义清单」的**差集** =
   **没被任何人用的那一个** ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 而本批的差集 = `contains`（唯一一个）**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒判据（本批最值钱，可复用）：
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 「**定义清单** − 「**`use` 清单**」= 死代码**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 而这是**一次集合运算**，不是**逐个 grep**** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 差别的量级**：
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 「一个模块的 16 个公开项里哪个没人用」
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 答案是**做一次差集**，而不是**逐个搜 16 次**** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒判据（第二句）：
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 前提是「`use` 是**完整**的」—— 而 B0688 核过它**可能是多行的**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ ⇒⇒ 差别的量级**：
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 若 `use` 写成**一行**，`grep` 直接可用；
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 写成**多行**（本例），**必须读那一段**才能拿到清单**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ 承 B0688 第 ③ 点「**看得够长**取决于
   下一步要判定什么**」**的第四次** —— **本批是「**差集**」这个操作决定的**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ 判据（第三句，**本批第二个**）：
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 「**一个模块的清单**」应当从**两侧**取**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ —— 定义侧（本批 16）+ 使用侧（B0688 的 14）**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 两侧都齐 ⇒⇒ 差集才有意义**
2. **⭐⭐⭐⭐⭐ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 而那 16 个里有**两个具名常量**（`READ_TIMEOUT: 5s` / `STEP_TIMEOUT: 10s`）⇒⇒
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒判据（本批第三个，可复用）：
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 「**超时**」在测试支撑里**必须是具名常量**，
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 否则每个测试各写一个 `Duration::from_secs(5)`，调超时就要改几十处** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 差别的量级**：
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 而 `STEP_TIMEOUT: 10s` 与 `READ_TIMEOUT: 5s` **是 2 倍关系**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ 判据（第二句）：
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 「**两个超时的比例**」本身也是一条判据**——
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 「等响应 5s」「等整步 10s」的比例说明**它们管的是不同的等待**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ ⇒⇒ ⇒ 承 B0604b 核的
   `NavMetrics`「唯一定义点」、B0601「常量不外传」**同族** ——
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 那边是「**一个值**只有一个定义点」，
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 这边是「**一类的值**（超时）集中在一个文件」** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ 差别的量级**：
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 而 AGENTS 记「实测假失败」那条教训（自检 3s vs 真合成 2.4s）
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 恰恰是**超时值定错的代价** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ ⇒⇒ ⇒ 承 B0612b 核的
   `NoSessionPrompts`（默认值选可查询的那侧）**在**时间**上的同族版**
3. **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 而 4 个 `assert_*` / `find_error_kind` / `collect_audio`
   都收 `&[EngineEvent]` ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒判据（细节，承 B0679 第 ③ 点「内聚的族用统一签名」）：
   **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 「**查事件**」那 5 个**统一收 `&[EngineEvent]`**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ ⇒⇒ ⇒ 差别的量级**：
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 而 `desktop/src/supervisor/support.rs` 那 5 个
   （B0679 核过）收的是 **`&Collector`**（= `Arc<Mutex<Vec<AppEvent>>>`）**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ ⇒⇒ ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 差别的量级**：
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 同一个「查事件」的族，在**两个 crate** 里**签名不同**
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 因为**事件类型不同**（`EngineEvent` vs `AppEvent`）
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ 差别的量级**：
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 而**接收者类型写在签名里** ⇒⇒
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ ⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒ ⇒⇒ ⇒ 承 B0684 / B0689 核的「用类型表达约束」
   ⇒⇒ **⇒⇒⇒4 ⇒⇒⇒4 ⇒⇒⇒4 ⇒ 在**族**这一层上的第三次兑现**

## 未核
`close_sock` / `read_speech_input` / `spawn_multi_server` / `write_head` / `write_pieces`
这 5 个在 16 项表里没有 ⇒ **它们可能是 `impl Handler` 的方法**（下一批可核）
`CapturedRequest`（`:51`）的字段 · `collect_audio`（`:220`）返回的元组形状

# BATCH-0653 落盘（极简）· ⭐ `json_error` **确认是唯一收口**（两级包装都走它）

## 编号
`BATCH-0653.md` **不存在**（首次占用）⇒ 直接落盘。

## 命令（只读，**带前缀**）
```
cd /home/skystar/Live2D-Ai-fe/AUDIT-REPO
echo "LS0653: $(ls BATCH-0653.md 2>&1|head -1)"   -> ls: cannot access（空号）
cd /home/skystar/Live2D-Ai-fe
grep -c "json_error(" crates/live2d-ai-desktop/src/web_api/env_routes.rs   -> 4
grep -n "fn json_|-> Resp\b" 同上 | head -5
sed -n '158,176p' 同上
```

## 逐字（`:158-174`）
```rust
:158 pub(crate) fn method_not_allowed() -> Resp {
:159     json_error(
:160         StatusCode(405),
:161         "method_not_allowed",
:162         "`/api/v1/env` 只支持 GET（读键名）与 PUT（写入）",
:163     )
:164 }
:166 fn bad_request(code: &'static str, message: &str) -> Resp {
:167     json_error(StatusCode(400), code, message)
:168 }
:170 fn json_error(status: StatusCode, code: &'static str, message: &str) -> Resp {
:171     let body = serde_json::json!({"error": {"code": code, "message": message}}).to_strin…
:172     Response::from_data(body.into_bytes())
:173         .with_status_code(status)
:174         .with_header(Header::from_bytes(&b"Content-Type"[..], …
```

## 三个可核点
1. **⭐⭐⭐⭐⭐ ⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ B0652 第 ③ 点那个「`json_error` 是不是唯一收口」**
   **核完：是** ⇒⇒ 三条返回 `Resp` 的路径里，
   - `:158 method_not_allowed()` → **走 `json_error`**
   - `:166 bad_request(code, message)` → **走 `json_error`**
   - `:170 json_error(status, code, message)` ← **它自己是唯一真正构造 body 的地方**
   ⇒⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 判据（本批最值钱，可复用）：
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ 「唯一收口」的正确核法是**找全所有返回同一类型的地方**，
   ⇒⇒ ⇒⇒ ⇒ 然后**看谁最终构造了 body** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 差别的量级**：
   ⇒⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ 「有几个函数返回 `Resp`」是 3；
   「有几个函数构造 body」是 1** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒判据（第二句）：
   **收口性属于「构造 body 的那一步」，不属于「返回这个类型的所有函数」**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ 与 B0621 核的 `appMotion` 对照**：
   ⇒⇒ ⇒⇒ ⇒ 那里的「两个出口」是**两个都自己算 `disableAnimationsOf`**；
   ⇒⇒ ⇒⇒ ⇒ 本例的两个包装**都直接调同一个 `json_error`** ⇒⇒
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 同样的「看起来有两个」，
   **因结构不同而结论相反** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒判据（第三句）：
   **数「出口」要数到「自己实现那个动作」的那一层** ——
   ⇒⇒ ⇒⇒ ⇒ 包装层不构成新出口
2. **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ 而 `:166 bad_request` 的签名**只收 `code` 与 `message`**，
   `status` 被**写死成 400** ⇒⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒判据（可复用，承 B0500「缺省值写在参数表里」**的同族**）：
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ 「这个函数只能产生某一种状态码」这件事，
   应该由**签名**表达而不是由每次调用传参** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 收益**：调用点无法误传 405 给 `bad_request`
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 与 `:158` 对照**：
   那个函数的 405 也**写死在函数体内**（它压根不收 `status`）⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 两个包装都把 status 写死
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒判据（第二句）：
   **「能写死在函数里的常量就别做成参数」** ——
   ⇒⇒ ⇒⇒ ⇒ 而这与 B0604b 核的 `NavMetrics` 那一族**同源**（度量不外传）
3. **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ 而 `:171` 的 body 结构是 `{"error": {"code":…,"message":…}}`**
   —— 与 B0647 核的 Dart 侧 `env_api_test.dart:18` 那条测试
   「解析键名与「是否已设置」；**响应里没有值**」**形状不同**
   ⇒⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒判据（细节）：
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ `GET` 的成功体与 `PUT` 的错误体是两个 schema** ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 后果**：前端解析器要认**两种**形状 ⇒⇒ ⇒⇒ ⇒
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒判据（第二句）：
   **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ 「错误体」与「成功体」的结构差异应该在**端点头注**里点明**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 承 B0610b 核的
   `ErrorKind::code()` / `hint()` 那条（「错误码由它给出」「禁新错误路径」）**同族**
   ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒④ ⇒⇒⇒④ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ ⇒⇒⇒④ ⇒ 未核**：
   ⇒⇒ `env_routes.rs` 头注有没有写「GET 成功体 ≠ PUT 错误体」（下一批）

## 未核
`env_routes.rs` 头注有没有点明「GET 成功体 ≠ PUT 错误体」·
`error_messages_never_contain_the_value()` 的**断言体** ·
`json_error` 的 4 处调用分别在哪（哪 2 处是测试里的）

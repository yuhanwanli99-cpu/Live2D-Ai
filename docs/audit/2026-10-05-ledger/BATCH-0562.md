# BATCH-0562 落盘 · SubscriptionId 是按 mod 分区的

## 跑的命令（只读）
```
grep -n "fn unsubscribe" -A 12 crates/live2d-ai-desktop/src/mod_registry.rs
grep -n "fn subscribe" -A 10 crates/live2d-ai-desktop/src/mod_registry.rs
```

## 逐行
```
:715  fn subscribe(&mut self, topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
:716      let id = SubscriptionId(self.next_id.fetch_add(1, Ordering::Relaxed));
:717      self.subscriptions.entry(self.mod_id).or_default().push(id);
:718      let _ = topic; // v1 登记 topic，细粒度路由留 E5。
:719      Ok(id)
:720  }
:722  fn unsubscribe(&mut self, id: SubscriptionId) -> Result<(), ModError> {
:723      if let Some(v) = self.subscriptions.get_mut(self.mod_id) {
:724          v.retain(|s| *s != id);
:725      }
:726      Ok(())
:727  }
```

## 三个可核点
1. **HostRegistrar 自带 mod_id**（struct 第一个字段：B0699 核过）
   ⇒ `subscriptions` 的 key 是 mod_id ⇒ subscribe 写进自己那一格
   ⇒ unsubscribe 只在 `self.mod_id` 那一格里 `retain`
   ⇒ **⇒ B0561 提的越权面不存在**（A Mod 的 id 撤不掉 B Mod 的订阅）
2. **`:718 let _ = topic;` + 「v1 登记 topic，细粒度路由留 E5」**
   ⇒ **订阅了哪个主题**这一项**当前不参与路由** ⇒ 与 B0491「两处不同原因不共用一处」同族：
   **接口已经是多主题、v1 的实现是一主题** ⇒ 记为观察（不记发现：没有行为后果）
3. **`unsubscribe` 对不存在的 id 静默成功**（`get_mut` 拿不到就跳过、仍 `Ok(())`）
   ⇒ 幂等 ⇒ 与 B0584 核的 `restarted = false` 缺省处理**同族**（不猜、不抛）

## 未核
mod-system 的 tests/ 四行 · session.rs / topics.rs / settings.rs 本体 ·
其余 9 个 mod crate 本体 · shared/ 其余 8 个 json ·
setup_linux.sh / ignition-precheck.sh / verify_core_chain.py /
check_public_secrets.py / font_subset_ranges.py 本体 · 面板三份余下部分

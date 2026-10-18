//! `swap_core` 的用例（0.3.6-rc.4 从 `swap_core_tests.rs` 拆出，守 >500 行棘轮）。
//!
//! 本模块是 `swap_core_tests` 的子模块：fake loader / fake host / Gate 等基础设施
//! 由父模块提供，这里只放断言用例。

use super::*;
use crate::preset::OverrideFetch;

/// 成功：装载 → revoke → **model_epoch +1** → 更新 URL → 重载字段表 → 只发一条 `loaded`。
#[test]
fn success_installs_revokes_and_emits_loaded_once() {
    let host = FakeHost::new(OLD);
    let loader = FakeLoader::new([Ok(NEW.to_owned())]);
    let field_map = FakeFieldMap::with_fetch([FakeFieldMap::external_override("Epsilon_free")]);

    let mut h = host.clone();
    block_on(run_swap(&mut h, NEW.to_owned(), &loader, &field_map));

    let st = host.snapshot();
    assert_eq!(st.generation, 1, "每个请求都必须自增一次代际");
    assert_eq!(st.current_url, NEW, "成功后 current_url 必须是新 URL");
    assert_eq!(
        st.model_epoch, 2,
        "成功换模必须把 model_epoch 从初始 1 推进到 2"
    );
    assert_eq!(
        st.install_attempts,
        vec![(NEW.to_owned(), true)],
        "成功路径只装一次新模型（不装载旧 URL）"
    );
    assert_eq!(st.revoked, 1, "成功后必须 revoke 一次旧贡献 / pending");
    assert_eq!(
        field_map.reloaded.borrow().as_slice(),
        &["Epsilon_free".to_owned()],
        "成功后必须按新 model id 重载字段表"
    );
    assert_eq!(
        st.field_map_model, "Epsilon_free",
        "换模成功必须装新 model id 的字段表"
    );
    let loaded: Vec<_> = st.emitted.iter().filter(|(ty, _)| ty == "loaded").collect();
    assert_eq!(loaded.len(), 1, "loaded 只能发一条");
    assert_eq!(loaded[0].1["url"], NEW);
    assert_eq!(loaded[0].1["model_id"], "Epsilon_free");
    assert_eq!(loaded[0].1["model_epoch"], 2);
    assert_eq!(
        loaded[0].1["model_epoch"].as_u64(),
        Some(st.model_epoch),
        "loaded 的 model_epoch 必须与 state 里的当前值一致"
    );
    assert_eq!(loaded[0].1["field_map"]["source"], "external");
    assert_eq!(loaded[0].1["field_map"]["degraded"], false);
    assert!(
        loaded[0].1["field_map"]["reason"]
            .as_str()
            .is_some_and(|r| !r.is_empty()),
        "field_map.reason 必须可读：{}",
        loaded[0].1
    );
    assert!(
        st.statuses.iter().any(|s| s.contains("模型已切换")),
        "状态栏必须报告成功：{:?}",
        st.statuses
    );
    assert!(st.phases.iter().any(|p| p.contains("装载成功")));
}

/// 失败（坏 URL）：只回 `error` 帧；旧模型、旧 URL、字段贡献都不动。
#[test]
fn fetch_failure_keeps_old_model_and_emits_error_only() {
    let host = FakeHost::new(OLD);
    let loader = FakeLoader::new([Err("HTTP 404".to_owned())]);
    let field_map = FakeFieldMap::default();

    let mut h = host.clone();
    block_on(run_swap(&mut h, NEW.to_owned(), &loader, &field_map));

    let st = host.snapshot();
    assert_eq!(st.current_url, OLD, "失败必须保留旧模型 URL");
    assert_eq!(st.model_epoch, 1, "失败不得换 model_epoch");
    assert_eq!(st.field_map_model, "bai", "失败不得重装字段表");
    assert!(st.install_attempts.is_empty(), "坏 URL 不得碰渲染核心");
    assert_eq!(st.revoked, 0, "失败不得 revoke 旧贡献");
    assert!(field_map.reloaded.borrow().is_empty(), "失败不得重载字段表");
    let errs: Vec<_> = st.emitted.iter().filter(|(ty, _)| ty == "error").collect();
    assert_eq!(errs.len(), 1);
    assert!(
        errs[0].1["message"]
            .as_str()
            .unwrap()
            .contains("切换失败: HTTP 404")
    );
    assert_eq!(errs[0].1["url"], NEW, "error.url 是请求失败的新地址");
    assert_eq!(
        errs[0].1["model_id"], "bai",
        "error.model_id 必须是当前仍在用的旧模型"
    );
    assert_eq!(
        errs[0].1["model_epoch"], 1,
        "error.model_epoch 必须保持当前值（旧模型仍在）"
    );
    assert!(
        !st.emitted.iter().any(|(ty, _)| ty == "loaded"),
        "失败不得发 loaded"
    );
}

/// 失败（装载半失败）：install 是原子的 → **旧核心一个字段都没被碰过**，
/// 不回滚装载旧 URL，只回 error（T1 修法 B 的核心断言）。
#[test]
fn install_failure_leaves_old_core_untouched_and_does_not_rollback_load() {
    let host = FakeHost::new(OLD);
    host.script_install([Err("physics3.json 解析失败".to_owned())]);
    let loader = FakeLoader::new([Ok(NEW.to_owned()), Ok(OLD.to_owned())]);
    let field_map = FakeFieldMap::default();

    let mut h = host.clone();
    block_on(run_swap(&mut h, NEW.to_owned(), &loader, &field_map));

    let st = host.snapshot();
    assert_eq!(st.current_url, OLD, "装载失败必须保留旧模型 URL");
    assert_eq!(st.model_epoch, 1, "失败不得换 model_epoch");
    assert_eq!(
        st.install_attempts,
        vec![(NEW.to_owned(), false)],
        "只允许尝试装新模型一次；**不得**再 load 旧 URL 回滚"
    );
    assert_eq!(st.revoked, 0);
    // 0.3.6 T2 修订：字段表在**装载之前**重建（所有 await 都在状态变更之前），
    // 所以「取到了表」不等于「装了表」——装载失败时一张表都没落地。
    assert_eq!(
        field_map.reloaded.borrow().as_slice(),
        &["Epsilon_free".to_owned()],
        "字段表在装载前取（新时序），但失败时不得 apply"
    );
    assert_eq!(st.field_map_model, "bai", "装载失败不得换字段表");
    assert!(
        !st.emitted.iter().any(|(ty, _)| ty == "loaded"),
        "失败不得发 loaded"
    );
    let errs: Vec<_> = st.emitted.iter().filter(|(ty, _)| ty == "error").collect();
    assert_eq!(errs.len(), 1);
    assert!(
        errs[0].1["message"]
            .as_str()
            .unwrap()
            .contains("渲染核心换模型失败")
    );
    assert_eq!(errs[0].1["model_id"], "bai");
    assert_eq!(errs[0].1["model_epoch"], 1, "失败不换 epoch");
    assert!(
        st.phases.iter().any(|p| p.contains("旧核心未受影响")),
        "失败阶段日志必须写明旧核心未受影响：{:?}",
        st.phases
    );
}

/// 连点 / 并发：前一条慢请求的结果后到 → 代际守卫丢弃，只装最后一次。
#[test]
fn stale_result_is_discarded_and_only_latest_swap_lands() {
    let host = FakeHost::new(OLD);
    let first_loader = GateLoader::default();
    let second_loader = FakeLoader::new([Ok(NEW.to_owned())]);
    let field_map = FakeFieldMap::with_fetch([FakeFieldMap::external_override("Epsilon_free")]);
    // 慢请求的字段表重载器单独持有：它被丢弃后**不得**被调用。
    let stale_field_map = FakeFieldMap::default();

    // 第一次请求：拉取被 Gate 挂起（模拟慢请求）。
    let mut h1 = host.clone();
    let mut first = Box::pin(run_swap(
        &mut h1,
        "/models/slow.model3.json".to_owned(),
        &first_loader,
        &stale_field_map,
    ));
    assert!(matches!(poll_once(&mut first), Poll::Pending));
    assert_eq!(host.snapshot().generation, 1);

    // 第二次请求：立刻完成（新请求取代旧请求）。
    let mut h2 = host.clone();
    block_on(run_swap(
        &mut h2,
        NEW.to_owned(),
        &second_loader,
        &field_map,
    ));
    assert_eq!(host.snapshot().current_url, NEW);

    // 放行第一次请求的结果：必须被丢弃。
    first_loader.release("/models/slow.model3.json");
    assert!(matches!(poll_once(&mut first), Poll::Ready(())));

    let st = host.snapshot();
    assert_eq!(st.generation, 2);
    assert_eq!(st.model_epoch, 2, "只有成功的换模推进一次代号");
    assert_eq!(st.current_url, NEW, "慢请求后到不得覆盖已生效的新模型");
    assert_eq!(
        st.install_attempts,
        vec![(NEW.to_owned(), true)],
        "只有最后一次请求允许装核心（连点只装最后一次）"
    );
    assert_eq!(st.revoked, 1, "被丢弃的请求不得 revoke");
    assert_eq!(
        field_map.reloaded.borrow().as_slice(),
        &["Epsilon_free".to_owned()],
        "只有最后一次请求允许重载字段表"
    );
    assert!(
        stale_field_map.reloaded.borrow().is_empty(),
        "被丢弃的慢请求不得重载字段表"
    );
    assert_eq!(
        st.emitted.iter().filter(|(ty, _)| ty == "loaded").count(),
        1,
        "被丢弃的请求不得发 loaded"
    );
    assert!(
        st.phases.iter().any(|p| p.contains("丢弃结果")),
        "被丢弃的请求必须留下阶段日志：{:?}",
        st.phases
    );
}
// ─────────────────────────────────── 0.3.6 T2：模型身份 / field map 重绑

/// 字段表**降级**（外置表读取失败）也必须换到新 model id 的内建表——绝不沿用旧表。
#[test]
fn field_map_failure_rebinds_to_new_model_and_never_reuses_old_table() {
    let host = FakeHost::new(OLD);
    let loader = FakeLoader::new([Ok(NEW.to_owned())]);
    let field_map = FakeFieldMap::with_fetch([OverrideFetch::Failed("HTTP 500".to_owned())]);

    let mut h = host.clone();
    block_on(run_swap(&mut h, NEW.to_owned(), &loader, &field_map));

    let st = host.snapshot();
    assert_eq!(st.model_epoch, 2, "字段表降级不影响「换模成功 +1」");
    assert_eq!(
        st.field_map_model, "Epsilon_free",
        "字段表失败后必须是**新 model id** 的表；旧表（bai）不得沿用"
    );
    let loaded: Vec<_> = st.emitted.iter().filter(|(ty, _)| ty == "loaded").collect();
    assert_eq!(loaded.len(), 1, "降级仍然发 loaded（模型确实换了）");
    assert_eq!(loaded[0].1["field_map"]["source"], "degraded");
    assert_eq!(loaded[0].1["field_map"]["degraded"], true);
    let reason = loaded[0].1["field_map"]["reason"].as_str().unwrap();
    assert!(reason.contains("HTTP 500"), "降级原因必须可读：{reason}");
}

/// 外置表文件**本来就没有**（404）→ builtin + degraded:false（Lead 裁决：
/// 桌面端没有 /actions/field_map.json 是正常配置，不该长期报降级噪声）。
#[test]
fn missing_override_table_is_builtin_not_degraded() {
    let host = FakeHost::new(OLD);
    let loader = FakeLoader::new([Ok(NEW.to_owned())]);
    let field_map = FakeFieldMap::with_fetch([OverrideFetch::Missing("HTTP 404".to_owned())]);

    let mut h = host.clone();
    block_on(run_swap(&mut h, NEW.to_owned(), &loader, &field_map));

    let st = host.snapshot();
    assert_eq!(st.field_map_model, "Epsilon_free");
    let loaded: Vec<_> = st.emitted.iter().filter(|(ty, _)| ty == "loaded").collect();
    assert_eq!(loaded[0].1["field_map"]["source"], "builtin");
    assert_eq!(loaded[0].1["field_map"]["degraded"], false);
}

/// 字段表重载期间被新请求取代：慢请求的**旧表不得装到新皮套上**、不得发 loaded。
/// Lead 要求 a)：字段表 await 晚于新请求返回 → **不 install、epoch 不变、不发 loaded**。
/// （T2 修订后所有 await 都在状态变更之前，被取代的请求一个状态都没改。）
#[test]
fn stale_field_map_from_superseded_request_changes_nothing() {
    let host = FakeHost::new(OLD);
    let slow_loader = FakeLoader::new([Ok("/models/slow.model3.json".to_owned())]);
    let slow_field_map = GateFieldMap::default();

    // 第一次请求：模型立刻就绪，卡在**装载之前**的字段表重建（模拟慢 fetch）。
    let mut h1 = host.clone();
    let mut first = Box::pin(run_swap(
        &mut h1,
        "/models/slow.model3.json".to_owned(),
        &slow_loader,
        &slow_field_map,
    ));
    assert!(matches!(poll_once(&mut first), Poll::Pending));
    {
        let st = host.snapshot();
        assert_eq!(st.model_epoch, 1, "await 期间不得推进 epoch");
        assert!(st.install_attempts.is_empty(), "await 期间不得装核心");
        assert_eq!(st.field_map_model, "bai", "字段表还没装（仍在旧表上）");
        assert!(!st.emitted.iter().any(|(ty, _)| ty == "loaded"));
    }

    // 第二次请求：完整跑完（epoch 只 +1）。
    let new_loader = FakeLoader::new([Ok(NEW.to_owned())]);
    let new_field_map = FakeFieldMap::with_fetch([FakeFieldMap::external_override("Epsilon_free")]);
    let mut h2 = host.clone();
    block_on(run_swap(
        &mut h2,
        NEW.to_owned(),
        &new_loader,
        &new_field_map,
    ));
    assert_eq!(host.snapshot().model_epoch, 2);
    assert_eq!(host.snapshot().field_map_model, "Epsilon_free");

    // 放行慢请求的字段表：必须被 generation 守卫丢弃，**一个状态都不改**。
    slow_field_map.release("slow");
    assert!(matches!(poll_once(&mut first), Poll::Ready(())));

    let st = host.snapshot();
    assert_eq!(st.model_epoch, 2, "被取代的请求不得推进 epoch");
    assert_eq!(
        st.install_attempts,
        vec![(NEW.to_owned(), true)],
        "被取代的请求不得装核心（只有最后一次请求装过）"
    );
    assert_eq!(
        st.field_map_model, "Epsilon_free",
        "被取代的请求不得把旧 model id 的表装到新皮套上"
    );
    assert_eq!(
        st.emitted.iter().filter(|(ty, _)| ty == "loaded").count(),
        1,
        "被取代的请求不得发 loaded"
    );
    assert!(
        st.phases.iter().any(|p| p.contains("丢弃 field_map")),
        "字段表被丢弃必须留下阶段日志：{:?}",
        st.phases
    );
}

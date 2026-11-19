//! 六组菜单的纯判定单测（Linux 也真跑）。
//!
//! 这八条都读写 `menu_state::STATE`（进程级静态）而且开头都 `reset()`：
//! `cargo test` 默认多线程并行，不串行化就会「A 刚 reset、B 刚拨好、A 又 reset」，
//! 表现为单跑必过、全量偶发失败。统一走 `crate::desk_pet::test_guard()`
//! （同一把锁也罩住 `desk_pet` 的 ARMED 状态），生产语义一行未改。

use super::*;

fn labels(groups: &[Group]) -> Vec<&str> {
    groups.iter().map(|g| g.label.as_str()).collect()
}

#[test]
fn default_state_has_voice_off_and_both_bubbles_on() {
    let _guard = crate::desk_pet::test_guard();
    reset();
    let s = get();
    assert_eq!(s.voice, VoiceMode::Off, "缺省必须是关闭，不录音");
    assert!(s.dialog_on && s.show_user && s.show_ai);
    assert!(!s.chat_open && !s.chat_topmost);
    assert_eq!(bubble_visibility(&s), (true, true));
}

#[test]
fn six_groups_are_all_present_in_the_order_of_the_plan() {
    let _guard = crate::desk_pet::test_guard();
    reset();
    let groups = build(&get(), true, &["bai".to_string()], 0);
    assert_eq!(groups.len(), 6);
    assert_eq!(
        labels(&groups),
        vec![
            "对话框显示 / 隐藏",
            "取消置顶",
            "回到 web 版本",
            "开启语音输入",
            "切换皮肤",
            "简易会话框",
        ]
    );
    // 不许回到旧版那五项。
    for forbidden in ["淡一点", "浓一点", "打开对话", "退出", "透明度"] {
        assert!(
            !groups.iter().any(|g| g.label.contains(forbidden)),
            "菜单里不该还有 {forbidden}"
        );
        assert!(
            !groups
                .iter()
                .flat_map(|g| g.children.iter())
                .any(|e| e.label.contains(forbidden))
        );
    }
}

#[test]
fn dialog_parent_off_hides_both_children_pixels() {
    let _guard = crate::desk_pet::test_guard();
    reset();
    apply(&Command::DialogToggle);
    let s = get();
    assert_eq!(bubble_visibility(&s), (false, false));
    // 父项关掉时两个子开关不可选（置灰），但「显示」总还能点回来。
    let groups = build(&s, false, &[], 0);
    assert_eq!(groups[0].children[0].label, "显示");
    assert!(!groups[0].children[1].enabled);
    assert!(!groups[0].children[2].enabled);
    apply(&Command::DialogToggle);
    assert_eq!(bubble_visibility(&get()), (true, true));
}

#[test]
fn two_bubble_switches_are_independent() {
    let _guard = crate::desk_pet::test_guard();
    reset();
    apply(&Command::UserToggle);
    assert_eq!(bubble_visibility(&get()), (false, true));
    apply(&Command::AiToggle);
    assert_eq!(bubble_visibility(&get()), (false, false));
    apply(&Command::UserToggle);
    assert_eq!(bubble_visibility(&get()), (true, false));
}

#[test]
fn voice_modes_are_mutually_exclusive_and_only_one_is_checked() {
    let _guard = crate::desk_pet::test_guard();
    reset();
    for mode in [VoiceMode::Always, VoiceMode::Hold, VoiceMode::Off] {
        apply(&Command::Voice(mode));
        let groups = build(&get(), false, &[], 0);
        let checked: Vec<usize> = groups[3]
            .children
            .iter()
            .filter(|e| e.checked)
            .map(|e| e.id)
            .collect();
        assert_eq!(checked, vec![mode.id()], "{mode:?} 只能自己打勾");
    }
    assert_eq!(get().voice, VoiceMode::Off);
}

#[test]
fn chat_topmost_is_independent_and_only_available_when_open() {
    let _guard = crate::desk_pet::test_guard();
    reset();
    let groups = build(&get(), true, &[], 0);
    let chat = &groups[5].children;
    assert!(!chat[1].enabled && !chat[2].enabled, "框没开时不可选");
    apply(&Command::ChatToggle);
    apply(&Command::ChatTopmost(true));
    let s = get();
    assert!(s.chat_open && s.chat_topmost);
    // 角色窗置顶与否不改这块小窗的状态：两者分别存。
    assert!(build(&s, false, &[], 0)[5].children[1].checked);
    assert!(build(&s, true, &[], 0)[5].children[1].checked);
}

#[test]
fn skin_entries_get_stable_ids_and_one_check() {
    let _guard = crate::desk_pet::test_guard();
    reset();
    let skins = vec!["bai".to_string(), "neko".to_string()];
    let groups = build(&get(), false, &skins, 1);
    let skin = &groups[4].children;
    assert_eq!(skin.len(), 2);
    assert_eq!(skin[0].id, ID_SKIN_BASE);
    assert_eq!(skin[1].id, ID_SKIN_BASE + 1);
    assert!(!skin[0].checked && skin[1].checked);
    assert_eq!(Command::from_id(ID_SKIN_BASE + 1), Some(Command::Skin(1)));
}

#[test]
fn every_built_command_id_round_trips() {
    let _guard = crate::desk_pet::test_guard();
    reset();
    let groups = build(&get(), true, &["bai".to_string()], 0);
    for group in groups {
        if let Some(id) = group.command {
            assert!(Command::from_id(id).is_some(), "组 {id} 没有命令");
        }
        for child in group.children {
            assert!(
                Command::from_id(child.id).is_some(),
                "子项 {child:?} 没有命令"
            );
        }
    }
    assert_eq!(Command::from_id(ID_BACK_TO_WEB), Some(Command::BackToWeb));
    assert_eq!(Command::from_id(999_999), None);
}

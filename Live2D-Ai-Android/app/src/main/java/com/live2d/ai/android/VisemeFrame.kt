package com.live2d.ai.android

// Copyright (c) 2026 Sakura Motion Project
// SPDX-License-Identifier: MIT
//
// mouthOpen/mouthForm 字段语义与取值取自 pymouth VOWEL_MAP
// （pymouth: MIT License, Copyright (c) 2024 organics,
//   https://github.com/organics2016/pymouth；详见 CREDITS.md §6.3）

/**
 * 一帧元音嘴型数据（M5 VisemeAnalyzer 输出）。
 *
 * mouthOpen/mouthForm：pymouth VOWEL_MAP 原值（保留——用于调参验证与 PC 端对等测试）；
 * paramA/I/U/E/O：五元音参数（由 §3.3 映射表从 vowel_label+mo/mf 计算，直接驱动模型；bai 无 ParamA/I/U/E/O，
 * 实际经 adapter.vowelParamMap 映射到 MouthOpenY/MouthForm），
 * 各 ∈ [0.0, 1.0]（计算后 coerceIn）。
 */
data class VisemeFrame(
    /** 窗口起点（相对本次 speak 起点，ms） */
    val timestampMs: Long,
    /** ∈ {A, I, U, E, O, SIL} */
    val vowelLabel: String,
    /** 0..1（pymouth VOWEL_MAP 原值） */
    val mouthOpen: Float,
    /** 0..1（pymouth VOWEL_MAP 原值） */
    val mouthForm: Float,
    /** 0..1 */
    val paramA: Float,
    /** 0..1 */
    val paramI: Float,
    /** 0..1 */
    val paramU: Float,
    /** 0..1 */
    val paramE: Float,
    /** 0..1 */
    val paramO: Float,
)

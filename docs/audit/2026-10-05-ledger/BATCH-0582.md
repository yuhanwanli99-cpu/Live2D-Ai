# BATCH-0582 落盘（极简）· parallel-mods/ 17 份，两处索引各收 3 份

## 命令（只读）
```
git ls-files docs/plans/parallel-mods/ | wc -l                              -> 17
grep -cE "\]\(plans/parallel-mods/" docs/README.md                          -> 3
grep -c "parallel-mods" AGENTS.md                                        -> 3
comm -23 <(basename 排序) <(README 收录 3 份)
```

## README 收录的 3 份
```
parallel-mods/PARALLEL-WAVE3-2026-09-14.md
parallel-mods/STABILIZE-CLOSEOUT.md
parallel-mods/WAVE3-CLOSEOUT-2026-09-14.md
```

## 未收录的 14 份
```
PARALLEL-PROTOCOL-2026-09-14.md
PARALLEL-WAVE2-2026-09-14.md
PLAN-persona-polish.md
PLAN-voice-input.md
PLAN-wallpaper.md
REGISTER-director-v0.md
REGISTER-external-input-v2.md
REGISTER-memory-v0.md
REGISTER-persona-polish.md
REGISTER-pet-desktop-v1.md
REGISTER-voice-input.md
REGISTER-voice-sidecar-v1.md
REGISTER-wallpaper-wire.md
REGISTER-wallpaper.md
```

## 三个可核点
1. **14 份里有 8 份是 REGISTER-* 前缀**（director-v0 / external-input-v2 /
   memory-v0 / persona-polish / pet-desktop-v1 / voice-input /
   voice-sidecar-v1 / wallpaper）。判据：**REGISTER-* 是「把某个 Mod 登记进册」的
   单件事记录**，一份 Mod 一份，所以前缀本身就是「这件事的主体」。
2. **两处索引各收 3 份、且收的是同 3 份**（PARALLEL-WAVE3 / STABILIZE-CLOSEOUT /
   WAVE3-CLOSEOUT）=> 覆盖率 3/17 = 18%。
   而 README 与 AGENTS 在这个子目录上**口径一致**（与 plans 根目录的 20% / 50% 不同）。
3. **未收录的 14 份里有 PARALLEL-PROTOCOL 与 PARALLEL-WAVE2** ——
   这两份正是 AGENTS 在 plans 根目录点名的那两份。
   => **⇒⇒⇒⇒** 判据：**同一个 basename 在子目录与根目录各有一份**，
   B0581 定的「按 basename 比」在这里**正好用上**，
   而 README 的 3 条用的是 `parallel-mods/` 前缀，AGENTS 的是裸名。

## 未核
REGISTER-director-v0.md 里 director 的登记状态（与 F-0637-01 对照）·
PLAN-persona-polish / PLAN-voice-input / PLAN-wallpaper 三份正文 ·
docs/audit/ 那 3 条

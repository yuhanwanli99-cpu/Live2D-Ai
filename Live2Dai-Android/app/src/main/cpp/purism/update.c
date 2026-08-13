/*
 * Purism Core: update pipeline
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <stdlib.h>
#include <string.h>
#include "private.h"
#include "debug.h"
#include "math2.h"
#include "moc3.h"
#include "model.h"
#include "update.h"
#include "param.h"
#include "part.h"
#include "interpolate.h"
#include "deformer.h"
#include "artmesh.h"
#include "glue.h"
#include "blendshape.h"
#include "offscreen.h"
#include "render.h"

/*
 * C-Perf7-fix: parameter-level dirty short-circuit (conservative).
 *
 * When no effective input changed since the previous psm__update_model
 * (every resolved parameter value and every clamped part opacity is bit-
 * identical to the saved snapshot), the ENTIRE vertex/mesh computation
 * chain is skipped and psm__update_model returns immediately. All output
 * buffers the renderer reads (art-mesh positions/opacities/colors/draw
 * order, render order, dynamic flags) keep the previous full update's
 * values, which are provably correct because every input feeding those
 * stages is unchanged.
 *
 * Correctness invariant (why "all or nothing" and no partial gating):
 * the skipped chain is one lockstep in-place accumulation system -
 *   - interp_* resets its outputs (memset + accumulate from keydata);
 *   - apply_transforms / apply_transforms_to_meshes transform positions
 *     in place on top of the freshly interpolated data;
 *   - apply_glues adds glue deltas to art-mesh positions in place
 *     (non-idempotent when intensity < 1);
 *   - apply_parts_to_meshes multiplies art-mesh opacity/colors in place;
 *   - apply_part_opacity multiplies offscreen opacity in place.
 * Running any subset of these without the rest would double-apply or
 * leave stale data (the previous implementation's per-stage gating did
 * exactly this and produced corrupted frames). Skipping the whole chain
 * together keeps every buffer identical to the previous frame.
 *
 * psm__reverse_y is part of the chain and MUST also be skipped: it
 * negates the y of every art-mesh vertex in place on each call (m->
 * y_reversed is a static canvas property, not a runtime latch), so on a
 * light frame it would flip the frozen (already flipped) vertices back,
 * corrupting the render - the root cause of the reverted bug.
 *
 * The cheap stages before the gate (save_flags, resolve params/key
 * tables/bindings, part opacity clamp) still run on every update; they
 * are deterministic functions of the unchanged inputs, so they produce
 * the same state as the last full update and are safe to run.
 *
 * First update / model switch / explicit force_update always take the
 * full path. The snapshot is saved at the end of each full update
 * (realloc-adaptive; on OOM the snapshot is invalidated and every update
 * falls back to the full path - safe, just slower).
 */
static struct psm__model *s_snap_model = NULL; /* model the snapshot belongs to */
static psm__f32 *s_snap_params = NULL;         /* resolved param value snapshot */
static psm__i32 s_snap_params_cap = 0;         /* capacity of s_snap_params */
static psm__f32 *s_snap_parts = NULL;          /* clamped part opacity snapshot */
static psm__i32 s_snap_parts_cap = 0;          /* capacity of s_snap_parts */
static bool s_snap_valid = false;              /* false until first snapshot save */

static bool
psm__inputs_changed(struct psm__model *m)
{
  if (m->force_update)
    return true;

  /* First update for this model (or model pointer switch): always full. */
  if (m != s_snap_model || !s_snap_valid)
    return true;

  /* Parameters: compare resolved values (post-clamp/wrap) exactly.
   * Values written by Kotlin are stored unchanged in input_value and
   * normalized by psm__resolve_params into items[i].value; when the
   * resolved value is bit-identical, the whole keydata/binding resolution
   * and every downstream stage would reproduce the same output. */
  struct psm__params *params = &m->params;
  if (params->count > 0) {
    if (!s_snap_params)
      return true;
    for (psm__i32 i = 0; i < params->count; i++) {
      if (params->items[i].value != s_snap_params[i])
        return true;
    }
  }

  /* Parts: clamped input opacities must match the saved snapshot. */
  psm__i32 nparts = m->parts.count;
  if (nparts > 0) {
    const psm__f32 *opa = m->parts.input_opacity;
    if (!s_snap_parts)
      return true;
    for (psm__i32 i = 0; i < nparts; i++) {
      if (opa[i] != s_snap_parts[i])
        return true;
    }
  }

  return false;
}

static void
psm__save_input_snapshot(struct psm__model *m)
{
  struct psm__params *params = &m->params;
  psm__i32 nparams = params->count;
  if (nparams > 0) {
    if (s_snap_params_cap < nparams) {
      psm__f32 *nw = (psm__f32 *)realloc(s_snap_params,
          nparams * sizeof(psm__f32));
      if (!nw) {
        /* OOM: fall back to always-full path (safe, just slower). */
        s_snap_params = NULL;
        s_snap_params_cap = 0;
        s_snap_valid = false;
        return;
      }
      s_snap_params = nw;
      s_snap_params_cap = nparams;
    }
    for (psm__i32 i = 0; i < nparams; i++)
      s_snap_params[i] = params->items[i].value;
  }

  psm__i32 nparts = m->parts.count;
  if (nparts > 0) {
    if (s_snap_parts_cap < nparts) {
      psm__f32 *nw = (psm__f32 *)realloc(s_snap_parts,
          nparts * sizeof(psm__f32));
      if (!nw) {
        s_snap_parts = NULL;
        s_snap_parts_cap = 0;
        s_snap_valid = false;
        return;
      }
      s_snap_parts = nw;
      s_snap_parts_cap = nparts;
    }
    memcpy(s_snap_parts, m->parts.input_opacity,
        nparts * sizeof(psm__f32));
  }
  s_snap_model = m;
  s_snap_valid = true;
}

#ifdef __ANDROID__
#include <android/log.h>
#include <time.h>
/*
 * C-Perf7-fix diagnostic probe: low-frequency counter (no clock reads),
 * one logcat line every 120 updates (~4s at 30Hz model updates).
 * C8b: extended with clock_gettime timing of the FULL path only (the path
 * that matters at 120Hz; light path is ~0.3ms and not the budget problem).
 * Cleanup: remove this block, the include above, and the two
 * psm__probe_update calls in psm__update_model.
 */
static psm__i32 s_probe_updates = 0;
static psm__i32 s_probe_light = 0;
static psm__i32 s_probe_full = 0;
static long long s_probe_full_us = 0; /* accumulated full-path microseconds */

static void
psm__probe_update(bool light, long long full_us)
{
  s_probe_updates++;
  if (light)
    s_probe_light++;
  else {
    s_probe_full++;
    s_probe_full_us += full_us;
  }
  if (s_probe_updates >= 120) {
    long long avg_full_us = s_probe_full > 0 ? s_probe_full_us / s_probe_full : 0;
    __android_log_print(ANDROID_LOG_INFO, "C-Perf7-fix",
        "updates=%d light=%d(%d%%) full=%d avg_full_us=%lld",
        s_probe_updates, s_probe_light,
        (int)(s_probe_light * 100 / s_probe_updates), s_probe_full,
        (long long)avg_full_us);
    s_probe_updates = 0;
    s_probe_light = 0;
    s_probe_full = 0;
    s_probe_full_us = 0;
  }
}

static long long
psm__now_us(void)
{
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (long long)ts.tv_sec * 1000000 + ts.tv_nsec / 1000;
}
#endif /* __ANDROID__ */

static void
psm__reverse_y(struct psm__model *m)
{
  if (m->y_reversed)
    return;

  psm__i32 count = m->art_meshes.count;
  if (count <= 0)
    return;

  struct psm__art_mesh *meshes = m->art_meshes.meshes;
  bool *en = m->art_meshes.enable;
  psm__f32 **pos = m->art_meshes.pos;

  for (psm__i32 i = 0; i < count; i++) {
    if (!en[i])
      continue;
    psm__i32 vc = meshes[i].vertex_count;
    if (vc <= 0)
      continue;
    psm__f32 *p = pos[i];
    for (psm__i32 j = 0; j < vc; j++)
      p[2 * j + 1] = -p[2 * j + 1];
  }
}

static void
psm__save_flags(struct psm__model *m)
{
  struct psm__art_meshes *am = &m->art_meshes;
  if (!am->state_changed)
    return;

  psm__u8  ver = m->source->header->version;
  psm__i32 n   = am->count;

  memcpy(am->last_render_order, m->render_order, sizeof(psm__i32) * n);
  memcpy(am->last_draw_order, am->draw_order, sizeof(psm__i32) * n);
  memcpy(am->last_opacity, am->opacity, sizeof(psm__f32) * n);

  if (ver < csmMocVersion_42)
    return;

  memcpy(am->last_mul_color, am->mul_color, 4 * sizeof(psm__f32) * n);
  memcpy(am->last_scr_color, am->scr_color, 4 * sizeof(psm__f32) * n);
}

static void
psm__update_flags(struct psm__model *m)
{
  struct psm__art_meshes *am = &m->art_meshes;
  psm__u8  ver = m->source->header->version;
  psm__i32 n   = am->count;

  /* Force update: mark everything dirty */
  if (m->force_update) {
    am->state_changed = 0;
    if (n <= 0)
      return;

    bool *en = am->enable;
    psm__f32 *opacity = am->opacity;
    psm__u8 *df = am->change_flags;

    for (psm__i32 i = 0; i < n; i++) {
      if (!en[i] || opacity[i] == 0.0f)
        df[i] = PSM__FLAG_ALL_CHANGED;
      else
        df[i] = PSM__FLAG_ALL;
    }
    return;
  }

  /* Normal dirty update: compare previous vs current */
  if (am->state_changed) {
    am->state_changed = 0;
    if (n <= 0)
      return;

    bool *en = am->enable;
    psm__f32 *opacity = am->opacity;
    psm__u8  *df      = am->change_flags;
    psm__i32 *ro      = m->render_order;
    psm__i32 *last_ro = am->last_render_order;
    psm__i32 *cdo     = am->draw_order;
    psm__i32 *last_do = am->last_draw_order;
    psm__f32 *last_op = am->last_opacity;

    psm__f32 *mc = NULL, *lmc = NULL;
    psm__f32 *sc = NULL, *lsc = NULL;
    if (ver >= csmMocVersion_42) {
      mc  = am->mul_color;
      lmc = am->last_mul_color;
      sc  = am->scr_color;
      lsc = am->last_scr_color;
    }

    for (psm__i32 i = 0; i < n; i++) {
      psm__i32 visible = en[i] && opacity[i] != 0.0f;
      psm__i32 was = (df[i] & PSM__FLAG_IS_VISIBLE) != 0;
      psm__u8 flags = visible;

      if (visible != was)
        flags |= PSM__FLAG_VISIBILITY_CHANGED;
      if (opacity[i] != last_op[i])
        flags |= PSM__FLAG_OPACITY_CHANGED;
      if (cdo[i] != last_do[i])
        flags |= PSM__FLAG_DRAW_ORDER_CHANGED;
      if (ro[i] != last_ro[i])
        flags |= PSM__FLAG_RENDER_ORDER_CHANGED;
      if (en[i])
        flags |= PSM__FLAG_VERTEX_CHANGED;

      if (mc && (memcmp(&mc[i * 4], &lmc[i * 4], 16) != 0
             ||  memcmp(&sc[i * 4], &lsc[i * 4], 16) != 0))
        flags |= PSM__FLAG_BLEND_COLOR_CHANGED;

      df[i] = flags;
    }
    return;
  }

  /* No dirty update: just refresh visibility bit */
  if (n <= 0)
    return;

  bool *en = am->enable;
  psm__f32 *opacity = am->opacity;
  psm__u8 *df = am->change_flags;

  for (psm__i32 i = 0; i < n; i++) {
    if (!en[i] || opacity[i] == 0.0f)
      df[i] &= ~PSM__FLAG_IS_VISIBLE;
    else
      df[i] |= PSM__FLAG_IS_VISIBLE;
  }
}

PSM__DEF void
psm__update_model(struct psm__model *m)
{
  psm__save_flags(m);

  psm__resolve_params(&m->params);
  psm__resolve_key_tables(m);
  psm__resolve_blend_key_tables(m);
  psm__resolve_bindings(m);
  psm__resolve_blend_bindings(m);

  psm__i32 nparts = m->parts.count;
  if (nparts > 0) {
    psm__f32 *opa = m->parts.input_opacity;
    for (psm__i32 i = 0; i < nparts; i++)
      opa[i] = psm__clamp_f32_01(opa[i]);
  }

  /* C-Perf7-fix: when no input changed, freeze the whole vertex/mesh
   * chain and return. Every output buffer the renderer reads still holds
   * the last full update's values (correct and consistent). No stage is
   * partially executed. */
  if (!psm__inputs_changed(m)) {
    m->last_update_light = true;
#ifdef __ANDROID__
    psm__probe_update(true, 0);
#endif
    return;
  }
  m->last_update_light = false;

#ifdef __ANDROID__
  long long t_full0 = psm__now_us();
#endif

  psm__enable_parts(m);
  psm__gather_parts(m);
  psm__interp_parts(m);

  psm__enable_deformers(m);
  psm__gather_warps(m);
  psm__gather_rotations(m);
  psm__interp_warps(m);
  psm__interp_rotations(m);

  psm__enable_art_meshes(m);
  psm__gather_art_meshes(m);
  psm__interp_art_meshes(m);

  psm__gather_glues(m);
  psm__interp_glues(m);

  psm__enable_offscreens(m);
  psm__gather_offscreens(m);
  psm__interp_offscreens(m);

  psm__blend_parts(m);
  psm__blend_warps(m);
  psm__blend_rotations(m);
  psm__blend_art_meshes(m);
  psm__blend_glues(m);
  psm__blend_offscreens(m);

  psm__apply_transforms(m);
  psm__apply_transforms_to_meshes(m);
  psm__apply_part_opacity(m);
  psm__apply_parts_to_meshes(m);

  psm__apply_glues(m); /* must come before reverse_y! */

  psm__reverse_y(m);
  psm__sort_render_order(m);
  psm__update_flags(m);

  /* v6+: zero opacity for disabled offscreen surfaces */
  if (m->source->header->version >= csmMocVersion_53) {
    psm__i32 oc = m->offscreens.count;
    bool *en = m->offscreens.enable;
    psm__f32 *opa = m->offscreens.opacity;
    if (oc > 0 && en && opa) {
      for (psm__i32 i = 0; i < oc; i++) {
        if (!en[i])
          opa[i] = 0.0f;
      }
    }
  }

  m->force_update = 0;

  /* C-Perf7-fix: remember the effective inputs (resolved param values +
   * clamped part opacities) for the next update's dirty check. */
  psm__save_input_snapshot(m);

#ifdef __ANDROID__
  psm__probe_update(false, psm__now_us() - t_full0);
#endif
}

PSMDEF void
csmUpdateModel(csmModel *model)
{
  psm__update_model((struct psm__model *)model);
}

/* C8b: query whether the most recent csmUpdateModel took the light path
 * (no input changed → frozen vertices). Lets the renderer skip the per-
 * drawable vertex re-fetch/re-upload on unchanged frames. */
PSMDEF int
csmIsLastUpdateLight(csmModel *model)
{
  struct psm__model *m = (struct psm__model *)model;
  return m->last_update_light ? 1 : 0;
}

PSMDEF void
csmResetDrawableDynamicFlags(csmModel *model)
{
  struct psm__model *m = (struct psm__model *)model;
  psm__i32 count = m->art_meshes.count;
  psm__u8 *flags = m->art_meshes.change_flags;
  for (psm__i32 i = 0; i < count; i++)
    flags[i] &= PSM__FLAG_IS_VISIBLE;
  m->art_meshes.state_changed = 1;
}

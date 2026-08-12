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

#ifdef __ANDROID__
#include <android/log.h>
#include <time.h>
#endif

/*
 * C-Perf7: parameter-level dirty short-circuit.
 *
 * When no effective input changed since the previous psm__update_model
 * (every resolved parameter value identical — tracked by the param->dirty
 * flags recomputed in psm__resolve_params — and every clamped part opacity
 * identical to the saved snapshot), the deformer/art-mesh keydata
 * interpolation and the warp transforms are skipped: their output buffers
 * (warp grid points, art-mesh vertices, opacities, colors) stay frozen at
 * the previous update's values, which are provably identical because every
 * input feeding those stages is unchanged.
 *
 * Correctness invariant: the skipped stages are exactly the interp -> blend
 * -> apply chain of warps/rotations/art-meshes plus apply_parts_to_meshes
 * (which multiplies art-mesh opacity in place and therefore must not run
 * without a fresh interp_art_meshes reset) and apply_glues (which mutates
 * mesh positions in place on top of the freshly transformed vertices and is
 * non-idempotent when the glue intensity < 1). Skipping both sides of each
 * in-place accumulation keeps every buffer equal to the previous frame.
 * All cheap stages (resolve, enable, gather, parts, glues, offscreens,
 * blend_parts/blend_glues/blend_offscreens, apply_part_opacity,
 * reverse_y, sort_render_order, update_flags) still run on every update,
 * so dynamic flags / render order / visibility semantics are identical to
 * the full path.
 */
static struct psm__model *s_snap_model = NULL; /* model the snapshot belongs to */
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

  /* Parameters: psm__resolve_params already compared each resolved value
   * against the previous update and set param->dirty accordingly. */
  struct psm__params *params = &m->params;
  for (psm__i32 i = 0; i < params->count; i++) {
    if (params->items[i].dirty)
      return true;
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
  psm__i32 nparts = m->parts.count;
  if (nparts > 0) {
    if (s_snap_parts_cap < nparts) {
      psm__f32 *nw = (psm__f32 *)realloc(s_snap_parts,
          nparts * sizeof(psm__f32));
      if (!nw) {
        /* OOM: fall back to always-full path (safe, just slower). */
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
/* C-Perf7 probe: light/full update counters + per-path average update time.
 * One logcat line per 120 updates (~4 s at 30 Hz model update). Overhead is
 * one CLOCK_MONOTONIC read per update + one log line per 120 updates (same
 * class as the maskCounts 30-frame beacon). Remove after measurement. */
static void
psm__probe_update(bool light, long long elapsed_ns)
{
  static long long light_ns = 0, full_ns = 0;
  static int light_n = 0, full_n = 0, total = 0;
  if (light) {
    light_n++;
    light_ns += elapsed_ns;
  } else {
    full_n++;
    full_ns += elapsed_ns;
  }
  if (++total >= 120) {
    __android_log_print(ANDROID_LOG_INFO, "C-Perf7",
        "updates=%d light=%d(%.0f%%) full=%d avg_light=%.3fms avg_full=%.3fms",
        total, light_n, 100.0f * light_n / total, full_n,
        light_n ? (double)light_ns / light_n / 1e6 : 0.0,
        full_n ? (double)full_ns / full_n / 1e6 : 0.0);
    light_n = full_n = total = 0;
    light_ns = full_ns = 0;
  }
}
#endif

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

  /* C-Perf7: short-circuit the heavy deformer stages when no input changed.
   * All cheap stages below run in both paths; the gated stages are skipped
   * in the light path and their output buffers stay frozen (see the block
   * comment at the top of this file). */
  bool inputs_changed = psm__inputs_changed(m);
#ifdef __ANDROID__
  struct timespec ts0, ts1;
  clock_gettime(CLOCK_MONOTONIC, &ts0);
#endif

  psm__enable_parts(m);
  psm__gather_parts(m);
  psm__interp_parts(m);

  psm__enable_deformers(m);
  psm__gather_warps(m);
  psm__gather_rotations(m);
  if (inputs_changed) {
    psm__interp_warps(m);
    psm__interp_rotations(m);
  }

  psm__enable_art_meshes(m);
  psm__gather_art_meshes(m);
  if (inputs_changed) {
    psm__interp_art_meshes(m);
  }

  psm__gather_glues(m);
  psm__interp_glues(m);

  psm__enable_offscreens(m);
  psm__gather_offscreens(m);
  psm__interp_offscreens(m);

  psm__blend_parts(m);
  if (inputs_changed) {
    psm__blend_warps(m);
    psm__blend_rotations(m);
    psm__blend_art_meshes(m);
  }
  psm__blend_glues(m);
  psm__blend_offscreens(m);

  if (inputs_changed) {
    psm__apply_transforms(m);
    psm__apply_transforms_to_meshes(m);
  }
  psm__apply_part_opacity(m);
  if (inputs_changed) {
    psm__apply_parts_to_meshes(m);
  }

  if (inputs_changed) {
    psm__apply_glues(m); /* must come before reverse_y! */
  }

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

  /* C-Perf7: remember the effective inputs for next update's dirty check.
   * Parameter values are covered by resolve_params' own dirty comparison;
   * only the clamped part opacities need an explicit snapshot. */
  psm__save_input_snapshot(m);

#ifdef __ANDROID__
  clock_gettime(CLOCK_MONOTONIC, &ts1);
  psm__probe_update(!inputs_changed,
      (long long)(ts1.tv_sec - ts0.tv_sec) * 1000000000LL +
      (long long)(ts1.tv_nsec - ts0.tv_nsec));
#endif
}

PSMDEF void
csmUpdateModel(csmModel *model)
{
  psm__update_model((struct psm__model *)model);
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

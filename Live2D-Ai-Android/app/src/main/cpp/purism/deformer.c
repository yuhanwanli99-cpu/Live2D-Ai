/*
 * Purism Core: warp and rotation deformer transforms
 *
 * Copyright (c) 2026 Sakura Motion Project
 * SPDX-License-Identifier: MIT
 */

#include <math.h>
#include <string.h>
#include "private.h"
#include "array.h"
#include "debug.h"
#include "deformer.h"
#include "gather.h"
#include "math2.h"
#include "moc3.h"
#include "model.h"

/*
 * C-Perf9: arm64 NEON vectorization of the hot point-transform loops.
 * Guarded by __aarch64__ so x86/32-bit builds keep the scalar paths.
 * PSM__NEON_DISABLED forces the scalar path (A/B harness only).
 * Each NEON lane computes one point; the arithmetic mirrors the scalar
 * fmaf() structure exactly, so per-point results are bit-identical to
 * the scalar code (NEON fmla == C fmaf on aarch64).
 */
#if defined(__aarch64__) && !defined(PSM__NEON_DISABLED)
#include <arm_neon.h>
#endif

struct psm__warp_basis {
  struct psm__vec2 center;
  struct psm__vec2 dpdu;
  struct psm__vec2 dpdv;
};

struct psm__warp_cell {
  psm__f32 fu, fv;
  struct psm__vec2 p00, p10, p01, p11;
};

static inline struct psm__warp_basis
psm__warp_extrap_basis(const psm__f32 *pos, psm__i32 row, psm__i32 col,
    psm__i32 stride)
{
  struct psm__vec2 c00 = psm__v2(pos[0], pos[1]);
  struct psm__vec2 c10 = psm__v2_load(pos, col);
  struct psm__vec2 c01 = psm__v2_load(pos, row * stride);
  struct psm__vec2 c11 = psm__v2_load(pos, row * stride + col);

  struct psm__vec2 d11_00 = psm__v2_sub(c11, c00);
  struct psm__vec2 d10_01 = psm__v2_sub(c10, c01);

  struct psm__warp_basis b;
  b.dpdu = psm__v2_scale(psm__v2_sub(d11_00, d10_01), 0.5f);
  b.dpdv = psm__v2_scale(psm__v2_add(d10_01, d11_00), 0.5f);

  struct psm__vec2 sum = psm__v2_add(
      psm__v2_add(c00, c10), psm__v2_add(c01, c11));
  b.center = psm__v2_sub(
      psm__v2_scale(sum, 0.25f), psm__v2_scale(d11_00, 0.5f));

  return b;
}

static inline struct psm__warp_cell
psm__warp_extrap_cell(psm__f32 u, psm__f32 v, psm__f32 gu, psm__f32 gv,
    psm__i32 row, psm__i32 col, psm__i32 stride, const psm__f32 *pos,
    const struct psm__warp_basis *basis)
{
  psm__f32 fr = (psm__f32)row, fc = (psm__f32)col;
  struct psm__vec2 cen = basis->center;
  struct psm__vec2 du = basis->dpdu;
  struct psm__vec2 dv = basis->dpdv;

  struct psm__warp_cell cell;

  if (u <= 0.0f) {
    if (v <= 0.0f) {
      cell.fu = (u + 2.0f) * 0.5f;
      cell.fv = (v + 2.0f) * 0.5f;
      cell.p00 = psm__v2_sub(cen, psm__v2_add(
          psm__v2_scale(du, 2.0f), psm__v2_scale(dv, 2.0f)));
      cell.p10 = psm__v2_sub(cen, psm__v2_scale(du, 2.0f));
      cell.p01 = psm__v2_sub(cen, psm__v2_scale(dv, 2.0f));
      cell.p11 = psm__v2(pos[0], pos[1]);
    } else if (v < 1.0f) {
      psm__i32 cv = (psm__i32)gv;
      if (cv == row) cv = row - 1;
      cell.fv = gv - (psm__f32)cv;
      cell.fu = (u + 2.0f) * 0.5f;
      psm__f32 vc = (psm__f32)cv / fr;
      psm__f32 vn = (psm__f32)(cv + 1) / fr;
      cell.p00 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(dv, 2.0f)),
          psm__v2_scale(du, vc));
      cell.p10 = psm__v2_load(pos, cv * stride);
      cell.p01 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(dv, 2.0f)),
          psm__v2_scale(du, vn));
      cell.p11 = psm__v2_load(pos, (cv + 1) * stride);
    } else {
      cell.fu = (u + 2.0f) * 0.5f;
      cell.fv = (v - 1.0f) * 0.5f;
      cell.p00 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(dv, 2.0f)), du);
      cell.p10 = psm__v2_load(pos, row * stride);
      cell.p01 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(dv, 2.0f)),
          psm__v2_scale(du, 3.0f));
      cell.p11 = psm__v2_add(cen, psm__v2_scale(du, 3.0f));
    }
  } else if (u < 1.0f) {
    psm__i32 cu = (psm__i32)gu;
    if (cu == col) cu = col - 1;
    cell.fu = gu - (psm__f32)cu;
    psm__f32 uc = (psm__f32)cu / fc;
    psm__f32 un = (psm__f32)(cu + 1) / fc;
    if (v <= 0.0f) {
      cell.fv = (v + 2.0f) * 0.5f;
      cell.p00 = psm__v2_add(psm__v2_scale(dv, uc),
          psm__v2_sub(cen, psm__v2_scale(du, 2.0f)));
      cell.p10 = psm__v2_add(psm__v2_scale(dv, un),
          psm__v2_sub(cen, psm__v2_scale(du, 2.0f)));
      cell.p01 = psm__v2_load(pos, cu);
      cell.p11 = psm__v2_load(pos, cu + 1);
    } else {
      cell.fv = (v - 1.0f) * 0.5f;
      cell.p00 = psm__v2_load(pos, row * stride + cu);
      cell.p10 = psm__v2_load(pos, row * stride + cu + 1);
      cell.p01 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(dv, uc)),
          psm__v2_scale(du, 3.0f));
      cell.p11 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(dv, un)),
          psm__v2_scale(du, 3.0f));
    }
  } else {
    if (v <= 0.0f) {
      cell.fu = (u - 1.0f) * 0.5f;
      cell.fv = (v + 2.0f) * 0.5f;
      cell.p00 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(du, 2.0f)), dv);
      cell.p10 = psm__v2_add(psm__v2_sub(cen, psm__v2_scale(du, 2.0f)),
          psm__v2_scale(dv, 3.0f));
      cell.p01 = psm__v2_load(pos, col);
      cell.p11 = psm__v2_add(cen, psm__v2_scale(dv, 3.0f));
    } else if (v < 1.0f) {
      psm__i32 cv = (psm__i32)gv;
      if (cv == row) cv = row - 1;
      cell.fv = gv - (psm__f32)cv;
      cell.fu = (u - 1.0f) * 0.5f;
      psm__f32 vc = (psm__f32)cv / fr;
      psm__f32 vn = (psm__f32)(cv + 1) / fr;
      cell.p00 = psm__v2_load(pos, col + cv * stride);
      cell.p10 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(dv, 3.0f)),
          psm__v2_scale(du, vc));
      cell.p01 = psm__v2_load(pos, col + (cv + 1) * stride);
      cell.p11 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(dv, 3.0f)),
          psm__v2_scale(du, vn));
    } else {
      cell.fu = (u - 1.0f) * 0.5f;
      cell.fv = (v - 1.0f) * 0.5f;
      cell.p00 = psm__v2_load(pos, row * stride + col);
      cell.p10 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(dv, 3.0f)), du);
      cell.p01 = psm__v2_add(psm__v2_add(cen, psm__v2_scale(du, 3.0f)), dv);
      cell.p11 = psm__v2_add(cen,
          psm__v2_add(psm__v2_scale(dv, 3.0f), psm__v2_scale(du, 3.0f)));
    }
  }

  return cell;
}

static inline struct psm__vec2
psm__triangle_interpolate(const struct psm__warp_cell *cell)
{
  psm__f32 fu = cell->fu, fv = cell->fv;
  if (fu + fv <= 1.0f) {
    psm__f32 w00 = 1.0f - fu - fv;
    return psm__v2_bary3(cell->p00, cell->p10, cell->p01, w00, fu, fv);
  } else {
    psm__f32 w10 = 1.0f - fv;
    psm__f32 w11 = fu + fv - 1.0f;
    psm__f32 w01 = 1.0f - fu;
    return psm__v2_bary3(cell->p10, cell->p11, cell->p01, w10, w11, w01);
  }
}

#if defined(__aarch64__) && !defined(PSM__NEON_DISABLED)
/*
 * NEON fast path for psm__warp_transform: processes 4 consecutive points
 * at once when all 4 are interior (0 <= u < 1 && 0 <= v < 1). Returns 1
 * if the batch was handled, 0 if the caller must fall back to the scalar
 * per-point path. The cell corners are gathered with one full-width load
 * per cell-row per lane followed by a 4x4 lane transpose (no contiguity
 * assumption; any cell position works).
 *
 * Semantics: identical branch classification to the scalar interior test;
 * the bilinear/triangle arithmetic mirrors psm__v2_bilinear / bary3 with
 * the same fmaf nesting, so each lane reproduces the scalar result
 * bit-for-bit. Non-interior points are never touched here.
 */
static int
psm__warp_neon_batch(const psm__f32 *inputs, const psm__f32 *pos,
    psm__f32 *outputs, psm__i32 base, psm__f32 fc, psm__f32 fr,
    psm__i32 stride, psm__i32 is_quad)
{
  float32x4_t a = vld1q_f32(inputs + 2 * base);      /* {x0,y0,x1,y1} */
  float32x4_t b = vld1q_f32(inputs + 2 * base + 4);  /* {x2,y2,x3,y3} */
  float32x4x2_t t = vuzpq_f32(a, b);
  float32x4_t x = t.val[0]; /* {x0,x1,x2,x3} */
  float32x4_t y = t.val[1]; /* {y0,y1,y2,y3} */

  /* Interior test for all 4 lanes: 0 <= x < 1 && 0 <= y < 1. */
  float32x4_t zero = vdupq_n_f32(0.0f);
  float32x4_t one = vdupq_n_f32(1.0f);
  uint32x4_t m = vcgeq_f32(x, zero);
  m = vandq_u32(m, vcltq_f32(x, one));
  m = vandq_u32(m, vcgeq_f32(y, zero));
  m = vandq_u32(m, vcltq_f32(y, one));
  if (vminvq_u32(m) != 0xFFFFFFFFu)
    return 0;

  float32x4_t gu = vmulq_n_f32(x, fc);
  float32x4_t gv = vmulq_n_f32(y, fr);
  /* C cast float->int truncates toward zero; vcvtq_s32_f32 does the same. */
  int32x4_t cu = vcvtq_s32_f32(gu);
  int32x4_t cv = vcvtq_s32_f32(gv);
  float32x4_t fu = vsubq_f32(gu, vcvtq_f32_s32(cu));
  float32x4_t fv = vsubq_f32(gv, vcvtq_f32_s32(cv));

  int32x4_t bi = vaddq_s32(vmulq_n_s32(cv, stride), cu);
  int32_t bi0 = vgetq_lane_s32(bi, 0);
  int32_t bi1 = vgetq_lane_s32(bi, 1);
  int32_t bi2 = vgetq_lane_s32(bi, 2);
  int32_t bi3 = vgetq_lane_s32(bi, 3);

  /* Gather the 4 cell corners (p00,p10,p01,p11) for each of the 4 points.
   * One full-width load per cell-row per lane (2 points = 4 floats),
   * then a 4x4 lane transpose — 8 vld1q instead of 32 vld1q_lane. */
  float32x4_t q0 = vld1q_f32(pos + 2 * bi0);  /* {p00x0,p00y0,p10x0,p10y0} */
  float32x4_t q1 = vld1q_f32(pos + 2 * bi1);
  float32x4_t q2 = vld1q_f32(pos + 2 * bi2);
  float32x4_t q3 = vld1q_f32(pos + 2 * bi3);
  float32x4_t r0 = vld1q_f32(pos + 2 * bi0 + 2 * stride); /* {p01x,p01y,p11x,p11y} */
  float32x4_t r1 = vld1q_f32(pos + 2 * bi1 + 2 * stride);
  float32x4_t r2 = vld1q_f32(pos + 2 * bi2 + 2 * stride);
  float32x4_t r3 = vld1q_f32(pos + 2 * bi3 + 2 * stride);

  /* Cross-pair lanes (0,2) and (1,3), then zip even/odd into per-corner
   * lane vectors: p00x = {p00x0,p00x1,p00x2,p00x3}, etc. */
  float32x4x2_t u0 = vtrnq_f32(q0, q2);
  float32x4x2_t u1 = vtrnq_f32(q1, q3);
  float32x4x2_t s0 = vtrnq_f32(r0, r2);
  float32x4x2_t s1 = vtrnq_f32(r1, r3);
  float32x4_t p00x = vzip1q_f32(u0.val[0], u1.val[0]);
  float32x4_t p10x = vzip2q_f32(u0.val[0], u1.val[0]);
  float32x4_t p00y = vzip1q_f32(u0.val[1], u1.val[1]);
  float32x4_t p10y = vzip2q_f32(u0.val[1], u1.val[1]);
  float32x4_t p01x = vzip1q_f32(s0.val[0], s1.val[0]);
  float32x4_t p11x = vzip2q_f32(s0.val[0], s1.val[0]);
  float32x4_t p01y = vzip1q_f32(s0.val[1], s1.val[1]);
  float32x4_t p11y = vzip2q_f32(s0.val[1], s1.val[1]);

  float32x4_t rx, ry;
  if (is_quad) {
    /* psm__v2_bilinear, same fmaf nesting per lane. */
    float32x4_t inv_u = vsubq_f32(one, fu);
    float32x4_t inv_v = vsubq_f32(one, fv);
    float32x4_t x0 = vfmaq_f32(vmulq_f32(inv_u, p00x), fu, p10x);
    float32x4_t x1 = vfmaq_f32(vmulq_f32(inv_u, p01x), fu, p11x);
    float32x4_t y0 = vfmaq_f32(vmulq_f32(inv_u, p00y), fu, p10y);
    float32x4_t y1 = vfmaq_f32(vmulq_f32(inv_u, p01y), fu, p11y);
    rx = vfmaq_f32(vmulq_f32(inv_v, x0), fv, x1);
    ry = vfmaq_f32(vmulq_f32(inv_v, y0), fv, y1);
  } else {
    /* psm__triangle_interpolate: compute both triangle cases, select by
     * the (fu + fv <= 1) mask — identical to the scalar branch outcome. */
    uint32x4_t sel = vcleq_f32(vaddq_f32(fu, fv), one);
    float32x4_t w00 = vsubq_f32(vsubq_f32(one, fu), fv);
    float32x4_t w10 = vsubq_f32(one, fv);
    float32x4_t w11 = vsubq_f32(vaddq_f32(fu, fv), one);
    float32x4_t w01 = vsubq_f32(one, fu);
    float32x4_t ax = vfmaq_f32(vfmaq_f32(vmulq_f32(w00, p00x), fu, p10x),
        fv, p01x);
    float32x4_t ay = vfmaq_f32(vfmaq_f32(vmulq_f32(w00, p00y), fu, p10y),
        fv, p01y);
    float32x4_t bx = vfmaq_f32(vfmaq_f32(vmulq_f32(w10, p10x), w11, p11x),
        w01, p01x);
    float32x4_t by = vfmaq_f32(vfmaq_f32(vmulq_f32(w10, p10y), w11, p11y),
        w01, p01y);
    rx = vbslq_f32(sel, ax, bx);
    ry = vbslq_f32(sel, ay, by);
  }

  /* Re-interleave to {x0,y0,x1,y1} and {x2,y2,x3,y3} and store. */
  float32x4x2_t z = vzipq_f32(rx, ry);
  vst1q_f32(outputs + 2 * base, z.val[0]);
  vst1q_f32(outputs + 2 * base + 4, z.val[1]);
  return 1;
}
#endif /* __aarch64__ */

static void
psm__warp_transform(struct psm__model *m, psm__i32 di,
    const psm__f32 *inputs, psm__f32 *outputs, psm__i32 count)
{
  struct psm__deformer_node *dn = m->deformers.nodes;
  psm__i32 si = dn[di].local_idx;
  struct psm__warp *wc = &m->deformers.warps.items[si];
  psm__f32 *pos = m->deformers.warps.pos[si];

  psm__i32 row = wc->row, col = wc->col;
  psm__i32 is_quad = wc->quad_transform, stride = col + 1;
  psm__f32 fr = (psm__f32)row, fc = (psm__f32)col;

  psm__i32 extrap_setup = 0;
  struct psm__warp_basis basis;

  for (psm__i32 i = 0; i < count; i++) {
#if defined(__aarch64__) && !defined(PSM__NEON_DISABLED)
    if (i + 4 <= count) {
      if (psm__warp_neon_batch(inputs, pos, outputs, i, fc, fr, stride,
              is_quad)) {
        i += 3;
        continue;
      }
    }
#endif
    struct psm__vec2 uv = psm__v2_load(inputs, i);
    psm__f32 gu = uv.x * fc, gv = uv.y * fr;

    if (uv.x >= 0.0f && uv.x < 1.0f && uv.y >= 0.0f && uv.y < 1.0f) {
      /* Interior: interpolate within grid cell */
      psm__i32 cu = (psm__i32)gu, cv = (psm__i32)gv;
      psm__f32 fu = gu - (psm__f32)cu;
      psm__f32 fv = gv - (psm__f32)cv;

      psm__i32 bi = cv * stride + cu;
      struct psm__vec2 p00 = psm__v2_load(pos, bi);
      struct psm__vec2 p10 = psm__v2_load(pos, bi + 1);
      struct psm__vec2 p01 = psm__v2_load(pos, bi + stride);
      struct psm__vec2 p11 = psm__v2_load(pos, bi + stride + 1);

      struct psm__vec2 result;
      if (is_quad) {
        result = psm__v2_bilinear(p00, p10, p01, p11, fu, fv);
      } else {
        struct psm__warp_cell cell = {fu, fv, p00, p10, p01, p11};
        result = psm__triangle_interpolate(&cell);
      }
      psm__v2_store(outputs, i, result);
    } else {
      /* Extrapolation: compute basis if needed */
      if (!extrap_setup) {
        basis = psm__warp_extrap_basis(pos, row, col, stride);
        extrap_setup = 1;
      }

      if (uv.x > -2.0f && uv.x < 3.0f && uv.y > -2.0f && uv.y < 3.0f) {
        /* Near-exterior: virtual cell + triangle */
        struct psm__warp_cell cell = psm__warp_extrap_cell(uv.x, uv.y, gu, gv,
            row, col, stride, pos, &basis);
        struct psm__vec2 r = psm__triangle_interpolate(&cell);
        psm__v2_store(outputs, i, r);
      } else {
        /* Far-exterior: simple affine */
        psm__f32 rx = basis.dpdv.x * uv.x
            + basis.center.x + basis.dpdu.x * uv.y;
        psm__f32 ry = basis.dpdv.y * uv.x
            + basis.center.y + basis.dpdu.y * uv.y;
        outputs[i * 2] = rx;
        outputs[i * 2 + 1] = ry;
      }
    }
  }
}

static void
psm__rot_transform(struct psm__model *m, psm__i32 di,
    const psm__f32 *inputs, psm__f32 *outputs, psm__i32 count)
{
  psm__i32 si = m->deformers.nodes[di].local_idx;
  struct psm__rotation *rc = &m->deformers.rotations.items[si];

  psm__f32 base_angle = rc->base_angle;
  psm__f32 angle = m->deformers.rotations.angle[si];
  psm__f32 scale = m->deformers.rotations.scale[si];
  struct psm__vec2 origin = psm__v2(m->deformers.rotations.origin_x[si],
      m->deformers.rotations.origin_y[si]);
  psm__i32 rx = m->deformers.rotations.reflect_x[si];
  psm__i32 ry = m->deformers.rotations.reflect_y[si];

  psm__f32 angle_rad = (base_angle + angle) * PSM__PI / 180.0f;
  psm__f32 sin_a = sinf(angle_rad);
  psm__f32 cos_a = cosf(angle_rad);

  psm__f32 rxf = rx ? -1.0f : 1.0f;
  psm__f32 ryf = ry ? -1.0f : 1.0f;

  psm__f32 m00 = scale * cos_a * rxf;
  psm__f32 m01 = scale * (-sin_a) * ryf;
  psm__f32 m10 = scale * sin_a * rxf;
  psm__f32 m11 = scale * cos_a * ryf;

#if defined(__aarch64__) && !defined(PSM__NEON_DISABLED)
  {
    /* 2 points per iteration (4 floats): pure per-point 2D affine,
     * vfmaq == fmaf on aarch64, so results are bit-identical. */
    float32x4_t v_m00 = vdupq_n_f32(m00);
    float32x4_t v_m01 = vdupq_n_f32(m01);
    float32x4_t v_m10 = vdupq_n_f32(m10);
    float32x4_t v_m11 = vdupq_n_f32(m11);
    float32x4_t v_ox = vdupq_n_f32(origin.x);
    float32x4_t v_oy = vdupq_n_f32(origin.y);

    psm__i32 i = 0;
    for (; i + 2 <= count; i += 2) {
      float32x4_t p = vld1q_f32(inputs + 2 * i); /* {x0,y0,x1,y1} */
      float32x4x2_t xy = vuzpq_f32(p, p);
      float32x4_t x = xy.val[0]; /* {x0,x1,x0,x1} */
      float32x4_t y = xy.val[1]; /* {y0,y1,y0,y1} */
      float32x4_t rx = vfmaq_f32(vfmaq_f32(v_ox, v_m00, x), v_m01, y);
      float32x4_t ry = vfmaq_f32(vfmaq_f32(v_oy, v_m10, x), v_m11, y);
      float32x4x2_t z = vzipq_f32(rx, ry);
      vst1q_f32(outputs + 2 * i, z.val[0]);
    }
    for (; i < count; i++) {
      struct psm__vec2 p = psm__v2_load(inputs, i);
      struct psm__vec2 r = psm__v2(
          fmaf(m01, p.y, fmaf(m00, p.x, origin.x)),
          fmaf(m11, p.y, fmaf(m10, p.x, origin.y)));
      psm__v2_store(outputs, i, r);
    }
    return;
  }
#else
  for (psm__i32 i = 0; i < count; i++) {
    struct psm__vec2 p = psm__v2_load(inputs, i);
    struct psm__vec2 r = psm__v2(fmaf(m01, p.y, fmaf(m00, p.x, origin.x)),
        fmaf(m11, p.y, fmaf(m10, p.x, origin.y)));
    psm__v2_store(outputs, i, r);
  }
#endif
}

static inline struct psm__vec2
psm__deformer_transform_point(struct psm__model *m,
    psm__i32 deformer_idx, struct psm__vec2 p)
{
  psm__f32 in[2] = {p.x, p.y};
  psm__f32 out[2];
  psm__i32 pt = m->deformers.nodes[deformer_idx].type;
  if (pt == PSM__DEFORMER_TYPE_WARP)
    psm__warp_transform(m, deformer_idx, in, out, 1);
  else
    psm__rot_transform(m, deformer_idx, in, out, 1);
  return (struct psm__vec2){out[0], out[1]};
}

static void
psm__propagate_deformer_colors(psm__f32 *mo, psm__f32 *so,
    const psm__f32 *sm, const psm__f32 *ss,
    psm__i32 self_idx, psm__i32 spec_idx, psm__i32 parent_idx)
{
  psm__i32 s4 = self_idx * 4, p4 = spec_idx * 4;

  if (parent_idx == -1) {
    mo[s4 + 0] = sm[p4 + 0];
    mo[s4 + 1] = sm[p4 + 1];
    mo[s4 + 2] = sm[p4 + 2];
    mo[s4 + 3] = 1.0f;

    so[s4 + 0] = ss[p4 + 0];
    so[s4 + 1] = ss[p4 + 1];
    so[s4 + 2] = ss[p4 + 2];
    so[s4 + 3] = 1.0f;
  } else {
    psm__i32 pi4 = parent_idx * 4;
    const psm__f32 *pm = &mo[pi4];
    const psm__f32 *ps = &so[pi4];

    mo[s4 + 0] = sm[p4 + 0] * pm[0];
    mo[s4 + 1] = sm[p4 + 1] * pm[1];
    mo[s4 + 2] = sm[p4 + 2] * pm[2];
    mo[s4 + 3] = 1.0f;

    so[s4 + 0] = fmaf(-ss[p4 + 0], ps[0], ss[p4 + 0] + ps[0]);
    so[s4 + 1] = fmaf(-ss[p4 + 1], ps[1], ss[p4 + 1] + ps[1]);
    so[s4 + 2] = fmaf(-ss[p4 + 2], ps[2], ss[p4 + 2] + ps[2]);
    so[s4 + 3] = 1.0f;
  }
}

static void
psm__apply_warp(struct psm__model *m, psm__i32 di)
{
  struct psm__deformer_node *dn = m->deformers.nodes;
  struct psm__deformer_node *self = &dn[di];
  psm__f32 *d_opa = m->deformers.opacity;
  psm__f32 *d_scl = m->deformers.scale;
  psm__i32 pi = self->parent_deformer_idx;
  psm__i32 si = self->local_idx;

  if (pi == -1) {
    d_opa[di] = m->deformers.warps.opacity[si];
    d_scl[di] = 1.0f;
  } else {
    psm__f32 **pos = m->deformers.warps.pos;
    psm__i32 vc = m->deformers.warps.items[si].vertex_count;
    psm__i32 pt = dn[pi].type;

    switch (pt) {
    case PSM__DEFORMER_TYPE_WARP:
      psm__warp_transform(m, pi, pos[si], pos[si], vc);
      break;
    case PSM__DEFORMER_TYPE_ROTATION:
      psm__rot_transform(m, pi, pos[si], pos[si], vc);
      break;
    }

    d_opa[di] = m->deformers.warps.opacity[si] * d_opa[pi];
    d_scl[di] = d_scl[pi];
  }

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__propagate_deformer_colors(
      m->deformers.mul_color, m->deformers.scr_color,
      m->deformers.warps.mul_color, m->deformers.warps.scr_color,
      di, si, pi);
}

static void
psm__apply_rotation(struct psm__model *m, psm__i32 di)
{
  struct psm__deformer_node *dn = m->deformers.nodes;
  struct psm__deformer_node *self = &dn[di];
  psm__f32 *d_opa = m->deformers.opacity;
  psm__f32 *d_scl = m->deformers.scale;
  psm__i32 pi = self->parent_deformer_idx;
  psm__i32 si = self->local_idx;

  psm__f32 *r_opa = m->deformers.rotations.opacity;
  psm__f32 *r_scl = m->deformers.rotations.scale;
  psm__f32 *r_ox = m->deformers.rotations.origin_x;
  psm__f32 *r_oy = m->deformers.rotations.origin_y;
  psm__f32 *r_ang = m->deformers.rotations.angle;

  if (pi == -1) {
    d_opa[di] = r_opa[si];
    d_scl[di] = r_scl[si];
  } else {
    struct psm__vec2 origin = psm__v2(r_ox[si], r_oy[si]);
    struct psm__vec2 direction = {0.0f, 0.0f};
    psm__i32 pt = dn[pi].type;
    psm__f32 dir_delta = (pt == PSM__DEFORMER_TYPE_ROTATION) ? -10.0f : -0.1f;

    struct psm__vec2 t_origin = psm__deformer_transform_point(m, pi, origin);

    psm__f32 scale = 1.0f;
    psm__i32 iter;
    for (iter = 0; iter < 16; iter++) {
      struct psm__vec2 tp = psm__v2(origin.x, origin.y + scale * dir_delta);
      struct psm__vec2 tt = psm__deformer_transform_point(m, pi, tp);
      struct psm__vec2 d = psm__v2_sub(tt, t_origin);

      if (d.x != 0.0f || d.y != 0.0f) {
        direction = d;
        break;
      }

      tp = psm__v2(origin.x, origin.y - scale * dir_delta);
      tt = psm__deformer_transform_point(m, pi, tp);
      d = psm__v2_sub(tt, t_origin);

      if (d.x != 0.0f || d.y != 0.0f) {
        direction = psm__v2_neg(d);
        break;
      }

      scale *= 0.1f;
    }

    if (iter >= 16) {
      PSM__WARN("rotation direction did not converge");
    }

    psm__f32 base_dir[2] = {0.0f, dir_delta};
    psm__f32 dir_arr[2] = {direction.x, direction.y};
    psm__f32 angle_adj =
        (psm__get_angle_not_abs(base_dir, dir_arr) * -180.0f) / PSM__PI;

    origin = psm__deformer_transform_point(m, pi, origin);

    r_ox[si] = origin.x;
    r_oy[si] = origin.y;
    r_ang[si] += angle_adj;

    d_opa[di] = r_opa[si] * d_opa[pi];

    psm__f32 cs = r_scl[si] * d_scl[pi];
    d_scl[di] = cs;
    r_scl[si] = cs;
  }

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__propagate_deformer_colors(
      m->deformers.mul_color, m->deformers.scr_color,
      m->deformers.rotations.mul_color,
      m->deformers.rotations.scr_color, di, si, pi);
}


PSM__DEF void
psm__enable_deformers(struct psm__model *m)
{
  psm__i32 count = m->deformers.count;
  if (count <= 0)
    return;

  struct psm__deformer_node *nodes = m->deformers.nodes;
  bool *en = m->deformers.enable;
  bool *pen = m->parts.enable;
  bool *wen = m->deformers.warps.enable;
  bool *ren = m->deformers.rotations.enable;

  for (psm__i32 i = 0; i < count; i++) {
    struct psm__deformer_node *node = &nodes[i];
    bool e = node->local_enable;
    psm__i32 ppi = node->parent_part_idx;
    psm__i32 pdi = node->parent_deformer_idx;

    if (e && ppi != -1) e = pen[ppi];
    if (e && pdi != -1) e = en[pdi];
    if (e) e = !node->binding->out_of_range;

    en[i] = e;

    psm__i32 dt = node->type;
    psm__i32 si = node->local_idx;

    switch (dt) {
    case PSM__DEFORMER_TYPE_WARP:
      wen[si] = e;
      break;
    case PSM__DEFORMER_TYPE_ROTATION:
      ren[si] = e;
      break;
    default:
      PSM__LOG("unknown deformer type");
      break;
    }
  }
}

PSM__DEF void
psm__gather_warps(struct psm__model *m)
{
  psm__i32 count = m->deformers.warps.count;
  if (count <= 0)
    return;

  struct psm__sections *ms = m->source->sections;
  struct psm__warp *items = m->deformers.warps.items;
  if (!items)
    return;

  struct psm__warp_keydata *wk = &m->deformers.warps.keydata;
  psm__i32 *kb = ms->warp_src.keyform_off;
  psm__i32 max_keyforms = ms->count_info->warp_keyforms;

  struct psm__binding *bindings[count];
  for (psm__i32 i = 0; i < count; i++)
    bindings[i] = items[i].binding;

  struct psm__gather_channel ch[] = {
    { ms->warp_key_src.opacity, wk->opacity },
  };
  psm__gather_scalars(count, bindings, kb, max_keyforms, &wk->interp, ch, 1);

  psm__gather_positions(count, bindings, kb, max_keyforms,
      ms->key_pos_src.xy, ms->warp_key_src.key_pos_off,
      ms->count_info->keyform_pos, wk->pos);

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__i32 *ckb = ms->warp_src.key_color_off;
  if (!ckb || !ms->keyform_mul_color_src.r || !ms->keyform_scr_color_src.r)
    return;

  psm__gather_colors(count, bindings, ckb,
      ms->count_info->keyform_mul_colors,
      &ms->keyform_mul_color_src, &ms->keyform_scr_color_src,
      &wk->mul_color, &wk->scr_color);
}

PSM__DEF void
psm__gather_rotations(struct psm__model *m)
{
  psm__i32 count = m->deformers.rotations.count;
  if (count <= 0)
    return;

  struct psm__sections *ms = m->source->sections;
  struct psm__rotation *items = m->deformers.rotations.items;
  if (!items)
    return;

  struct psm__rotation_keydata *rk = &m->deformers.rotations.keydata;
  psm__i32 *kb = ms->rotation_src.keyform_off;
  psm__i32 max_keyforms = ms->count_info->rotation_keyforms;

  struct psm__binding *bindings[count];
  for (psm__i32 i = 0; i < count; i++)
    bindings[i] = items[i].binding;

  struct psm__gather_channel ch[] = {
    { ms->rotation_key_src.opacity,  rk->opacity },
    { ms->rotation_key_src.angle,    rk->angle },
    { ms->rotation_key_src.origin_x, rk->origin_x },
    { ms->rotation_key_src.origin_y, rk->origin_y },
    { ms->rotation_key_src.scale,    rk->scale },
  };
  psm__gather_scalars(count, bindings, kb, max_keyforms, &rk->interp, ch, 5);

  psm__gather_reflect(count, bindings, kb, max_keyforms,
      ms->rotation_key_src.reflect_x, ms->rotation_key_src.reflect_y,
      m->deformers.rotations.reflect_x, m->deformers.rotations.reflect_y);

  if (m->source->header->version < csmMocVersion_42)
    return;

  psm__i32 *ckb = ms->rotation_src.key_color_off;
  if (!ckb || !ms->keyform_mul_color_src.r || !ms->keyform_scr_color_src.r)
    return;

  psm__gather_colors(count, bindings, ckb,
      ms->count_info->keyform_mul_colors,
      &ms->keyform_mul_color_src, &ms->keyform_scr_color_src,
      &rk->mul_color, &rk->scr_color);
}

PSM__DEF void
psm__apply_transforms(struct psm__model *m)
{
  psm__i32 count = m->deformers.count;
  if (count <= 0)
    return;

  bool *en = m->deformers.enable;
  struct psm__deformer_node *nodes = m->deformers.nodes;

  /*
   * Iterate in array order. MOC3 stores deformers in
   * topological order (parents before children), so each
   * deformer's parent is already transformed.
   */
  for (psm__i32 i = 0; i < count; i++) {
    if (en[i]) {
      switch (nodes[i].type) {
      case PSM__DEFORMER_TYPE_WARP:
        psm__apply_warp(m, i);
        break;
      case PSM__DEFORMER_TYPE_ROTATION:
        psm__apply_rotation(m, i);
        break;
      }
    }
  }
}

PSM__DEF void
psm__apply_transforms_to_meshes(struct psm__model *m)
{
  psm__i32 count = m->art_meshes.count;
  if (count <= 0)
    return;

  struct psm__art_mesh *am = m->art_meshes.meshes;
  psm__f32 *d_opa = m->deformers.opacity;
  struct psm__deformer_node *dn = m->deformers.nodes;
  psm__f32 **cp = m->art_meshes.pos;
  psm__f32 *am_opa = m->art_meshes.opacity;
  bool *en = m->art_meshes.enable;

  for (psm__i32 i = 0; i < count; i++) {
    if (!en[i]) continue;

    psm__i32 pdi = am[i].parent_deformer_idx;
    if (pdi == -1) continue;

    psm__i32 vc = am[i].vertex_count;
    am_opa[i] *= d_opa[pdi];

    psm__i32 dt = dn[pdi].type;
    if (dt == PSM__DEFORMER_TYPE_WARP)
      psm__warp_transform(m, pdi, cp[i], cp[i], vc);
    else
      psm__rot_transform(m, pdi, cp[i], cp[i], vc);
  }
}

/**
 * 4×4 affine transformation in homogeneous coordinates.
 *
 * Stored as a flat 16-element array in **row-major** order. The
 * row-major choice matches how we build the matrix in code (row by
 * row reads naturally), at the cost of one transpose if you ever
 * hand it to WebGL (which expects column-major). The render layer
 * (knot-mesh) handles that conversion at the boundary.
 *
 * The bottom row is always `[0, 0, 0, 1]` for affine transforms;
 * we don't explicitly enforce it but every constructor produces
 * one. Perspective / projection transforms aren't supported here —
 * that's a different abstraction belonging in the render layer.
 *
 * Distinction between `transformPoint` and `transformVector`:
 * points get the translation, vectors don't. Same matrix, different
 * w-coordinate at the conceptual w=0/w=1 boundary.
 */

import { type Vec3, vec3, sub, dot, cross, normalize, length } from './vec3';

export interface Transform {
  /** 16 elements, row-major: m[row*4 + col]. */
  readonly m: readonly number[];
}

/** Identity transform. */
export const IDENTITY: Transform = {
  m: [
    1, 0, 0, 0,
    0, 1, 0, 0,
    0, 0, 1, 0,
    0, 0, 0, 1,
  ],
};

/** Translation by `t`. */
export const translation = (t: Vec3): Transform => ({
  m: [
    1, 0, 0, t.x,
    0, 1, 0, t.y,
    0, 0, 1, t.z,
    0, 0, 0, 1,
  ],
});

/**
 * Rotation by `angle` radians around `axis`. Axis is normalized
 * internally; pass any non-zero vector. Right-hand rule: looking
 * down the axis from positive infinity, positive angle rotates
 * counter-clockwise.
 */
export const rotation = (axis: Vec3, angle: number): Transform => {
  const a = normalize(axis);
  if (length(a) === 0) {
    throw new Error('rotation: axis cannot be zero');
  }
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  const t = 1 - c;
  const { x, y, z } = a;
  // Standard Rodrigues' rotation matrix.
  return {
    m: [
      t * x * x + c,     t * x * y - s * z, t * x * z + s * y, 0,
      t * x * y + s * z, t * y * y + c,     t * y * z - s * x, 0,
      t * x * z - s * y, t * y * z + s * x, t * z * z + c,     0,
      0,                 0,                 0,                 1,
    ],
  };
};

/** Non-uniform scale. Use `scaleUniform` for the common s·s·s case. */
export const scale = (s: Vec3): Transform => ({
  m: [
    s.x, 0,   0,   0,
    0,   s.y, 0,   0,
    0,   0,   s.z, 0,
    0,   0,   0,   1,
  ],
});

export const scaleUniform = (s: number): Transform => scale(vec3(s, s, s));

/**
 * Compose transforms: result = `a · b`. When applied to a point,
 * `b` runs first. So `translate(t) · rotate(...)` rotates first,
 * then translates — matches the convention of "right-most matrix
 * runs first" from rendering pipelines.
 */
export const compose = (a: Transform, b: Transform): Transform => {
  const out = new Array<number>(16);
  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 4; c++) {
      let sum = 0;
      for (let k = 0; k < 4; k++) {
        sum += a.m[r * 4 + k]! * b.m[k * 4 + c]!;
      }
      out[r * 4 + c] = sum;
    }
  }
  return { m: out };
};

/**
 * Transform a point. Includes translation. Equivalent to
 * `m · (p.x, p.y, p.z, 1)` then dropping the w.
 */
export const transformPoint = (t: Transform, p: Vec3): Vec3 => {
  const { m } = t;
  return vec3(
    m[0]! * p.x + m[1]! * p.y + m[2]! * p.z + m[3]!,
    m[4]! * p.x + m[5]! * p.y + m[6]! * p.z + m[7]!,
    m[8]! * p.x + m[9]! * p.y + m[10]! * p.z + m[11]!,
  );
};

/**
 * Transform a direction vector. NO translation (treats input as a
 * displacement). Equivalent to `m · (v.x, v.y, v.z, 0)`. For
 * rigid + uniform-scale transforms this is correct; for
 * non-uniform-scale transforms the result is the affine
 * transformation of the displacement, which may not be what you
 * want for normals (use `transformNormal` for those).
 */
export const transformVector = (t: Transform, v: Vec3): Vec3 => {
  const { m } = t;
  return vec3(
    m[0]! * v.x + m[1]! * v.y + m[2]! * v.z,
    m[4]! * v.x + m[5]! * v.y + m[6]! * v.z,
    m[8]! * v.x + m[9]! * v.y + m[10]! * v.z,
  );
};

/**
 * Transform a surface normal. For non-uniform-scale transforms,
 * normals transform by the inverse-transpose of the upper-left
 * 3×3 block, NOT by the transform matrix itself. This implementation
 * computes the inverse-transpose explicitly.
 *
 * For rigid + uniform-scale transforms `transformNormal` returns
 * the same direction as `transformVector` (up to length), so if
 * you know your transform is rigid you can use the cheaper
 * `transformVector` and re-normalize. The caller is responsible
 * for re-normalizing in either case.
 */
export const transformNormal = (t: Transform, n: Vec3): Vec3 => {
  const it = inverse(t);
  if (it === null) {
    throw new Error('transformNormal: transform is singular');
  }
  // Inverse-transpose of the upper-left 3×3 block. We have `it.m`
  // which is the full 4×4 inverse; we just need the transposed 3×3.
  const { m } = it;
  return vec3(
    m[0]! * n.x + m[4]! * n.y + m[8]!  * n.z,
    m[1]! * n.x + m[5]! * n.y + m[9]!  * n.z,
    m[2]! * n.x + m[6]! * n.y + m[10]! * n.z,
  );
};

/**
 * Matrix inverse via cofactor expansion. Returns `null` if the
 * matrix is singular (determinant near zero). For frequently-used
 * transforms with known structure (pure translation, pure rotation),
 * specialized inverses are cheaper — but this general path is
 * fine for graph-eval cadence.
 *
 * Implementation cribbed from a standard 4×4 cofactor inverse —
 * not the prettiest but it's well-tested code shape.
 */
export const inverse = (t: Transform): Transform | null => {
  const m = t.m;
  const inv = new Array<number>(16);

  inv[0] =  m[5]!*m[10]!*m[15]! - m[5]!*m[11]!*m[14]! - m[9]!*m[6]!*m[15]! + m[9]!*m[7]!*m[14]! + m[13]!*m[6]!*m[11]! - m[13]!*m[7]!*m[10]!;
  inv[4] = -m[4]!*m[10]!*m[15]! + m[4]!*m[11]!*m[14]! + m[8]!*m[6]!*m[15]! - m[8]!*m[7]!*m[14]! - m[12]!*m[6]!*m[11]! + m[12]!*m[7]!*m[10]!;
  inv[8] =  m[4]!*m[9]! *m[15]! - m[4]!*m[11]!*m[13]! - m[8]!*m[5]!*m[15]! + m[8]!*m[7]!*m[13]! + m[12]!*m[5]!*m[11]! - m[12]!*m[7]!*m[9]!;
  inv[12]= -m[4]!*m[9]! *m[14]! + m[4]!*m[10]!*m[13]! + m[8]!*m[5]!*m[14]! - m[8]!*m[6]!*m[13]! - m[12]!*m[5]!*m[10]! + m[12]!*m[6]!*m[9]!;
  inv[1] = -m[1]!*m[10]!*m[15]! + m[1]!*m[11]!*m[14]! + m[9]!*m[2]!*m[15]! - m[9]!*m[3]!*m[14]! - m[13]!*m[2]!*m[11]! + m[13]!*m[3]!*m[10]!;
  inv[5] =  m[0]!*m[10]!*m[15]! - m[0]!*m[11]!*m[14]! - m[8]!*m[2]!*m[15]! + m[8]!*m[3]!*m[14]! + m[12]!*m[2]!*m[11]! - m[12]!*m[3]!*m[10]!;
  inv[9] = -m[0]!*m[9]! *m[15]! + m[0]!*m[11]!*m[13]! + m[8]!*m[1]!*m[15]! - m[8]!*m[3]!*m[13]! - m[12]!*m[1]!*m[11]! + m[12]!*m[3]!*m[9]!;
  inv[13]=  m[0]!*m[9]! *m[14]! - m[0]!*m[10]!*m[13]! - m[8]!*m[1]!*m[14]! + m[8]!*m[2]!*m[13]! + m[12]!*m[1]!*m[10]! - m[12]!*m[2]!*m[9]!;
  inv[2] =  m[1]!*m[6]! *m[15]! - m[1]!*m[7]! *m[14]! - m[5]!*m[2]!*m[15]! + m[5]!*m[3]!*m[14]! + m[13]!*m[2]!*m[7]!  - m[13]!*m[3]!*m[6]!;
  inv[6] = -m[0]!*m[6]! *m[15]! + m[0]!*m[7]! *m[14]! + m[4]!*m[2]!*m[15]! - m[4]!*m[3]!*m[14]! - m[12]!*m[2]!*m[7]!  + m[12]!*m[3]!*m[6]!;
  inv[10]=  m[0]!*m[5]! *m[15]! - m[0]!*m[7]! *m[13]! - m[4]!*m[1]!*m[15]! + m[4]!*m[3]!*m[13]! + m[12]!*m[1]!*m[7]!  - m[12]!*m[3]!*m[5]!;
  inv[14]= -m[0]!*m[5]! *m[14]! + m[0]!*m[6]! *m[13]! + m[4]!*m[1]!*m[14]! - m[4]!*m[2]!*m[13]! - m[12]!*m[1]!*m[6]!  + m[12]!*m[2]!*m[5]!;
  inv[3] = -m[1]!*m[6]! *m[11]! + m[1]!*m[7]! *m[10]! + m[5]!*m[2]!*m[11]! - m[5]!*m[3]!*m[10]! - m[9]! *m[2]!*m[7]!  + m[9]! *m[3]!*m[6]!;
  inv[7] =  m[0]!*m[6]! *m[11]! - m[0]!*m[7]! *m[10]! - m[4]!*m[2]!*m[11]! + m[4]!*m[3]!*m[10]! + m[8]! *m[2]!*m[7]!  - m[8]! *m[3]!*m[6]!;
  inv[11]= -m[0]!*m[5]! *m[11]! + m[0]!*m[7]! *m[9]!  + m[4]!*m[1]!*m[11]! - m[4]!*m[3]!*m[9]!  - m[8]! *m[1]!*m[7]!  + m[8]! *m[3]!*m[5]!;
  inv[15]=  m[0]!*m[5]! *m[10]! - m[0]!*m[6]! *m[9]!  - m[4]!*m[1]!*m[10]! + m[4]!*m[2]!*m[9]!  + m[8]! *m[1]!*m[6]!  - m[8]! *m[2]!*m[5]!;

  const det = m[0]! * inv[0]! + m[1]! * inv[4]! + m[2]! * inv[8]! + m[3]! * inv[12]!;
  if (Math.abs(det) < 1e-15) return null;
  const invDet = 1 / det;
  for (let i = 0; i < 16; i++) inv[i] = inv[i]! * invDet;
  return { m: inv };
};

/**
 * Look-at transform: a frame whose origin is at `eye`, with -Z
 * pointing toward `target` and +Y aligned with `up`. Useful for
 * camera-style transforms; the mirror inverse (camera-from-world)
 * is what you'd typically pass to a renderer.
 *
 * Returns the world-from-camera frame. Apply `inverse(...)` for
 * the camera-from-world matrix.
 */
export const lookAt = (eye: Vec3, target: Vec3, up: Vec3): Transform => {
  const forward = normalize(sub(target, eye));
  if (length(forward) === 0) {
    throw new Error('lookAt: eye and target coincide');
  }
  // Build right and recomputed-up via Gram-Schmidt.
  const right = normalize(cross(forward, up));
  if (length(right) === 0) {
    throw new Error('lookAt: forward direction parallel to up');
  }
  const trueUp = cross(right, forward);
  // -Z is the conventional camera "look" direction.
  const _check = dot(right, trueUp); // for completeness; should be ~0
  void _check;
  return {
    m: [
      right.x, trueUp.x, -forward.x, eye.x,
      right.y, trueUp.y, -forward.y, eye.y,
      right.z, trueUp.z, -forward.z, eye.z,
      0,       0,        0,          1,
    ],
  };
};

/** Type guard for runtime use. */
export const isTransform = (v: unknown): v is Transform =>
  typeof v === 'object' &&
  v !== null &&
  Array.isArray((v as Transform).m) &&
  (v as Transform).m.length === 16;

/**
 * Infinite plane in 3D.
 *
 * Stored as origin + orthonormal frame (normal, uAxis, vAxis), the
 * same shape as `knot-geom::Plane`. The frame is materialized once
 * at construction so projection and parameterization are cheap.
 *
 * Convention: `normal × uAxis = vAxis` (right-handed frame). All
 * three axes are unit length and mutually perpendicular within
 * floating-point tolerance after construction.
 */

import {
  type Vec3,
  add,
  sub,
  scale,
  dot,
  cross,
  normalize,
  length,
  X_AXIS,
  Y_AXIS,
} from './vec3';

export interface Plane {
  readonly origin: Vec3;
  readonly normal: Vec3;
  readonly uAxis: Vec3;
  readonly vAxis: Vec3;
}

/**
 * Build a plane from origin and normal. The u/v axes are derived to
 * form a right-handed orthonormal frame; their orientation is
 * determined by which world axis is "least aligned" with the
 * normal — gives a deterministic but unspecified rotation around
 * the normal. For a specific in-plane orientation, use `withFrame`.
 */
export const plane = (origin: Vec3, normal: Vec3): Plane => {
  const n = normalize(normal);
  if (length(n) === 0) {
    throw new Error('plane: normal cannot be zero');
  }
  // Pick a seed reference direction least aligned with the normal,
  // so the cross product is numerically stable.
  const seed = Math.abs(n.x) < 0.9 ? X_AXIS : Y_AXIS;
  const uAxis = normalize(cross(n, seed));
  const vAxis = cross(n, uAxis); // already unit length since n, uAxis are unit & perpendicular
  return { origin, normal: n, uAxis, vAxis };
};

/**
 * Build a plane with an explicit in-plane reference direction.
 * `refDirection` is projected onto the plane and normalized to
 * become uAxis; vAxis is then `normal × uAxis`. Throws if
 * `refDirection` is parallel to `normal`.
 */
export const withFrame = (origin: Vec3, normal: Vec3, refDirection: Vec3): Plane => {
  const n = normalize(normal);
  // Project refDirection onto the plane: ref - (ref·n)·n.
  const refProj = sub(refDirection, scale(n, dot(refDirection, n)));
  const u = normalize(refProj);
  if (length(u) === 0) {
    throw new Error('withFrame: refDirection is parallel to normal');
  }
  const v = cross(n, u);
  return { origin, normal: n, uAxis: u, vAxis: v };
};

/**
 * Build a plane from three non-collinear points. The origin becomes
 * `a`, the normal is `(b-a) × (c-a)` normalized, and uAxis is
 * `b - a` projected to the plane (i.e. just `normalize(b-a)` since
 * it already lies in the plane).
 */
export const fromPoints = (a: Vec3, b: Vec3, c: Vec3): Plane => {
  const ab = sub(b, a);
  const ac = sub(c, a);
  const n = cross(ab, ac);
  if (length(n) === 0) {
    throw new Error('fromPoints: the three points are collinear');
  }
  return withFrame(a, n, ab);
};

/**
 * Signed distance from `p` to the plane. Positive means `p` is on
 * the side the normal points to. Sign convention matches
 * `knot-geom::Plane::signed_distance`.
 */
export const signedDistance = (pl: Plane, p: Vec3): number =>
  dot(pl.normal, sub(p, pl.origin));

/** Absolute distance from `p` to the plane. */
export const distance = (pl: Plane, p: Vec3): number => Math.abs(signedDistance(pl, p));

/** Closest point on the plane to `p`. */
export const project = (pl: Plane, p: Vec3): Vec3 =>
  sub(p, scale(pl.normal, signedDistance(pl, p)));

/**
 * 3D point at plane parameter `(u, v)`. Inverse of `paramAt` for
 * points on the plane.
 */
export const pointAt = (pl: Plane, u: number, v: number): Vec3 =>
  add(add(pl.origin, scale(pl.uAxis, u)), scale(pl.vAxis, v));

/**
 * `(u, v)` parameter of the closest point on the plane to `p`. For
 * `p` already on the plane this is exact; for `p` off the plane it's
 * the parameter of `project(pl, p)`.
 */
export const paramAt = (pl: Plane, p: Vec3): { u: number; v: number } => {
  const d = sub(p, pl.origin);
  return { u: dot(d, pl.uAxis), v: dot(d, pl.vAxis) };
};

/**
 * Does `p` lie on the plane within `tol`? Uses absolute distance,
 * so the tolerance is in world units, not parametric units.
 */
export const contains = (pl: Plane, p: Vec3, tol = 1e-9): boolean =>
  distance(pl, p) <= tol;

/** Type guard for runtime use (graph evaluator port-type checks). */
export const isPlane = (v: unknown): v is Plane => {
  if (typeof v !== 'object' || v === null) return false;
  const p = v as Plane;
  return (
    typeof p.origin === 'object' &&
    typeof p.normal === 'object' &&
    typeof p.uAxis === 'object' &&
    typeof p.vAxis === 'object'
  );
};

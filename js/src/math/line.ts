/**
 * Line in 3D, stored as origin + direction.
 *
 * Two flavors share the same struct: an *infinite line* (no
 * parameter range) and a *line segment* with `[0, 1]` parameter
 * range from origin (t=0) to origin+direction (t=1). Operations
 * accept a `clamped` flag where the distinction matters
 * (`closestPoint`, `intersect`).
 *
 * For pure rendering / display use cases use the segment form
 * (`fromPoints`); for analytic operations against planes or other
 * lines the infinite form (`fromRay`) is usually right.
 */

import {
  type Vec3,
  add,
  sub,
  scale,
  dot,
  lengthSq,
  length,
  normalize,
} from './vec3';
import { type Plane, signedDistance as planeSignedDistance } from './plane';

export interface Line {
  readonly origin: Vec3;
  /**
   * Direction vector. NOT normalized — the parameterization
   * `origin + t·direction` runs from t=0 to t=1 over the segment
   * length, which makes the segment form natural without a separate
   * "length" field.
   */
  readonly direction: Vec3;
}

/**
 * Build a line segment from two points. `t=0` is `start`, `t=1` is
 * `end`; the direction vector is `end - start` (length-encoding).
 */
export const fromPoints = (start: Vec3, end: Vec3): Line => ({
  origin: start,
  direction: sub(end, start),
});

/**
 * Build an infinite ray. `direction` is normalized so `t` is
 * arc-length distance from `origin`.
 */
export const fromRay = (origin: Vec3, direction: Vec3): Line => ({
  origin,
  direction: normalize(direction),
});

/** Length of the segment form. For infinite lines this is 1. */
export const length_ = (l: Line): number => length(l.direction);

/** Evaluate the line at parameter `t`. Linear in `t` regardless of segment vs ray. */
export const pointAt = (l: Line, t: number): Vec3 => add(l.origin, scale(l.direction, t));

/** End point of a segment (`t=1`). For a ray this is `origin + direction`. */
export const end = (l: Line): Vec3 => pointAt(l, 1);

/**
 * Closest point on the line to `query`. With `clamped = true`
 * (default for safety with segment-form lines), `t` is clamped to
 * `[0, 1]` so the result lies within the segment. Pass `false` for
 * infinite-line semantics.
 */
export const closestPoint = (
  l: Line,
  query: Vec3,
  clamped = true,
): { point: Vec3; t: number } => {
  const ds = lengthSq(l.direction);
  if (ds === 0) return { point: l.origin, t: 0 };
  let t = dot(sub(query, l.origin), l.direction) / ds;
  if (clamped) t = Math.max(0, Math.min(1, t));
  return { point: pointAt(l, t), t };
};

/** Distance from `query` to the line/segment. */
export const distanceToPoint = (l: Line, query: Vec3, clamped = true): number => {
  const { point } = closestPoint(l, query, clamped);
  return length(sub(query, point));
};

/**
 * Intersect with a plane. Returns `null` when the line is parallel
 * to the plane (no intersection or infinite intersections — both
 * cases collapse to "no single point"). When `clamped`, returns
 * `null` if the intersection parameter falls outside `[0, 1]`.
 */
export const intersectPlane = (
  l: Line,
  pl: Plane,
  clamped = true,
): { point: Vec3; t: number } | null => {
  const denom = dot(l.direction, pl.normal);
  if (Math.abs(denom) < 1e-15) return null; // parallel
  const t = -planeSignedDistance(pl, l.origin) / denom;
  if (clamped && (t < 0 || t > 1)) return null;
  return { point: pointAt(l, t), t };
};

/**
 * Closest pair of points between two lines. Returns the points on
 * each line plus the distance and parameters. For parallel lines
 * (or coincident), returns the closest perpendicular foot from
 * `a.origin` to `b`.
 *
 * Standard 3D line-line minimum-distance derivation. With both
 * `clamped`, gives the closest points within the two segments.
 */
export const closestPair = (
  a: Line,
  b: Line,
  clamped = true,
): {
  pointA: Vec3;
  pointB: Vec3;
  tA: number;
  tB: number;
  distance: number;
} => {
  const da = a.direction;
  const db = b.direction;
  const w0 = sub(a.origin, b.origin);
  const aa = dot(da, da);
  const bb = dot(db, db);
  const ab = dot(da, db);
  const aw = dot(da, w0);
  const bw = dot(db, w0);
  const denom = aa * bb - ab * ab;
  let tA: number;
  let tB: number;
  if (Math.abs(denom) < 1e-15) {
    // Parallel — pick the foot of perpendicular from a.origin onto b.
    tA = 0;
    tB = bb === 0 ? 0 : bw / bb;
  } else {
    tA = (ab * bw - bb * aw) / denom;
    tB = (aa * bw - ab * aw) / denom;
  }
  if (clamped) {
    tA = Math.max(0, Math.min(1, tA));
    tB = Math.max(0, Math.min(1, tB));
  }
  const pointA = pointAt(a, tA);
  const pointB = pointAt(b, tB);
  return { pointA, pointB, tA, tB, distance: length(sub(pointA, pointB)) };
};

/** Type guard for runtime use. */
export const isLine = (v: unknown): v is Line => {
  if (typeof v !== 'object' || v === null) return false;
  const l = v as Line;
  return typeof l.origin === 'object' && typeof l.direction === 'object';
};

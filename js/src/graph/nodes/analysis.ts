/**
 * Analysis nodes — read-only inspection of curves and breps. Each
 * node wraps a method already exposed on the typed JS Curve/Brep
 * classes; nothing new on the WASM side.
 *
 * Pattern: returns scalars / vec3 / lists, not new geometry. Compose
 * with other graph nodes to drive sizes, positions, conditional flow,
 * etc.
 */
import { ZERO } from '../../math/vec3';
import { defineNode } from './define';

// ── Curve analysis ──────────────────────────────────────────────────

/** Arc length of a curve. */
export const CurveLengthNode = defineNode({
  id: 'core.curve.length',
  label: 'Curve Length',
  inputs: {
    curve: { kind: 'curve' as const },
    tolerance: { kind: 'number' as const, default: 1e-6 },
  },
  outputs: { length: { kind: 'number' as const } },
  evaluate: ({ curve, tolerance }) => ({ length: curve.length(tolerance) }),
});

/**
 * Axis-aligned bounding box of a curve. Exposes corners and size as
 * three separate vec3 outputs so downstream graphs can pick what they
 * need without an extra Deconstruct.
 */
export const CurveBoundingBoxNode = defineNode({
  id: 'core.curve.boundingBox',
  label: 'Curve BBox',
  inputs: {
    curve: { kind: 'curve' as const },
  },
  outputs: {
    min: { kind: 'vec3' as const },
    max: { kind: 'vec3' as const },
    size: { kind: 'vec3' as const },
  },
  evaluate: ({ curve }) => {
    const b = curve.boundingBox();
    return {
      min: b.min,
      max: b.max,
      size: { x: b.max.x - b.min.x, y: b.max.y - b.min.y, z: b.max.z - b.min.z },
    };
  },
});

/**
 * Closest point on a curve to a query point. Returns the projected
 * point, the parameter on the curve at which it sits, and the
 * 3D distance.
 */
export const CurveClosestPointNode = defineNode({
  id: 'core.curve.closestPoint',
  label: 'Curve Closest Point',
  inputs: {
    curve: { kind: 'curve' as const },
    query: { kind: 'vec3' as const, default: ZERO },
  },
  outputs: {
    point: { kind: 'vec3' as const },
    param: { kind: 'number' as const },
    distance: { kind: 'number' as const },
  },
  evaluate: ({ curve, query }) => {
    const r = curve.closestPoint(query);
    return { point: r.point, param: r.param, distance: r.distance };
  },
});

/** Unit tangent vector at parameter t. */
export const CurveTangentAtNode = defineNode({
  id: 'core.curve.tangentAt',
  label: 'Tangent At',
  inputs: {
    curve: { kind: 'curve' as const },
    t: { kind: 'number' as const, default: 0 },
  },
  outputs: { tangent: { kind: 'vec3' as const } },
  evaluate: ({ curve, t }) => ({ tangent: curve.tangentAt(t) }),
});

/**
 * Divide a curve into `n` equal-arc-length segments. Returns the full
 * list of `n + 1` parameter values (including both endpoints) — unlike
 * `core.curve.divide` (parameter-uniform), this is arc-length-uniform.
 */
export const CurveDivideByLengthNode = defineNode({
  id: 'core.curve.divideByLength',
  label: 'Divide By Length',
  inputs: {
    curve: { kind: 'curve' as const },
    n: { kind: 'number' as const, default: 10 },
    tolerance: { kind: 'number' as const, default: 1e-6 },
  },
  outputs: { params: { kind: 'list' as const } },
  evaluate: ({ curve, n, tolerance }) => {
    const segments = Math.max(1, Math.round(n));
    const params = curve.divideByLength(segments, tolerance);
    return { params };
  },
});

/**
 * Curve-curve intersection. Returns a list of 3D points where the two
 * curves meet (or empty when they don't intersect within tolerance).
 *
 * The returned list carries `Vec3` elements — feed it through `list.item`
 * or directly to a node that accepts a list of vec3.
 */
export const CurveCurveIntersectNode = defineNode({
  id: 'core.curve.intersect',
  label: 'Curve × Curve',
  inputs: {
    a: { kind: 'curve' as const },
    b: { kind: 'curve' as const },
    tolerance: { kind: 'number' as const, default: 1e-6 },
  },
  outputs: {
    points: { kind: 'list' as const },
    count: { kind: 'number' as const },
  },
  evaluate: ({ a, b, tolerance }) => {
    const hits = a.intersect(b, tolerance);
    return {
      points: hits.map((h) => h.point),
      count: hits.length,
    };
  },
});

// ── Brep analysis ───────────────────────────────────────────────────

export const BrepBoundingBoxNode = defineNode({
  id: 'core.brep.boundingBox',
  label: 'Brep BBox',
  inputs: {
    brep: { kind: 'brep' as const },
  },
  outputs: {
    min: { kind: 'vec3' as const },
    max: { kind: 'vec3' as const },
    size: { kind: 'vec3' as const },
  },
  evaluate: ({ brep }) => {
    const b = brep.boundingBox();
    return {
      min: b.min,
      max: b.max,
      size: { x: b.max.x - b.min.x, y: b.max.y - b.min.y, z: b.max.z - b.min.z },
    };
  },
});

export const BrepFaceCountNode = defineNode({
  id: 'core.brep.faceCount',
  label: 'Face Count',
  inputs: {
    brep: { kind: 'brep' as const },
  },
  outputs: { count: { kind: 'number' as const } },
  evaluate: ({ brep }) => ({ count: brep.faceCount }),
});

/**
 * Curve-fitting nodes: pass a list of points through, get a NURBS
 * Curve that interpolates or approximates them.
 *
 * Input is a `list` port carrying `Vec3` records; the node validates
 * the shape before calling the kernel so a misconnected list (e.g.
 * a Range of numbers) surfaces as a typed error port rather than a
 * confusing WASM error.
 */
import type { Knot, Vec3 } from '../../kernel';
import { defineNode } from './define';

/** Exact-interpolation NURBS through a list of points. */
export const makeInterpolateCurveNode = (knot: Knot) =>
  defineNode({
    id: 'core.curve.interpolate',
    label: 'Interpolate Curve',
    inputs: {
      points: { kind: 'list' as const, default: [] },
      degree: { kind: 'number' as const, default: 3 },
    },
    outputs: { curve: { kind: 'curve' as const } },
    evaluate: ({ points, degree }) => {
      const pts = coerceVec3List(points, 'points');
      const deg = Math.max(1, Math.round(degree));
      return { curve: knot.interpolateCurve(pts, deg) };
    },
  });

/**
 * Least-squares NURBS approximation through a list of points. The
 * curve passes exactly through the first and last points; interior
 * points are minimized in a least-squares sense.
 */
export const makeApproximateCurveNode = (knot: Knot) =>
  defineNode({
    id: 'core.curve.approximate',
    label: 'Approximate Curve',
    inputs: {
      points: { kind: 'list' as const, default: [] },
      numControlPoints: { kind: 'number' as const, default: 6 },
      degree: { kind: 'number' as const, default: 3 },
    },
    outputs: { curve: { kind: 'curve' as const } },
    evaluate: ({ points, numControlPoints, degree }) => {
      const pts = coerceVec3List(points, 'points');
      const deg = Math.max(1, Math.round(degree));
      const numCp = Math.max(deg + 1, Math.round(numControlPoints));
      return { curve: knot.approximateCurve(pts, numCp, deg) };
    },
  });

function coerceVec3List(value: unknown[], path: string): Vec3[] {
  const out: Vec3[] = [];
  for (let i = 0; i < value.length; i++) {
    const v = value[i];
    if (!isVec3(v)) {
      throw new Error(
        `${path}[${i}] is not a Vec3 (expected { x: number, y: number, z: number })`,
      );
    }
    out.push(v);
  }
  return out;
}

function isVec3(v: unknown): v is Vec3 {
  if (typeof v !== 'object' || v === null) return false;
  const o = v as { x?: unknown; y?: unknown; z?: unknown };
  return typeof o.x === 'number' && typeof o.y === 'number' && typeof o.z === 'number';
}

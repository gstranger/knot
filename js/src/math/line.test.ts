import { describe, expect, it } from 'vitest';
import * as Vec3 from './vec3';
import * as Plane from './plane';
import * as Line from './line';

describe('Line — construction', () => {
  it('fromPoints stores end - start as direction', () => {
    const l = Line.fromPoints(Vec3.vec3(1, 2, 3), Vec3.vec3(4, 6, 3));
    expect(Vec3.approxEquals(l.origin, Vec3.vec3(1, 2, 3))).toBe(true);
    expect(Vec3.approxEquals(l.direction, Vec3.vec3(3, 4, 0))).toBe(true);
    expect(Line.length_(l)).toBeCloseTo(5);
  });

  it('fromRay normalizes direction to unit length', () => {
    const l = Line.fromRay(Vec3.ZERO, Vec3.vec3(0, 0, 5));
    expect(Math.abs(Vec3.length(l.direction) - 1)).toBeLessThan(1e-12);
  });

  it('pointAt evaluates linearly', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, 0), Vec3.vec3(10, 0, 0));
    expect(Vec3.approxEquals(Line.pointAt(l, 0), Vec3.vec3(0, 0, 0))).toBe(true);
    expect(Vec3.approxEquals(Line.pointAt(l, 0.5), Vec3.vec3(5, 0, 0))).toBe(true);
    expect(Vec3.approxEquals(Line.pointAt(l, 1), Vec3.vec3(10, 0, 0))).toBe(true);
    // Extrapolation
    expect(Vec3.approxEquals(Line.pointAt(l, 2), Vec3.vec3(20, 0, 0))).toBe(true);
  });

  it('end is pointAt(1)', () => {
    const l = Line.fromPoints(Vec3.ZERO, Vec3.vec3(1, 2, 3));
    expect(Vec3.approxEquals(Line.end(l), Vec3.vec3(1, 2, 3))).toBe(true);
  });
});

describe('Line — closestPoint', () => {
  it('clamped: within segment → t in [0, 1]', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, 0), Vec3.vec3(10, 0, 0));
    const { point, t } = Line.closestPoint(l, Vec3.vec3(5, 7, 0));
    expect(t).toBeCloseTo(0.5);
    expect(Vec3.approxEquals(point, Vec3.vec3(5, 0, 0))).toBe(true);
  });

  it('clamped: query before start → t = 0', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, 0), Vec3.vec3(10, 0, 0));
    const { point, t } = Line.closestPoint(l, Vec3.vec3(-5, 7, 0));
    expect(t).toBe(0);
    expect(Vec3.approxEquals(point, Vec3.vec3(0, 0, 0))).toBe(true);
  });

  it('clamped: query past end → t = 1', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, 0), Vec3.vec3(10, 0, 0));
    const { t } = Line.closestPoint(l, Vec3.vec3(99, 7, 0));
    expect(t).toBe(1);
  });

  it('unclamped: extrapolates beyond segment', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, 0), Vec3.vec3(10, 0, 0));
    const { t } = Line.closestPoint(l, Vec3.vec3(50, 0, 0), false);
    expect(t).toBeCloseTo(5);
  });

  it('zero-length segment: returns origin', () => {
    const l = Line.fromPoints(Vec3.vec3(3, 4, 5), Vec3.vec3(3, 4, 5));
    const { point, t } = Line.closestPoint(l, Vec3.vec3(0, 0, 0));
    expect(t).toBe(0);
    expect(Vec3.approxEquals(point, Vec3.vec3(3, 4, 5))).toBe(true);
  });
});

describe('Line — distanceToPoint', () => {
  it('perpendicular distance to a segment', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, 0), Vec3.vec3(10, 0, 0));
    expect(Line.distanceToPoint(l, Vec3.vec3(5, 4, 0))).toBeCloseTo(4);
  });
});

describe('Line — intersectPlane', () => {
  const xy = Plane.plane(Vec3.ZERO, Vec3.Z_AXIS);

  it('crossing segment yields a point and t in (0, 1)', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, -5), Vec3.vec3(0, 0, 5));
    const hit = Line.intersectPlane(l, xy);
    expect(hit).not.toBeNull();
    expect(hit!.t).toBeCloseTo(0.5);
    expect(Vec3.approxEquals(hit!.point, Vec3.vec3(0, 0, 0))).toBe(true);
  });

  it('parallel line returns null', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, 5), Vec3.vec3(10, 0, 5)); // above plane, parallel
    expect(Line.intersectPlane(l, xy)).toBeNull();
  });

  it('clamped: hit outside [0, 1] returns null', () => {
    // Segment from z=1 to z=2 — hit would be at t=-1, outside.
    const l = Line.fromPoints(Vec3.vec3(0, 0, 1), Vec3.vec3(0, 0, 2));
    expect(Line.intersectPlane(l, xy)).toBeNull();
  });

  it('unclamped: extrapolated hit is returned', () => {
    const l = Line.fromPoints(Vec3.vec3(0, 0, 1), Vec3.vec3(0, 0, 2));
    const hit = Line.intersectPlane(l, xy, false);
    expect(hit).not.toBeNull();
    expect(hit!.t).toBeCloseTo(-1);
  });
});

describe('Line — closestPair', () => {
  it('skew lines: finds the perpendicular feet', () => {
    // Line A along x-axis, Line B along y-axis at z=1 — their closest
    // approach is along z, distance 1, midpoint at origin in xy.
    const a = Line.fromPoints(Vec3.vec3(-5, 0, 0), Vec3.vec3(5, 0, 0));
    const b = Line.fromPoints(Vec3.vec3(0, -5, 1), Vec3.vec3(0, 5, 1));
    const r = Line.closestPair(a, b);
    expect(r.distance).toBeCloseTo(1);
    expect(Vec3.approxEquals(r.pointA, Vec3.vec3(0, 0, 0))).toBe(true);
    expect(Vec3.approxEquals(r.pointB, Vec3.vec3(0, 0, 1))).toBe(true);
  });

  it('parallel lines: distance equals offset', () => {
    const a = Line.fromPoints(Vec3.vec3(0, 0, 0), Vec3.vec3(10, 0, 0));
    const b = Line.fromPoints(Vec3.vec3(0, 3, 0), Vec3.vec3(10, 3, 0));
    const r = Line.closestPair(a, b);
    expect(r.distance).toBeCloseTo(3);
  });

  it('intersecting lines: distance is zero', () => {
    const a = Line.fromPoints(Vec3.vec3(-5, 0, 0), Vec3.vec3(5, 0, 0));
    const b = Line.fromPoints(Vec3.vec3(0, -5, 0), Vec3.vec3(0, 5, 0));
    const r = Line.closestPair(a, b);
    expect(r.distance).toBeLessThan(1e-9);
  });
});

describe('Line — type guard', () => {
  it('accepts a well-formed line', () => {
    expect(Line.isLine(Line.fromPoints(Vec3.ZERO, Vec3.X_AXIS))).toBe(true);
  });
  it('rejects partial objects', () => {
    expect(Line.isLine({ origin: Vec3.ZERO })).toBe(false);
    expect(Line.isLine(null)).toBe(false);
  });
});

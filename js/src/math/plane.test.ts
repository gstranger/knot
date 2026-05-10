import { describe, expect, it } from 'vitest';
import * as Vec3 from './vec3';
import * as Plane from './plane';

describe('Plane — construction', () => {
  it('plane: orthonormal frame with derived axes', () => {
    const pl = Plane.plane(Vec3.vec3(0, 0, 0), Vec3.Z_AXIS);
    expect(Vec3.approxEquals(pl.normal, Vec3.Z_AXIS)).toBe(true);
    // u and v lie in the xy-plane
    expect(Math.abs(Vec3.dot(pl.uAxis, Vec3.Z_AXIS))).toBeLessThan(1e-12);
    expect(Math.abs(Vec3.dot(pl.vAxis, Vec3.Z_AXIS))).toBeLessThan(1e-12);
    // Orthogonal to each other
    expect(Math.abs(Vec3.dot(pl.uAxis, pl.vAxis))).toBeLessThan(1e-12);
    // Right-handed: normal × u = v
    expect(Vec3.approxEquals(Vec3.cross(pl.normal, pl.uAxis), pl.vAxis)).toBe(true);
  });

  it('plane: throws on zero normal', () => {
    expect(() => Plane.plane(Vec3.ZERO, Vec3.ZERO)).toThrow();
  });

  it('plane normalizes the input normal', () => {
    const pl = Plane.plane(Vec3.ZERO, Vec3.vec3(0, 0, 5));
    expect(Math.abs(Vec3.length(pl.normal) - 1)).toBeLessThan(1e-12);
  });

  it('withFrame: explicit ref direction sets uAxis', () => {
    // Plane in xy with refDirection +x → uAxis = +x, vAxis = +y
    const pl = Plane.withFrame(Vec3.ZERO, Vec3.Z_AXIS, Vec3.X_AXIS);
    expect(Vec3.approxEquals(pl.uAxis, Vec3.X_AXIS)).toBe(true);
    expect(Vec3.approxEquals(pl.vAxis, Vec3.Y_AXIS)).toBe(true);
  });

  it('withFrame: refDirection projected to plane', () => {
    // refDirection has both in-plane and out-of-plane components;
    // the in-plane projection should become uAxis.
    const pl = Plane.withFrame(Vec3.ZERO, Vec3.Z_AXIS, Vec3.vec3(1, 0, 7));
    expect(Vec3.approxEquals(pl.uAxis, Vec3.X_AXIS)).toBe(true);
  });

  it('withFrame: throws when refDirection is parallel to normal', () => {
    expect(() => Plane.withFrame(Vec3.ZERO, Vec3.Z_AXIS, Vec3.Z_AXIS)).toThrow();
  });

  it('fromPoints: three collinear → throws', () => {
    const a = Vec3.vec3(0, 0, 0);
    const b = Vec3.vec3(1, 0, 0);
    const c = Vec3.vec3(2, 0, 0);
    expect(() => Plane.fromPoints(a, b, c)).toThrow();
  });

  it('fromPoints: non-collinear → frame anchored at first point', () => {
    const a = Vec3.vec3(0, 0, 0);
    const b = Vec3.vec3(2, 0, 0);
    const c = Vec3.vec3(0, 3, 0);
    const pl = Plane.fromPoints(a, b, c);
    expect(Vec3.approxEquals(pl.origin, a)).toBe(true);
    // Normal is +z (or -z if right-hand-ruled the other way; check the contains test)
    expect(Plane.contains(pl, a)).toBe(true);
    expect(Plane.contains(pl, b)).toBe(true);
    expect(Plane.contains(pl, c)).toBe(true);
  });
});

describe('Plane — distance and projection', () => {
  const pl = Plane.plane(Vec3.ZERO, Vec3.Z_AXIS);

  it('signedDistance: sign matches normal direction', () => {
    expect(Plane.signedDistance(pl, Vec3.vec3(5, 5, 2))).toBeCloseTo(2);
    expect(Plane.signedDistance(pl, Vec3.vec3(5, 5, -3))).toBeCloseTo(-3);
    expect(Plane.signedDistance(pl, Vec3.vec3(5, 5, 0))).toBeCloseTo(0);
  });

  it('distance is unsigned', () => {
    expect(Plane.distance(pl, Vec3.vec3(0, 0, -7))).toBeCloseTo(7);
  });

  it('project drops onto the plane', () => {
    const projected = Plane.project(pl, Vec3.vec3(3, 4, 9));
    expect(Vec3.approxEquals(projected, Vec3.vec3(3, 4, 0))).toBe(true);
    expect(Plane.contains(pl, projected, 1e-10)).toBe(true);
  });

  it('project of in-plane point is identity', () => {
    const p = Vec3.vec3(2.5, -1, 0);
    expect(Vec3.approxEquals(Plane.project(pl, p), p)).toBe(true);
  });
});

describe('Plane — parameterization', () => {
  // Plane through (10, 0, 0) with normal +z; uAxis = +x by default-ish
  const pl = Plane.withFrame(
    Vec3.vec3(10, 0, 0),
    Vec3.Z_AXIS,
    Vec3.X_AXIS,
  );

  it('pointAt and paramAt are inverses for in-plane points', () => {
    const p = Plane.pointAt(pl, 3, 7);
    expect(Vec3.approxEquals(p, Vec3.vec3(13, 7, 0))).toBe(true);
    const { u, v } = Plane.paramAt(pl, p);
    expect(u).toBeCloseTo(3);
    expect(v).toBeCloseTo(7);
  });

  it('paramAt of an off-plane point gives the projected param', () => {
    const { u, v } = Plane.paramAt(pl, Vec3.vec3(13, 7, 99));
    expect(u).toBeCloseTo(3);
    expect(v).toBeCloseTo(7);
  });
});

describe('Plane — type guard', () => {
  it('accepts well-formed plane', () => {
    const pl = Plane.plane(Vec3.ZERO, Vec3.Z_AXIS);
    expect(Plane.isPlane(pl)).toBe(true);
  });

  it('rejects bare object', () => {
    expect(Plane.isPlane({ origin: Vec3.ZERO })).toBe(false);
  });

  it('rejects null / non-objects', () => {
    expect(Plane.isPlane(null)).toBe(false);
    expect(Plane.isPlane(42)).toBe(false);
  });
});

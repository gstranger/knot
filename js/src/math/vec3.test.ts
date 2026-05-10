import { describe, expect, it } from 'vitest';
import * as Vec3 from './vec3';

describe('Vec3 — basics', () => {
  it('vec3 constructs an immutable triple', () => {
    const v = Vec3.vec3(1, 2, 3);
    expect(v.x).toBe(1);
    expect(v.y).toBe(2);
    expect(v.z).toBe(3);
  });

  it('axis constants', () => {
    expect(Vec3.equals(Vec3.X_AXIS, Vec3.vec3(1, 0, 0))).toBe(true);
    expect(Vec3.equals(Vec3.ONE, Vec3.vec3(1, 1, 1))).toBe(true);
    expect(Vec3.equals(Vec3.ZERO, Vec3.vec3(0, 0, 0))).toBe(true);
  });
});

describe('Vec3 — arithmetic', () => {
  const a = Vec3.vec3(1, 2, 3);
  const b = Vec3.vec3(4, 5, 6);

  it('add / sub / neg', () => {
    expect(Vec3.equals(Vec3.add(a, b), Vec3.vec3(5, 7, 9))).toBe(true);
    expect(Vec3.equals(Vec3.sub(b, a), Vec3.vec3(3, 3, 3))).toBe(true);
    expect(Vec3.equals(Vec3.neg(a), Vec3.vec3(-1, -2, -3))).toBe(true);
  });

  it('scale / mul / div', () => {
    expect(Vec3.equals(Vec3.scale(a, 2), Vec3.vec3(2, 4, 6))).toBe(true);
    expect(Vec3.equals(Vec3.mul(a, b), Vec3.vec3(4, 10, 18))).toBe(true);
    expect(Vec3.approxEquals(Vec3.div(b, a), Vec3.vec3(4, 2.5, 2))).toBe(true);
  });
});

describe('Vec3 — vector ops', () => {
  it('dot / cross', () => {
    expect(Vec3.dot(Vec3.X_AXIS, Vec3.X_AXIS)).toBe(1);
    expect(Vec3.dot(Vec3.X_AXIS, Vec3.Y_AXIS)).toBe(0);
    expect(Vec3.equals(Vec3.cross(Vec3.X_AXIS, Vec3.Y_AXIS), Vec3.Z_AXIS)).toBe(true);
    expect(Vec3.equals(Vec3.cross(Vec3.Y_AXIS, Vec3.X_AXIS), Vec3.neg(Vec3.Z_AXIS))).toBe(true);
  });

  it('length / lengthSq / distance / distanceSq', () => {
    const v = Vec3.vec3(3, 4, 0);
    expect(Vec3.length(v)).toBe(5);
    expect(Vec3.lengthSq(v)).toBe(25);
    expect(Vec3.distance(Vec3.ZERO, v)).toBe(5);
    expect(Vec3.distanceSq(Vec3.ZERO, v)).toBe(25);
  });

  it('normalize: unit vector', () => {
    const n = Vec3.normalize(Vec3.vec3(3, 4, 0));
    expect(Vec3.approxEquals(n, Vec3.vec3(0.6, 0.8, 0))).toBe(true);
    expect(Math.abs(Vec3.length(n) - 1)).toBeLessThan(1e-12);
  });

  it('normalize(0) returns 0 (not NaN)', () => {
    expect(Vec3.equals(Vec3.normalize(Vec3.ZERO), Vec3.ZERO)).toBe(true);
  });

  it('lerp endpoints and midpoint', () => {
    const a = Vec3.vec3(0, 0, 0);
    const b = Vec3.vec3(10, 20, 30);
    expect(Vec3.approxEquals(Vec3.lerp(a, b, 0), a)).toBe(true);
    expect(Vec3.approxEquals(Vec3.lerp(a, b, 1), b)).toBe(true);
    expect(Vec3.approxEquals(Vec3.lerp(a, b, 0.5), Vec3.vec3(5, 10, 15))).toBe(true);
  });

  it('project / reject decompose a vector', () => {
    const a = Vec3.vec3(3, 4, 0);
    const onto = Vec3.X_AXIS;
    const para = Vec3.project(a, onto);
    const perp = Vec3.reject(a, onto);
    expect(Vec3.approxEquals(para, Vec3.vec3(3, 0, 0))).toBe(true);
    expect(Vec3.approxEquals(perp, Vec3.vec3(0, 4, 0))).toBe(true);
    expect(Vec3.approxEquals(Vec3.add(para, perp), a)).toBe(true);
  });

  it('project onto zero is zero (not NaN)', () => {
    expect(Vec3.equals(Vec3.project(Vec3.X_AXIS, Vec3.ZERO), Vec3.ZERO)).toBe(true);
  });

  it('reflect across axis-aligned normal', () => {
    // Vector pointing into +x +y, reflected across the y=0 plane (normal +y) flips y
    const v = Vec3.vec3(1, 1, 0);
    const r = Vec3.reflect(v, Vec3.Y_AXIS);
    expect(Vec3.approxEquals(r, Vec3.vec3(1, -1, 0))).toBe(true);
  });

  it('angleBetween: standard cases', () => {
    expect(Vec3.angleBetween(Vec3.X_AXIS, Vec3.X_AXIS)).toBeCloseTo(0);
    expect(Vec3.angleBetween(Vec3.X_AXIS, Vec3.Y_AXIS)).toBeCloseTo(Math.PI / 2);
    expect(Vec3.angleBetween(Vec3.X_AXIS, Vec3.neg(Vec3.X_AXIS))).toBeCloseTo(Math.PI);
  });

  it('angleBetween clamps cosine to [-1, 1]', () => {
    // f64 drift can push dot/(|a||b|) just past ±1; ensure no NaN.
    const v = Vec3.vec3(1, 0, 0);
    const result = Vec3.angleBetween(v, v);
    expect(Number.isNaN(result)).toBe(false);
  });

  it('min / max / abs component-wise', () => {
    const a = Vec3.vec3(1, -2, 3);
    const b = Vec3.vec3(-1, 5, 2);
    expect(Vec3.equals(Vec3.min(a, b), Vec3.vec3(-1, -2, 2))).toBe(true);
    expect(Vec3.equals(Vec3.max(a, b), Vec3.vec3(1, 5, 3))).toBe(true);
    expect(Vec3.equals(Vec3.abs(a), Vec3.vec3(1, 2, 3))).toBe(true);
  });
});

describe('Vec3 — comparisons', () => {
  it('equals is bit-exact', () => {
    expect(Vec3.equals(Vec3.vec3(0.1 + 0.2, 0, 0), Vec3.vec3(0.3, 0, 0))).toBe(false);
  });

  it('approxEquals tolerates ULP drift', () => {
    expect(Vec3.approxEquals(Vec3.vec3(0.1 + 0.2, 0, 0), Vec3.vec3(0.3, 0, 0))).toBe(true);
  });

  it('approxEquals respects custom tolerance', () => {
    const a = Vec3.vec3(1, 0, 0);
    const b = Vec3.vec3(1.001, 0, 0);
    expect(Vec3.approxEquals(a, b, 1e-9)).toBe(false);
    expect(Vec3.approxEquals(a, b, 1e-2)).toBe(true);
  });
});

describe('Vec3 — conversion + type guard', () => {
  it('fromArray / toArray roundtrip', () => {
    const v = Vec3.vec3(1, 2, 3);
    const arr = Vec3.toArray(v);
    expect(arr).toEqual([1, 2, 3]);
    expect(Vec3.equals(Vec3.fromArray(arr), v)).toBe(true);
  });

  it('fromArray throws on too-short input', () => {
    expect(() => Vec3.fromArray([1, 2])).toThrow();
  });

  it('isVec3 accepts valid, rejects garbage', () => {
    expect(Vec3.isVec3(Vec3.vec3(0, 0, 0))).toBe(true);
    expect(Vec3.isVec3({ x: 1, y: 2, z: 3 })).toBe(true);
    expect(Vec3.isVec3({ x: 1, y: 2 })).toBe(false);
    expect(Vec3.isVec3(null)).toBe(false);
    expect(Vec3.isVec3('hello')).toBe(false);
    expect(Vec3.isVec3({ x: '1', y: '2', z: '3' })).toBe(false);
  });
});

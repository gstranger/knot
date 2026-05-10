import { describe, expect, it } from 'vitest';
import * as Vec3 from './vec3';
import * as Transform from './transform';

describe('Transform — identity', () => {
  it('IDENTITY transforms point and vector unchanged', () => {
    const p = Vec3.vec3(1, 2, 3);
    expect(Vec3.approxEquals(Transform.transformPoint(Transform.IDENTITY, p), p)).toBe(true);
    expect(Vec3.approxEquals(Transform.transformVector(Transform.IDENTITY, p), p)).toBe(true);
  });
});

describe('Transform — translation', () => {
  const t = Transform.translation(Vec3.vec3(10, 20, 30));

  it('moves points', () => {
    const p = Vec3.vec3(1, 2, 3);
    expect(Vec3.approxEquals(Transform.transformPoint(t, p), Vec3.vec3(11, 22, 33))).toBe(true);
  });

  it('does NOT move vectors (direction-preserving)', () => {
    const v = Vec3.vec3(1, 2, 3);
    expect(Vec3.approxEquals(Transform.transformVector(t, v), v)).toBe(true);
  });
});

describe('Transform — rotation', () => {
  it('90° around Z maps +X to +Y', () => {
    const r = Transform.rotation(Vec3.Z_AXIS, Math.PI / 2);
    const out = Transform.transformVector(r, Vec3.X_AXIS);
    expect(Vec3.approxEquals(out, Vec3.Y_AXIS, 1e-12)).toBe(true);
  });

  it('180° around X negates Y and Z', () => {
    const r = Transform.rotation(Vec3.X_AXIS, Math.PI);
    const out = Transform.transformVector(r, Vec3.vec3(0, 1, 1));
    expect(Vec3.approxEquals(out, Vec3.vec3(0, -1, -1), 1e-12)).toBe(true);
  });

  it('full revolution returns to identity (within ULP)', () => {
    const r = Transform.rotation(Vec3.Z_AXIS, 2 * Math.PI);
    const out = Transform.transformVector(r, Vec3.X_AXIS);
    expect(Vec3.approxEquals(out, Vec3.X_AXIS, 1e-10)).toBe(true);
  });

  it('throws on zero axis', () => {
    expect(() => Transform.rotation(Vec3.ZERO, 1)).toThrow();
  });
});

describe('Transform — scale', () => {
  it('non-uniform scales components independently', () => {
    const s = Transform.scale(Vec3.vec3(2, 3, 4));
    expect(Vec3.approxEquals(Transform.transformPoint(s, Vec3.vec3(1, 1, 1)), Vec3.vec3(2, 3, 4))).toBe(true);
  });

  it('uniform scale multiplies all components', () => {
    const s = Transform.scaleUniform(5);
    expect(Vec3.approxEquals(Transform.transformPoint(s, Vec3.vec3(1, 2, 3)), Vec3.vec3(5, 10, 15))).toBe(true);
  });
});

describe('Transform — composition', () => {
  it('translate then rotate: rotation runs first (right-most matrix first)', () => {
    // Compose: T(10, 0, 0) · R(z, 90°). Apply to +X:
    //   R(z,90°) takes +X → +Y
    //   then T translates +Y by (10,0,0) → (10, 1, 0)
    const r = Transform.rotation(Vec3.Z_AXIS, Math.PI / 2);
    const t = Transform.translation(Vec3.vec3(10, 0, 0));
    const composed = Transform.compose(t, r);
    const out = Transform.transformPoint(composed, Vec3.X_AXIS);
    expect(Vec3.approxEquals(out, Vec3.vec3(10, 1, 0), 1e-12)).toBe(true);
  });

  it('compose with identity is identity', () => {
    const t = Transform.translation(Vec3.vec3(1, 2, 3));
    expect(approxMat(Transform.compose(t, Transform.IDENTITY), t)).toBe(true);
    expect(approxMat(Transform.compose(Transform.IDENTITY, t), t)).toBe(true);
  });
});

describe('Transform — inverse', () => {
  it('inverse of translation is negative translation', () => {
    const t = Transform.translation(Vec3.vec3(1, 2, 3));
    const inv = Transform.inverse(t)!;
    expect(inv).not.toBeNull();
    const out = Transform.transformPoint(inv, Vec3.vec3(11, 22, 33));
    expect(Vec3.approxEquals(out, Vec3.vec3(10, 20, 30), 1e-12)).toBe(true);
  });

  it('inverse of rotation is negated angle', () => {
    const r = Transform.rotation(Vec3.Z_AXIS, Math.PI / 3);
    const inv = Transform.inverse(r)!;
    const composed = Transform.compose(r, inv);
    expect(approxMat(composed, Transform.IDENTITY, 1e-10)).toBe(true);
  });

  it('singular matrix returns null', () => {
    // Zero scale collapses everything → singular
    const s = Transform.scale(Vec3.vec3(0, 1, 1));
    expect(Transform.inverse(s)).toBeNull();
  });

  it('inverse(inverse(t)) ≈ t for invertible t', () => {
    const t = Transform.compose(
      Transform.translation(Vec3.vec3(5, -3, 2)),
      Transform.rotation(Vec3.X_AXIS, 0.7),
    );
    const inv2 = Transform.inverse(Transform.inverse(t)!)!;
    expect(approxMat(t, inv2, 1e-10)).toBe(true);
  });
});

describe('Transform — normal transform', () => {
  it('rigid rotation: transforms normal like a vector', () => {
    const r = Transform.rotation(Vec3.Z_AXIS, Math.PI / 2);
    const n = Vec3.X_AXIS;
    const transformed = Transform.transformNormal(r, n);
    expect(Vec3.approxEquals(Vec3.normalize(transformed), Vec3.Y_AXIS, 1e-10)).toBe(true);
  });

  it('non-uniform scale: normal transformed by inverse-transpose', () => {
    // Scale x by 2, y by 1: a normal pointing in +x direction on a
    // surface should remain pointing in +x (axis-aligned scale of an
    // axis-aligned surface). The inverse-transpose for scale(2,1,1)
    // is scale(1/2, 1, 1) — applied to (1,0,0) gives (0.5, 0, 0),
    // which normalized is +x.
    const s = Transform.scale(Vec3.vec3(2, 1, 1));
    const n = Vec3.normalize(Transform.transformNormal(s, Vec3.X_AXIS));
    expect(Vec3.approxEquals(n, Vec3.X_AXIS, 1e-12)).toBe(true);
  });
});

describe('Transform — type guard', () => {
  it('accepts well-formed transform', () => {
    expect(Transform.isTransform(Transform.IDENTITY)).toBe(true);
  });
  it('rejects wrong-length array', () => {
    expect(Transform.isTransform({ m: [1, 2, 3] })).toBe(false);
  });
  it('rejects non-array m', () => {
    expect(Transform.isTransform({ m: 'hello' })).toBe(false);
  });
});

// Helper: compare two transforms element-wise within tolerance.
function approxMat(a: Transform.Transform, b: Transform.Transform, tol = 1e-12): boolean {
  for (let i = 0; i < 16; i++) {
    if (Math.abs(a.m[i]! - b.m[i]!) > tol) return false;
  }
  return true;
}

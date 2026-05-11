/**
 * End-to-end tests for the Track A Curve op surface — every new method
 * routes Rust → WASM → typed JS wrapper, so these tests double as a
 * smoke test for the binding layer.
 */
import { describe, it, expect, beforeAll } from 'vitest';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createKnot } from './kernel';
import type { Knot } from './kernel';

let knot: Knot;

beforeAll(async () => {
  const here = fileURLToPath(import.meta.url);
  knot = await createKnot(await readFile(resolve(here, '../../../pkg/knot_bg.wasm')));
}, 30_000);

describe('Curve — length', () => {
  it('line length matches euclidean distance', () => {
    const line = knot.line({ x: 0, y: 0, z: 0 }, { x: 3, y: 4, z: 0 });
    expect(line.length()).toBeCloseTo(5, 9);
    line.free();
  });

  it('quarter circle of radius 2 has length π', () => {
    const arc = knot.arc({
      center: { x: 0, y: 0, z: 0 },
      normal: { x: 0, y: 0, z: 1 },
      radius: 2,
      refAxis: { x: 1, y: 0, z: 0 },
      startAngle: 0,
      endAngle: Math.PI / 2,
    });
    expect(arc.length()).toBeCloseTo(Math.PI, 6);
    arc.free();
  });
});

describe('Curve — splitAt', () => {
  it('splits a line at its midpoint into two equal-length halves', () => {
    const line = knot.line({ x: 0, y: 0, z: 0 }, { x: 10, y: 0, z: 0 });
    const { left, right } = line.splitAt(0.5);
    expect(left.length()).toBeCloseTo(5, 9);
    expect(right.length()).toBeCloseTo(5, 9);
    line.free();
    left.free();
    right.free();
  });

  it('rejects splits at the domain endpoints', () => {
    const line = knot.line({ x: 0, y: 0, z: 0 }, { x: 1, y: 0, z: 0 });
    expect(() => line.splitAt(0)).toThrow();
    expect(() => line.splitAt(1)).toThrow();
    line.free();
  });
});

describe('Curve — reverse', () => {
  it('reversed line starts at the original endpoint', () => {
    const line = knot.line({ x: 0, y: 0, z: 0 }, { x: 1, y: 2, z: 3 });
    const rev = line.reverse();
    const startOfRev = rev.pointAt(0);
    expect(startOfRev.x).toBeCloseTo(1, 9);
    expect(startOfRev.y).toBeCloseTo(2, 9);
    expect(startOfRev.z).toBeCloseTo(3, 9);
    line.free();
    rev.free();
  });
});

describe('Curve — derivativesAt', () => {
  it('returns point, first, and second derivatives for a line', () => {
    const line = knot.line({ x: 0, y: 0, z: 0 }, { x: 4, y: 0, z: 0 });
    const d = line.derivativesAt(0.25);
    expect(d.point.x).toBeCloseTo(1, 9);
    // first derivative for a line on [0,1] is (end-start) = (4,0,0)
    expect(d.d1.x).toBeCloseTo(4, 9);
    // second derivative of a line is zero
    expect(d.d2).not.toBeNull();
    expect(d.d2!.x).toBeCloseTo(0, 9);
    line.free();
  });
});

describe('Curve — divideByLength', () => {
  it('divides a line into n equal-length segments', () => {
    const line = knot.line({ x: 0, y: 0, z: 0 }, { x: 10, y: 0, z: 0 });
    const params = line.divideByLength(4);
    expect(params).toHaveLength(5);
    // Line is parameterized on [0, 1]; equal arc length = equal param.
    expect(params[0]).toBeCloseTo(0, 6);
    expect(params[2]).toBeCloseTo(0.5, 6);
    expect(params[4]).toBeCloseTo(1, 6);
    line.free();
  });
});

describe('Curve — intersect', () => {
  it('finds the crossing of two perpendicular line segments', () => {
    const a = knot.line({ x: -1, y: 0, z: 0 }, { x: 1, y: 0, z: 0 });
    const b = knot.line({ x: 0, y: -1, z: 0 }, { x: 0, y: 1, z: 0 });
    const hits = a.intersect(b);
    expect(hits.length).toBeGreaterThanOrEqual(1);
    const hit = hits[0]!;
    expect(hit.point.x).toBeCloseTo(0, 6);
    expect(hit.point.y).toBeCloseTo(0, 6);
    expect(hit.paramA).toBeCloseTo(0.5, 6);
    expect(hit.paramB).toBeCloseTo(0.5, 6);
    a.free();
    b.free();
  });

  it('returns no hits when the segments do not cross', () => {
    const a = knot.line({ x: 0, y: 0, z: 0 }, { x: 1, y: 0, z: 0 });
    const b = knot.line({ x: 0, y: 1, z: 0 }, { x: 1, y: 1, z: 0 });
    expect(a.intersect(b)).toHaveLength(0);
    a.free();
    b.free();
  });
});

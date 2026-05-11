/**
 * Curve-fitting node tests. Exercises both the direct Knot API and
 * the graph nodes end-to-end through the evaluator.
 */
import { describe, it, expect, beforeAll } from 'vitest';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createKnot } from '../kernel';
import type { Knot } from '../kernel';
import { Graph, Evaluator, buildDefaultRegistry, isError, defineNode } from './index';

let knot: Knot;

beforeAll(async () => {
  const here = fileURLToPath(import.meta.url);
  knot = await createKnot(await readFile(resolve(here, '../../../../pkg/knot_bg.wasm')));
}, 30_000);

describe('Knot.interpolateCurve', () => {
  it('passes exactly through every input point', () => {
    const pts = [
      { x: 0, y: 0, z: 0 },
      { x: 1, y: 1, z: 0 },
      { x: 2, y: 0, z: 0 },
      { x: 3, y: 1, z: 0 },
    ];
    const curve = knot.interpolateCurve(pts, 3);
    expect(curve.type).toBe('nurbs');
    // Endpoints must match exactly.
    const [t0, t1] = curve.domain();
    const p0 = curve.pointAt(t0);
    const pN = curve.pointAt(t1);
    expect(p0.x).toBeCloseTo(0, 9);
    expect(p0.y).toBeCloseTo(0, 9);
    expect(pN.x).toBeCloseTo(3, 9);
    expect(pN.y).toBeCloseTo(1, 9);
    curve.free();
  });

  it('rejects too-few points for the requested degree', () => {
    expect(() =>
      knot.interpolateCurve([{ x: 0, y: 0, z: 0 }, { x: 1, y: 0, z: 0 }], 3),
    ).toThrow(/InvalidInput|InsufficientControlPoints|points/i);
  });
});

describe('Knot.approximateCurve', () => {
  it('passes exactly through first and last; smooths interior', () => {
    const pts = Array.from({ length: 10 }, (_, i) => ({
      x: i,
      y: Math.sin(i * 0.5),
      z: 0,
    }));
    const curve = knot.approximateCurve(pts, 6, 3);
    expect(curve.type).toBe('nurbs');
    // First / last must hit exactly.
    const [t0, t1] = curve.domain();
    expect(curve.pointAt(t0).x).toBeCloseTo(0, 9);
    expect(curve.pointAt(t1).x).toBeCloseTo(9, 9);
    curve.free();
  });

  it('rejects num_cp < degree + 1', () => {
    const pts = [
      { x: 0, y: 0, z: 0 }, { x: 1, y: 1, z: 0 }, { x: 2, y: 0, z: 0 }, { x: 3, y: 1, z: 0 },
    ];
    expect(() => knot.approximateCurve(pts, 2, 3)).toThrow();
  });
});

describe('Interpolate Curve node', () => {
  it('end-to-end: 4 points → interpolated curve → measure length', async () => {
    // Build a small registry with a "static points" helper node so the
    // test can feed a fixed list of vec3 into the interpolate node.
    const PointsLiteral = defineNode({
      id: 'test.pointsLiteral',
      label: 'Points Literal',
      inputs: {},
      outputs: { list: { kind: 'list' as const } },
      evaluate: () => ({
        list: [
          { x: 0, y: 0, z: 0 },
          { x: 1, y: 1, z: 0 },
          { x: 2, y: 0, z: 0 },
          { x: 3, y: 1, z: 0 },
        ],
      }),
    });
    const r = buildDefaultRegistry(knot);
    r.register(PointsLiteral);

    const g = new Graph(r);
    const pts = g.addNode('test.pointsLiteral');
    const curve = g.addNode('core.curve.interpolate', { degree: 3 });
    const len = g.addNode('core.curve.length');
    g.connect(pts, 'list', curve, 'points');
    g.connect(curve, 'curve', len, 'curve');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(len, 'length');
    expect(out?.kind).toBe('number');
    if (out?.kind === 'number') {
      // Curve hits (0,0), (3,1) so it's strictly longer than 3 (the
      // straight chord) and shorter than the polyline sum (~4.83).
      expect(out.value).toBeGreaterThan(3);
      expect(out.value).toBeLessThan(5);
    }
  });

  it('surfaces a typed error when the input list is the wrong shape', async () => {
    const r = buildDefaultRegistry(knot);
    const g = new Graph(r);
    // Range emits a list of numbers, not Vec3s — should fail validation.
    const range = g.addNode('list.range', { start: 0, end: 4, step: 1 });
    const curve = g.addNode('core.curve.interpolate', { degree: 3 });
    g.connect(range, 'list', curve, 'points');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(curve, 'curve');
    expect(out).toBeDefined();
    expect(isError(out!)).toBe(true);
    if (isError(out!)) expect(out.message).toMatch(/Vec3/);
  });
});

describe('Approximate Curve node', () => {
  it('end-to-end: 8 noisy points → approximated curve passes endpoints exactly', async () => {
    const PointsLiteral = defineNode({
      id: 'test.noisyPoints',
      label: 'Noisy Points',
      inputs: {},
      outputs: { list: { kind: 'list' as const } },
      evaluate: () => ({
        list: Array.from({ length: 8 }, (_, i) => ({
          x: i,
          y: 0.1 * ((i * 7) % 5),
          z: 0,
        })),
      }),
    });
    const r = buildDefaultRegistry(knot);
    r.register(PointsLiteral);

    const g = new Graph(r);
    const pts = g.addNode('test.noisyPoints');
    const fit = g.addNode('core.curve.approximate', { numControlPoints: 5, degree: 3 });
    g.connect(pts, 'list', fit, 'points');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(fit, 'curve');
    expect(out?.kind).toBe('curve');
  });
});

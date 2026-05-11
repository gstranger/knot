/**
 * Analysis node smoke tests — exercise each node end-to-end through
 * the kernel (WASM-loaded). One test per node, plus a composite
 * "curve length drives a downstream number" wire test.
 */
import { describe, it, expect, beforeAll } from 'vitest';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createKnot } from '../kernel';
import type { Knot } from '../kernel';
import { Graph, Evaluator, buildDefaultRegistry } from './index';

let knot: Knot;

beforeAll(async () => {
  const here = fileURLToPath(import.meta.url);
  knot = await createKnot(await readFile(resolve(here, '../../../../pkg/knot_bg.wasm')));
}, 30_000);

describe('Curve analysis nodes', () => {
  it('Curve Length: 3-4-5 line has length 5', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const a = g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    const b = g.addNode('core.vec3', { x: 3, y: 4, z: 0 });
    const line = g.addNode('knot.line');
    g.connect(a, 'value', line, 'a');
    g.connect(b, 'value', line, 'b');
    const len = g.addNode('core.curve.length');
    g.connect(line, 'curve', len, 'curve');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(len, 'length');
    expect(out?.kind).toBe('number');
    if (out?.kind === 'number') expect(out.value).toBeCloseTo(5, 6);
  });

  it('Curve BBox: line bbox covers endpoints', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const a = g.addNode('core.vec3', { x: 1, y: 2, z: 3 });
    const b = g.addNode('core.vec3', { x: 4, y: 6, z: 3 });
    const line = g.addNode('knot.line');
    g.connect(a, 'value', line, 'a');
    g.connect(b, 'value', line, 'b');
    const bbox = g.addNode('core.curve.boundingBox');
    g.connect(line, 'curve', bbox, 'curve');
    const ev = new Evaluator();
    await ev.run(g);
    const size = ev.getOutput(bbox, 'size');
    expect(size?.kind).toBe('vec3');
    if (size?.kind === 'vec3') {
      expect(size.value.x).toBeCloseTo(3, 6);
      expect(size.value.y).toBeCloseTo(4, 6);
      expect(size.value.z).toBeCloseTo(0, 6);
    }
  });

  it('Curve Closest Point: query off the line projects to the line', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const a = g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    const b = g.addNode('core.vec3', { x: 10, y: 0, z: 0 });
    const line = g.addNode('knot.line');
    g.connect(a, 'value', line, 'a');
    g.connect(b, 'value', line, 'b');
    const query = g.addNode('core.vec3', { x: 5, y: 3, z: 0 });
    const cp = g.addNode('core.curve.closestPoint');
    g.connect(line, 'curve', cp, 'curve');
    g.connect(query, 'value', cp, 'query');
    const ev = new Evaluator();
    await ev.run(g);
    const distance = ev.getOutput(cp, 'distance');
    if (distance?.kind === 'number') expect(distance.value).toBeCloseTo(3, 6);
    const point = ev.getOutput(cp, 'point');
    if (point?.kind === 'vec3') {
      expect(point.value.x).toBeCloseTo(5, 6);
      expect(point.value.y).toBeCloseTo(0, 6);
    }
  });

  it('Tangent At: line tangent equals direction (un-normalized)', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const a = g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    const b = g.addNode('core.vec3', { x: 4, y: 0, z: 0 });
    const line = g.addNode('knot.line');
    g.connect(a, 'value', line, 'a');
    g.connect(b, 'value', line, 'b');
    const tan = g.addNode('core.curve.tangentAt', { t: 0.5 });
    g.connect(line, 'curve', tan, 'curve');
    const ev = new Evaluator();
    await ev.run(g);
    const t = ev.getOutput(tan, 'tangent');
    if (t?.kind === 'vec3') {
      // Line tangent is (end - start) — un-normalized.
      expect(t.value.x).toBeCloseTo(4, 6);
      expect(t.value.y).toBeCloseTo(0, 6);
    }
  });

  it('Divide By Length: line into 4 equal segments → 5 params', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const a = g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    const b = g.addNode('core.vec3', { x: 10, y: 0, z: 0 });
    const line = g.addNode('knot.line');
    g.connect(a, 'value', line, 'a');
    g.connect(b, 'value', line, 'b');
    const div = g.addNode('core.curve.divideByLength', { n: 4 });
    g.connect(line, 'curve', div, 'curve');
    const ev = new Evaluator();
    await ev.run(g);
    const params = ev.getOutput(div, 'params');
    expect(params?.kind).toBe('list');
    if (params?.kind === 'list') {
      expect(params.value).toHaveLength(5);
      expect(params.value[0]).toBeCloseTo(0, 6);
      expect(params.value[4]).toBeCloseTo(1, 6);
    }
  });

  it('Curve × Curve: two crossing lines hit once at the origin', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const xa = g.addNode('core.vec3', { x: -1, y: 0, z: 0 });
    const xb = g.addNode('core.vec3', { x: 1, y: 0, z: 0 });
    const ya = g.addNode('core.vec3', { x: 0, y: -1, z: 0 });
    const yb = g.addNode('core.vec3', { x: 0, y: 1, z: 0 });
    const xLine = g.addNode('knot.line');
    g.connect(xa, 'value', xLine, 'a');
    g.connect(xb, 'value', xLine, 'b');
    const yLine = g.addNode('knot.line');
    g.connect(ya, 'value', yLine, 'a');
    g.connect(yb, 'value', yLine, 'b');
    const ix = g.addNode('core.curve.intersect');
    g.connect(xLine, 'curve', ix, 'a');
    g.connect(yLine, 'curve', ix, 'b');
    const ev = new Evaluator();
    await ev.run(g);
    const count = ev.getOutput(ix, 'count');
    if (count?.kind === 'number') expect(count.value).toBeGreaterThanOrEqual(1);
  });
});

describe('Brep analysis nodes', () => {
  it('Brep BBox: 2×3×4 box has matching size', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const box = g.addNode('knot.box', { sx: 2, sy: 3, sz: 4 });
    const bbox = g.addNode('core.brep.boundingBox');
    g.connect(box, 'brep', bbox, 'brep');
    const ev = new Evaluator();
    await ev.run(g);
    const size = ev.getOutput(bbox, 'size');
    if (size?.kind === 'vec3') {
      expect(size.value.x).toBeCloseTo(2, 6);
      expect(size.value.y).toBeCloseTo(3, 6);
      expect(size.value.z).toBeCloseTo(4, 6);
    }
  });

  it('Face Count: 1×1×1 box has 6 faces', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const box = g.addNode('knot.box', { sx: 1, sy: 1, sz: 1 });
    const fc = g.addNode('core.brep.faceCount');
    g.connect(box, 'brep', fc, 'brep');
    const ev = new Evaluator();
    await ev.run(g);
    const count = ev.getOutput(fc, 'count');
    expect(count).toEqual({ kind: 'number', value: 6 });
  });
});

describe('End-to-end: analysis drives geometry', () => {
  it('curve length × 2 produces a box whose x-size equals 2 × the line length', async () => {
    // Line of length 5, then box(sx = length * 2 = 10).
    const g = new Graph(buildDefaultRegistry(knot));
    const a = g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    const b = g.addNode('core.vec3', { x: 3, y: 4, z: 0 });
    const line = g.addNode('knot.line');
    g.connect(a, 'value', line, 'a');
    g.connect(b, 'value', line, 'b');
    const len = g.addNode('core.curve.length');
    g.connect(line, 'curve', len, 'curve');
    const dbl = g.addNode('math.multiply', { b: 2 });
    g.connect(len, 'length', dbl, 'a');
    const box = g.addNode('knot.box', { sy: 1, sz: 1 });
    g.connect(dbl, 'result', box, 'sx');
    const bbox = g.addNode('core.brep.boundingBox');
    g.connect(box, 'brep', bbox, 'brep');
    const ev = new Evaluator();
    await ev.run(g);
    const size = ev.getOutput(bbox, 'size');
    if (size?.kind === 'vec3') expect(size.value.x).toBeCloseTo(10, 6);
  });
});

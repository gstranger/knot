/**
 * Offset node smoke — verifies a line and an arc offset behave as the
 * underlying knot-geom math says they should, and that an Offset on an
 * unsupported curve type produces a poisoned output port instead of
 * throwing through the runtime.
 */
import { describe, it, expect, beforeAll } from 'vitest';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createKnot } from '../kernel';
import type { Knot } from '../kernel';
import { Graph, Evaluator, buildDefaultRegistry, isError } from './index';

let knot: Knot;

beforeAll(async () => {
  const here = fileURLToPath(import.meta.url);
  knot = await createKnot(await readFile(resolve(here, '../../../../pkg/knot_bg.wasm')));
}, 30_000);

describe('Offset node', () => {
  it('offsets a line in +Y when plane normal is +Z', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const a = g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    const b = g.addNode('core.vec3', { x: 2, y: 0, z: 0 });
    const line = g.addNode('knot.line');
    g.connect(a, 'value', line, 'a');
    g.connect(b, 'value', line, 'b');

    const off = g.addNode('core.curve.offset', { distance: 0.5 });
    g.connect(line, 'curve', off, 'curve');

    // Read offset start point via PointAt(t=0).
    const t0 = g.addNode('core.number', { value: 0 });
    const pt = g.addNode('core.curve.pointAt');
    g.connect(off, 'curve', pt, 'curve');
    g.connect(t0, 'value', pt, 't');

    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(pt, 'point');
    expect(out?.kind).toBe('vec3');
    if (out?.kind === 'vec3') {
      expect(out.value.x).toBeCloseTo(0, 9);
      expect(out.value.y).toBeCloseTo(0.5, 9);
      expect(out.value.z).toBeCloseTo(0, 9);
    }
    ev.dispose();
  }, 30_000);

  it('shrinks an arc radius for positive offset in the arc plane', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const arc = g.addNode('knot.arc', { radius: 1 });
    const off = g.addNode('core.curve.offset', { distance: 0.25 });
    g.connect(arc, 'curve', off, 'curve');

    // Sample point at arc parameter 0 (= start angle 0). For a unit arc on
    // XY with ref +X, that's (1, 0, 0); after a 0.25 inward offset it
    // should land at (0.75, 0, 0).
    const t0 = g.addNode('core.number', { value: 0 });
    const pt = g.addNode('core.curve.pointAt');
    g.connect(off, 'curve', pt, 'curve');
    g.connect(t0, 'value', pt, 't');

    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(pt, 'point');
    expect(out?.kind).toBe('vec3');
    if (out?.kind === 'vec3') {
      expect(out.value.x).toBeCloseTo(0.75, 9);
      expect(out.value.y).toBeCloseTo(0, 9);
    }
    ev.dispose();
  }, 30_000);

  it('produces a poisoned port instead of throwing on degenerate offset', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    // Arc of radius 1, distance 1.5 → would invert the radius.
    const arc = g.addNode('knot.arc', { radius: 1 });
    const off = g.addNode('core.curve.offset', { distance: 1.5 });
    g.connect(arc, 'curve', off, 'curve');

    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(off, 'curve');
    expect(out).toBeDefined();
    expect(out && isError(out)).toBe(true);
    ev.dispose();
  }, 30_000);
});

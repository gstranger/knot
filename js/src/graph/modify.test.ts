/**
 * Edge enumeration + Fillet / Chamfer node tests.
 *
 * Box → BrepEdges → Fillet/Chamfer is the canonical "round every
 * edge of this part" pipeline. These tests drive it end-to-end and
 * also check that misshapen list inputs fail with a useful error
 * port instead of corrupting the kernel call.
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

describe('Brep Edges node', () => {
  it('1×1×1 box yields 12 unique edges', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const box = g.addNode('knot.box', { sx: 1, sy: 1, sz: 1 });
    const edges = g.addNode('core.brep.edges');
    g.connect(box, 'brep', edges, 'brep');
    const ev = new Evaluator();
    await ev.run(g);
    const count = ev.getOutput(edges, 'count');
    expect(count).toEqual({ kind: 'number', value: 12 });
  });

  it('edges output carries EdgeRef shape', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const box = g.addNode('knot.box', { sx: 2, sy: 2, sz: 2 });
    const edges = g.addNode('core.brep.edges');
    g.connect(box, 'brep', edges, 'brep');
    const ev = new Evaluator();
    await ev.run(g);
    const list = ev.getOutput(edges, 'edges');
    expect(list?.kind).toBe('list');
    if (list?.kind === 'list') {
      const first = list.value[0] as { start: unknown; end: unknown };
      expect(first).toHaveProperty('start');
      expect(first).toHaveProperty('end');
      expect(typeof (first.start as { x: number }).x).toBe('number');
    }
  });
});

describe('Fillet node', () => {
  it('end-to-end through the node graph: box → edges → fillet (single edge)', async () => {
    // Filleting all 12 edges at once exercises corner-blend topology
    // that the current kernel fillet doesn't yet handle. We use a
    // small test-only node that picks just the first edge out of the
    // list, then pipe that into Fillet.
    const { defineNode } = await import('./index');
    const PickFirstEdge = defineNode({
      id: 'test.pickFirstEdge',
      label: 'Pick First Edge',
      inputs: { list: { kind: 'list' as const, default: [] } },
      outputs: { list: { kind: 'list' as const } },
      evaluate: ({ list }) => ({ list: list.length > 0 ? [list[0]] : [] }),
    });
    const r = buildDefaultRegistry(knot);
    r.register(PickFirstEdge);

    const g = new Graph(r);
    const box = g.addNode('knot.box', { sx: 2, sy: 2, sz: 2 });
    const edges = g.addNode('core.brep.edges');
    const pick = g.addNode('test.pickFirstEdge');
    const fillet = g.addNode('core.brep.fillet', { radius: 0.2 });
    g.connect(box, 'brep', edges, 'brep');
    g.connect(edges, 'edges', pick, 'list');
    g.connect(box, 'brep', fillet, 'brep');
    g.connect(pick, 'list', fillet, 'edges');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(fillet, 'brep');
    if (out && isError(out)) {
      throw new Error(`Fillet failed: ${out.message}`);
    }
    expect(out?.kind).toBe('brep');
  });

  it('fillet with a list of non-edge objects → error port (no kernel crash)', async () => {
    const g = new Graph(buildDefaultRegistry(knot));
    const box = g.addNode('knot.box', { sx: 1, sy: 1, sz: 1 });
    // Manually build a "list" with a bogus element via a custom node.
    // Easier: pipe a Range (numbers, not EdgeRefs) into the edges input.
    const bogus = g.addNode('list.range', { start: 0, end: 3, step: 1 });
    const fillet = g.addNode('core.brep.fillet', { radius: 0.1 });
    g.connect(box, 'brep', fillet, 'brep');
    g.connect(bogus, 'list', fillet, 'edges');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(fillet, 'brep');
    expect(out).toBeDefined();
    expect(isError(out!)).toBe(true);
    if (isError(out!)) {
      expect(out.message).toMatch(/EdgeRef/);
    }
  });
});

describe('Chamfer node', () => {
  it('end-to-end through the node graph: box → edges → chamfer (single edge)', async () => {
    const { defineNode } = await import('./index');
    const PickFirst = defineNode({
      id: 'test.pickFirstEdgeChamfer',
      label: 'Pick First Edge',
      inputs: { list: { kind: 'list' as const, default: [] } },
      outputs: { list: { kind: 'list' as const } },
      evaluate: ({ list }) => ({ list: list.length > 0 ? [list[0]] : [] }),
    });
    const r = buildDefaultRegistry(knot);
    r.register(PickFirst);

    const g = new Graph(r);
    const box = g.addNode('knot.box', { sx: 2, sy: 2, sz: 2 });
    const edges = g.addNode('core.brep.edges');
    const pick = g.addNode('test.pickFirstEdgeChamfer');
    const chamfer = g.addNode('core.brep.chamfer', { distance: 0.1 });
    g.connect(box, 'brep', edges, 'brep');
    g.connect(edges, 'edges', pick, 'list');
    g.connect(box, 'brep', chamfer, 'brep');
    g.connect(pick, 'list', chamfer, 'edges');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(chamfer, 'brep');
    if (out && isError(out)) {
      throw new Error(`Chamfer failed: ${out.message}`);
    }
    expect(out?.kind).toBe('brep');
  });
});

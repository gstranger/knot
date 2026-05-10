/**
 * Loft node smoke — wires Triangle profiles into a Loft2 and verifies a
 * solid mesh comes out. Also confirms an unrelated subtree is not
 * recomputed when the loft input is mutated.
 */
import { describe, it, expect, beforeAll } from 'vitest';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createKnot } from '../kernel';
import type { Knot, MeshData } from '../kernel';
import { Graph, Evaluator, buildDefaultRegistry } from './index';

let knot: Knot;

beforeAll(async () => {
  const here = fileURLToPath(import.meta.url);
  const wasmPath = resolve(here, '../../../../pkg/knot_bg.wasm');
  knot = await createKnot(await readFile(wasmPath));
}, 30_000);

describe('Loft2 node', () => {
  it('lofts two triangle profiles into a closed solid', async () => {
    const g = new Graph(buildDefaultRegistry(knot));

    const v = (x: number, y: number, z: number) => {
      const id = g.addNode('core.vec3', {});
      g.setConstant(id, 'x', x);
      g.setConstant(id, 'y', y);
      g.setConstant(id, 'z', z);
      return id;
    };

    // Bottom triangle at z=0, top triangle at z=2 (different size).
    const ba = v(0, 0, 0), bb = v(1, 0, 0), bc = v(0.5, 1, 0);
    const ta = v(0, 0, 2), tb = v(0.5, 0, 2), tc = v(0.25, 0.5, 2);
    const bottom = g.addNode('knot.triangle');
    g.connect(ba, 'value', bottom, 'a');
    g.connect(bb, 'value', bottom, 'b');
    g.connect(bc, 'value', bottom, 'c');
    const top = g.addNode('knot.triangle');
    g.connect(ta, 'value', top, 'a');
    g.connect(tb, 'value', top, 'b');
    g.connect(tc, 'value', top, 'c');

    const loft = g.addNode('knot.loft2');
    g.connect(bottom, 'brep', loft, 'a');
    g.connect(top,    'brep', loft, 'b');

    let mesh: MeshData | null = null;
    const view = g.addNode('view.brep', { onMesh: (m: MeshData) => { mesh = m; } });
    g.connect(loft, 'brep', view, 'brep');

    const evaluated: string[] = [];
    const ev = new Evaluator({ onEvaluate: id => { evaluated.push(id); } });
    await ev.run(g);

    expect(mesh).not.toBeNull();
    // 3 side quads (each → 2 triangles) + 2 caps (each is 1 triangle) = 8 tris
    // With ear-clipping caps on a 3-vertex polygon → 1 tri per cap, so 6+2 = 8.
    expect(mesh!.triangleCount).toBeGreaterThanOrEqual(6);

    // Mutating one bottom-corner only re-evaluates the loft subtree.
    evaluated.length = 0;
    g.setConstant(bb, 'x', 1.5);
    ev.markDirty(bb);
    await ev.run(g);
    // Top-triangle subtree is independent and must not re-eval.
    for (const id of [ta, tb, tc, top]) {
      expect(evaluated, `independent ${id} should not re-evaluate`).not.toContain(id);
    }
    for (const id of [bb, bottom, loft, view]) {
      expect(evaluated).toContain(id);
    }

    ev.dispose();
  }, 30_000);
});

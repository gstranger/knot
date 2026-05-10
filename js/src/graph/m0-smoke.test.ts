/**
 * M0 acceptance test — the goal of this milestone.
 *
 * Builds a graph: two boxes driven by Number nodes -> Translate one ->
 * Boolean(subtract) -> View. Mutates a number, asserts that:
 *   1. only the affected subtree re-evaluates, and
 *   2. the rendered mesh actually changed.
 *
 * If this test passes, the M0 runtime spine is real.
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
  const bytes = await readFile(wasmPath);
  knot = await createKnot(bytes);
}, 30_000);

describe('M0 smoke — parametric boolean pipeline', () => {
  it('builds, evaluates, mutates, recomputes only the dirty subtree', async () => {
    const registry = buildDefaultRegistry(knot);
    const g = new Graph(registry);

    // Sliders (Number nodes) for box-A size and box-B offset along X.
    const sizeA = g.addNode('core.number', { value: 2 });
    const sizeB = g.addNode('core.number', { value: 1.5 });
    const offX  = g.addNode('core.number', { value: 0.8 });
    const zero  = g.addNode('core.number', { value: 0 });

    // Two boxes.
    const boxA = g.addNode('knot.box');
    g.connect(sizeA, 'value', boxA, 'sx');
    g.connect(sizeA, 'value', boxA, 'sy');
    g.connect(sizeA, 'value', boxA, 'sz');

    const boxB = g.addNode('knot.box');
    g.connect(sizeB, 'value', boxB, 'sx');
    g.connect(sizeB, 'value', boxB, 'sy');
    g.connect(sizeB, 'value', boxB, 'sz');

    // Translate box B by (offX, 0, 0).
    const offset = g.addNode('core.vec3');
    g.connect(offX, 'value', offset, 'x');
    g.connect(zero, 'value', offset, 'y');
    g.connect(zero, 'value', offset, 'z');

    const trans = g.addNode('core.translate');
    g.connect(boxB,   'brep',  trans, 'brep');
    g.connect(offset, 'value', trans, 'offset');

    // Boolean: A subtract translated-B.   op=2 → subtraction.
    const op  = g.addNode('core.number', { value: 2 });
    const bop = g.addNode('knot.boolean');
    g.connect(boxA,  'brep',  bop, 'a');
    g.connect(trans, 'brep',  bop, 'b');
    g.connect(op,    'value', bop, 'op');

    // View sink — capture the latest mesh.
    let lastMesh: MeshData | null = null;
    const view = g.addNode('view.brep', { onMesh: (m: MeshData) => { lastMesh = m; } });
    g.connect(bop, 'brep', view, 'brep');

    // ── First run: everything evaluates ─────────────────────────
    const evaluated: string[] = [];
    const ev = new Evaluator({ onEvaluate: id => { evaluated.push(id); } });
    await ev.run(g);

    expect(lastMesh).not.toBeNull();
    expect(lastMesh!.triangleCount).toBeGreaterThan(0);
    const firstTriCount = lastMesh!.triangleCount;
    const firstPositionsHash = hashF32(lastMesh!.positions);

    // First run: every node is fresh, so all evaluate exactly once.
    const allIds = [sizeA, sizeB, offX, zero, boxA, boxB, offset, trans, op, bop, view];
    for (const id of allIds) expect(evaluated).toContain(id);

    // ── Mutate offX: only the dirty subtree should recompute ────
    evaluated.length = 0;
    g.setConstant(offX, 'value', 1.6);
    ev.markDirty(offX);
    await ev.run(g);

    // boxA, sizeA, sizeB, zero, boxB are independent of offX → must NOT re-eval.
    for (const id of [sizeA, sizeB, zero, boxA, boxB]) {
      expect(evaluated, `untouched node ${id} should not re-evaluate`).not.toContain(id);
    }
    // offX, offset, trans, bop, view are downstream → MUST re-eval.
    for (const id of [offX, offset, trans, bop, view]) {
      expect(evaluated, `downstream node ${id} should re-evaluate`).toContain(id);
    }

    // Mesh changed: triangle count may stay the same (still subtraction)
    // but the positions buffer must differ.
    expect(lastMesh!.triangleCount).toBeGreaterThan(0);
    expect(hashF32(lastMesh!.positions)).not.toBe(firstPositionsHash);
    expect(firstTriCount).toBeGreaterThan(0);

    // ── Second run with no mutation: nothing recomputes ─────────
    evaluated.length = 0;
    await ev.run(g);
    expect(evaluated).toEqual([]);

    ev.dispose();
  }, 30_000);
});

/** djb2-ish for Float32Array. Good enough to detect any change in the buffer. */
function hashF32(a: Float32Array): number {
  let h = 5381 >>> 0;
  const view = new Uint32Array(a.buffer, a.byteOffset, a.byteLength >>> 2);
  for (let i = 0; i < view.length; i++) h = (((h << 5) + h) ^ view[i]) >>> 0;
  return h;
}

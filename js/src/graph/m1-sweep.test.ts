/**
 * M1 smoke — sweep through the graph runtime.
 *
 * Builds:  Triangle profile  +  Arc rail  ->  Sweep  ->  View
 *
 * Asserts the swept solid tessellates, then mutates the arc radius and
 * confirms the dirty subtree (arc, sweep, view) re-evaluates while the
 * profile subtree (triangle, its three points) is left alone.
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

describe('M1 smoke — sweep along a curve', () => {
  it('builds a swept solid and recomputes only the dirty subtree on radius change', async () => {
    const g = new Graph(buildDefaultRegistry(knot));

    // Triangle profile — three small points in the XY plane.
    const v = (x: number, y: number, z: number) => {
      const id = g.addNode('core.vec3', {});
      g.setConstant(id, 'x', x);
      g.setConstant(id, 'y', y);
      g.setConstant(id, 'z', z);
      return id;
    };
    const a = v(0,    0,   0);
    const b = v(0.1,  0,   0);
    const c = v(0.05, 0.1, 0);
    const tri = g.addNode('knot.triangle');
    g.connect(a, 'value', tri, 'a');
    g.connect(b, 'value', tri, 'b');
    g.connect(c, 'value', tri, 'c');

    // Arc rail — quarter arc, radius driven by a Number so we can mutate it.
    const radius = g.addNode('core.number', { value: 1 });
    const startAngle = g.addNode('core.number', { value: 0 });
    const endAngle   = g.addNode('core.number', { value: Math.PI / 2 });
    const arc = g.addNode('knot.arc');
    g.connect(radius,     'value', arc, 'radius');
    g.connect(startAngle, 'value', arc, 'startAngle');
    g.connect(endAngle,   'value', arc, 'endAngle');

    // Sweep + View.
    const sweep = g.addNode('knot.sweep');
    g.connect(tri, 'brep',  sweep, 'profile');
    g.connect(arc, 'curve', sweep, 'rail');

    let mesh: MeshData | null = null;
    const view = g.addNode('view.brep', { onMesh: (m: MeshData) => { mesh = m; } });
    g.connect(sweep, 'brep', view, 'brep');

    // ── First run ───────────────────────────────────────────────
    const evaluated: string[] = [];
    const ev = new Evaluator({ onEvaluate: id => { evaluated.push(id); } });
    await ev.run(g);

    expect(mesh).not.toBeNull();
    expect(mesh!.triangleCount).toBeGreaterThan(0);
    const firstHash = hashF32(mesh!.positions);

    // Sanity: every node ran once.
    for (const id of [a, b, c, tri, radius, startAngle, endAngle, arc, sweep, view]) {
      expect(evaluated).toContain(id);
    }

    // ── Mutate radius: profile subtree must NOT re-evaluate ─────
    evaluated.length = 0;
    g.setConstant(radius, 'value', 1.5);
    ev.markDirty(radius);
    await ev.run(g);

    // Profile subtree (vec3 points + triangle) is independent of the rail.
    for (const id of [a, b, c, tri]) {
      expect(evaluated, `profile node ${id} should not re-evaluate`).not.toContain(id);
    }
    // startAngle, endAngle are independent too.
    for (const id of [startAngle, endAngle]) {
      expect(evaluated, `arc-angle ${id} should not re-evaluate`).not.toContain(id);
    }
    // Radius + arc + sweep + view all re-evaluate.
    for (const id of [radius, arc, sweep, view]) {
      expect(evaluated, `downstream ${id} should re-evaluate`).toContain(id);
    }

    expect(mesh!.triangleCount).toBeGreaterThan(0);
    expect(hashF32(mesh!.positions)).not.toBe(firstHash);

    ev.dispose();
  }, 30_000);
});

function hashF32(a: Float32Array): number {
  let h = 5381 >>> 0;
  const view = new Uint32Array(a.buffer, a.byteOffset, a.byteLength >>> 2);
  for (let i = 0; i < view.length; i++) h = (((h << 5) + h) ^ view[i]!) >>> 0;
  return h;
}

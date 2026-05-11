/**
 * Graph serialization round-trip tests.
 *
 * Pure-JS (no WASM): exercises the toJSON / fromJSON contract against
 * a small synthetic registry. The shape we care about: nodes, wires,
 * and constants survive a JSON.stringify → JSON.parse cycle and a
 * fresh Graph constructed from the parsed data is structurally
 * identical to the original.
 */
import { describe, it, expect } from 'vitest';
import { Graph, Registry } from './graph';
import { defineNode } from './nodes/define';

const Add = defineNode({
  id: 'test.add',
  inputs: { a: { kind: 'number', default: 0 }, b: { kind: 'number', default: 0 } },
  outputs: { sum: { kind: 'number' } },
  evaluate: ({ a, b }) => ({ sum: a + b }),
});

const Vec3Lit = defineNode({
  id: 'test.vec3',
  inputs: {
    x: { kind: 'number', default: 0 },
    y: { kind: 'number', default: 0 },
    z: { kind: 'number', default: 0 },
  },
  outputs: { value: { kind: 'vec3' } },
  evaluate: ({ x, y, z }) => ({ value: { x, y, z } }),
});

const buildRegistry = () => {
  const r = new Registry();
  r.register(Add);
  r.register(Vec3Lit);
  return r;
};

describe('Graph.toJSON', () => {
  it('produces a JSON-stringify-able snapshot', () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.add', { a: 1, b: 2 });
    const b = g.addNode('test.add', { a: 5 });
    g.connect(a, 'sum', b, 'b');

    const data = g.toJSON();
    expect(data.schemaVersion).toBe(1);
    expect(data.nodes).toHaveLength(2);
    expect(data.wires).toHaveLength(1);
    // Doesn't throw — must be pure JSON-safe data.
    expect(() => JSON.parse(JSON.stringify(data))).not.toThrow();
  });

  it('preserves constants including nested vec3', () => {
    const g = new Graph(buildRegistry());
    g.addNode('test.vec3', { x: 1.5, y: -2, z: 3 });
    const data = g.toJSON();
    expect(data.nodes[0]!.constants).toEqual({ x: 1.5, y: -2, z: 3 });
  });

  it('rejects non-JSON-safe constants with a useful path', () => {
    const g = new Graph(buildRegistry());
    g.addNode('test.add', { a: 1 });
    g.setConstant([...g.nodes.keys()][0]!, 'a', () => 42); // function
    expect(() => g.toJSON()).toThrow(/is a function/);
  });

  it('rejects non-plain-object constants (e.g. class instances)', () => {
    const g = new Graph(buildRegistry());
    const id = g.addNode('test.add');
    class Foo { x = 1; }
    g.setConstant(id, 'a', new Foo());
    expect(() => g.toJSON()).toThrow(/non-plain object/);
  });
});

describe('Graph.fromJSON', () => {
  it('round-trips: every node, wire, and constant survives', () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.add', { a: 1, b: 2 });
    const b = g.addNode('test.add', { a: 5 });
    g.connect(a, 'sum', b, 'b');

    const data = JSON.parse(JSON.stringify(g.toJSON()));
    const g2 = Graph.fromJSON(data, buildRegistry());

    expect([...g2.nodes.keys()].sort()).toEqual([a, b].sort());
    expect(g2.wires).toHaveLength(1);
    expect(g2.wires[0]).toEqual({ fromNode: a, fromPort: 'sum', toNode: b, toPort: 'b' });
    expect(g2.getNode(a)!.constants).toEqual({ a: 1, b: 2 });
  });

  it('continues node-id allocation past the restored next-id', () => {
    const g = new Graph(buildRegistry());
    g.addNode('test.add');
    g.addNode('test.add');
    const data = g.toJSON();
    const g2 = Graph.fromJSON(data, buildRegistry());
    const newId = g2.addNode('test.add');
    expect(newId).toBe('n3'); // n1, n2 already taken
  });

  it('rejects unknown schemaVersion', () => {
    expect(() =>
      Graph.fromJSON({ schemaVersion: 99, nextId: 1, nodes: [], wires: [] } as never, buildRegistry()),
    ).toThrow(/schemaVersion/);
  });

  it('lists missing defIds when the registry is incomplete', () => {
    const g = new Graph(buildRegistry());
    g.addNode('test.add');
    const data = g.toJSON();
    // Build a registry without `test.add`.
    const partial = new Registry();
    partial.register(Vec3Lit);
    expect(() => Graph.fromJSON(data, partial)).toThrow(/missing node defs: test.add/);
  });

  it('refuses snapshots with kind-mismatched wires', () => {
    // Hand-craft a snapshot that wires a vec3 output into a number input.
    const data = {
      schemaVersion: 1 as const,
      nextId: 3,
      nodes: [
        { id: 'n1', defId: 'test.vec3', constants: {} },
        { id: 'n2', defId: 'test.add', constants: {} },
      ],
      wires: [{ fromNode: 'n1', fromPort: 'value', toNode: 'n2', toPort: 'a' }],
    };
    expect(() => Graph.fromJSON(data, buildRegistry())).toThrow(/kind mismatch/);
  });

  it('rejects snapshots whose wires form a cycle', () => {
    const data = {
      schemaVersion: 1 as const,
      nextId: 3,
      nodes: [
        { id: 'n1', defId: 'test.add', constants: {} },
        { id: 'n2', defId: 'test.add', constants: {} },
      ],
      wires: [
        { fromNode: 'n1', fromPort: 'sum', toNode: 'n2', toPort: 'a' },
        { fromNode: 'n2', fromPort: 'sum', toNode: 'n1', toPort: 'a' },
      ],
    };
    expect(() => Graph.fromJSON(data, buildRegistry())).toThrow(/cycle/);
  });
});

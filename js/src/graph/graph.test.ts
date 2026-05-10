import { describe, it, expect } from 'vitest';
import { Graph, Registry } from './graph';
import { defineNode } from './nodes/define';

const Add = defineNode({
  id: 'test.add',
  inputs: { a: { kind: 'number', default: 0 }, b: { kind: 'number', default: 0 } },
  outputs: { sum: { kind: 'number' } },
  evaluate: ({ a, b }) => ({ sum: a + b }),
});

const Lift = defineNode({
  id: 'test.lift',
  inputs: { x: { kind: 'number', default: 0 } },
  outputs: { x: { kind: 'number' } },
  evaluate: ({ x }) => ({ x }),
});

const buildRegistry = () => {
  const r = new Registry();
  r.register(Add);
  r.register(Lift);
  return r;
};

describe('Graph', () => {
  it('adds nodes and bumps version', () => {
    const g = new Graph(buildRegistry());
    const v0 = g.version;
    g.addNode('test.add');
    expect(g.version).toBe(v0 + 1);
    expect(g.nodes.size).toBe(1);
  });

  it('rejects unknown node defs', () => {
    const g = new Graph(buildRegistry());
    expect(() => g.addNode('nope')).toThrow(/unknown node def/);
  });

  it('handles fan-out from one source into multiple ports of one target', () => {
    // Regression: a single source feeding several inputs of the same node
    // must not be misread as a cycle (in-degree double-counting).
    const g = new Graph(buildRegistry());
    const src = g.addNode('test.lift');
    const sum = g.addNode('test.add');
    g.connect(src, 'x', sum, 'a');
    g.connect(src, 'x', sum, 'b');
    const order = g.topoSort();
    expect(order.indexOf(src)).toBeLessThan(order.indexOf(sum));
  });

  it('connects compatible kinds and produces a topo order', () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.lift');
    const b = g.addNode('test.add');
    g.connect(a, 'x', b, 'a');
    const order = g.topoSort();
    expect(order.indexOf(a)).toBeLessThan(order.indexOf(b));
  });

  it('rejects connections that would create a cycle', () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.lift');
    const b = g.addNode('test.lift');
    g.connect(a, 'x', b, 'x');
    expect(() => g.connect(b, 'x', a, 'x')).toThrow(/cycle/);
  });

  it('replaces a wire when connecting to an already-wired input', () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.lift');
    const b = g.addNode('test.lift');
    const c = g.addNode('test.lift');
    g.connect(a, 'x', c, 'x');
    g.connect(b, 'x', c, 'x');
    expect(g.wires).toHaveLength(1);
    expect(g.incomingWire(c, 'x')?.fromNode).toBe(b);
  });

  it('rejects kind mismatches', () => {
    const r = new Registry();
    r.register(Add);
    r.register(defineNode({
      id: 'test.bool',
      inputs: { v: { kind: 'bool', default: false } },
      outputs: { v: { kind: 'bool' } },
      evaluate: ({ v }) => ({ v }),
    }));
    const g = new Graph(r);
    const a = g.addNode('test.add');
    const b = g.addNode('test.bool');
    expect(() => g.connect(a, 'sum', b, 'v')).toThrow(/kind mismatch/);
  });

  it('removes a node and its incident wires', () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.lift');
    const b = g.addNode('test.lift');
    g.connect(a, 'x', b, 'x');
    g.removeNode(a);
    expect(g.wires).toHaveLength(0);
    expect(g.nodes.has(a)).toBe(false);
  });
});

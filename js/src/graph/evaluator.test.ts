import { describe, it, expect } from 'vitest';
import { Graph, Registry } from './graph';
import { Evaluator } from './evaluator';
import { defineNode } from './nodes/define';
import { isError } from './types';

const Add = defineNode({
  id: 'test.add',
  inputs: { a: { kind: 'number', default: 0 }, b: { kind: 'number', default: 0 } },
  outputs: { sum: { kind: 'number' } },
  evaluate: ({ a, b }) => ({ sum: a + b }),
});

const Mul = defineNode({
  id: 'test.mul',
  inputs: { a: { kind: 'number', default: 1 }, b: { kind: 'number', default: 1 } },
  outputs: { product: { kind: 'number' } },
  evaluate: ({ a, b }) => ({ product: a * b }),
});

const Boom = defineNode({
  id: 'test.boom',
  inputs: { x: { kind: 'number', default: 0 } },
  outputs: { y: { kind: 'number' } },
  evaluate: () => { throw new Error('explode'); },
});

const Required = defineNode({
  id: 'test.required',
  inputs: { x: { kind: 'number' } },
  outputs: { x: { kind: 'number' } },
  evaluate: ({ x }) => ({ x }),
});

const RangeNode = defineNode({
  id: 'test.range',
  inputs: { count: { kind: 'number', default: 3 } },
  outputs: { list: { kind: 'list' } },
  evaluate: ({ count }) => ({ list: Array.from({ length: count }, (_, i) => i) }),
});

const buildRegistry = () => {
  const r = new Registry();
  r.register(Add);
  r.register(Mul);
  r.register(Boom);
  r.register(Required);
  r.register(RangeNode);
  return r;
};

describe('Evaluator', () => {
  it('evaluates a simple chain in topo order', async () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.add', { a: 2, b: 3 });
    const m = g.addNode('test.mul', { b: 10 });
    g.connect(a, 'sum', m, 'a');
    const ev = new Evaluator();
    await ev.run(g);
    expect(ev.getOutput(a, 'sum')).toEqual({ kind: 'number', value: 5 });
    expect(ev.getOutput(m, 'product')).toEqual({ kind: 'number', value: 50 });
  });

  it('only re-evaluates dirty subtree on the next run', async () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.add', { a: 1, b: 1 });
    const b = g.addNode('test.add', { a: 10, b: 20 });    // independent of `a`
    const m = g.addNode('test.mul');
    g.connect(a, 'sum', m, 'a');
    g.connect(b, 'sum', m, 'b');

    const evaluated: string[] = [];
    const ev = new Evaluator({ onEvaluate: id => { evaluated.push(id); } });
    await ev.run(g);
    expect(evaluated).toEqual([a, b, m]);   // first run: everything

    evaluated.length = 0;
    g.setConstant(a, 'a', 5);
    ev.markDirty(a);
    await ev.run(g);
    // `a` and `m` re-evaluate; `b` does not.
    expect(evaluated).toContain(a);
    expect(evaluated).toContain(m);
    expect(evaluated).not.toContain(b);
  });

  it('caches when nothing is dirty', async () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.add', { a: 1, b: 2 });
    const ev = new Evaluator();
    await ev.run(g);
    const evaluated: string[] = [];
    const ev2 = new Evaluator({ onEvaluate: id => { evaluated.push(id); } });
    await ev2.run(g);   // fresh evaluator: should re-evaluate once
    expect(evaluated).toEqual([a]);
    evaluated.length = 0;
    await ev2.run(g);   // second run with no dirty: noop
    expect(evaluated).toEqual([]);
  });

  it('propagates errors as poisoned outputs without throwing', async () => {
    const g = new Graph(buildRegistry());
    const boom = g.addNode('test.boom');
    const downstream = g.addNode('test.add');
    g.connect(boom, 'y', downstream, 'a');
    const ev = new Evaluator();
    await ev.run(g);
    const boomOut = ev.getOutput(boom, 'y');
    const downOut = ev.getOutput(downstream, 'sum');
    expect(boomOut && isError(boomOut)).toBe(true);
    expect(downOut && isError(downOut)).toBe(true);
    if (downOut && isError(downOut)) {
      expect(downOut.message).toMatch(/explode/);
    }
  });

  it('errors when a required input is unwired', async () => {
    const g = new Graph(buildRegistry());
    const r = g.addNode('test.required');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(r, 'x');
    expect(out && isError(out)).toBe(true);
  });

  it('drops cache for nodes removed from the graph', async () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('test.add', { a: 1, b: 1 });
    const ev = new Evaluator();
    await ev.run(g);
    expect(ev.getOutput(a, 'sum')).toBeDefined();
    g.removeNode(a);
    await ev.run(g);
    expect(ev.getOutput(a, 'sum')).toBeUndefined();
  });

  it('auto-iterates when a list feeds into a scalar input', async () => {
    const g = new Graph(buildRegistry());
    // Range → [0, 1, 2], feed into Add (a=list, b=10)
    const range = g.addNode('test.range', { count: 3 });
    const add = g.addNode('test.add', { b: 10 });
    g.connect(range, 'list', add, 'a');
    const ev = new Evaluator();
    await ev.run(g);

    const out = ev.getOutput(add, 'sum');
    expect(out).toBeDefined();
    expect(out!.kind).toBe('list');
    expect(out!.value).toEqual([10, 11, 12]);
  });

  it('auto-iterates with longest-list matching', async () => {
    const g = new Graph(buildRegistry());
    const r3 = g.addNode('test.range', { count: 3 }); // [0, 1, 2]
    const r2 = g.addNode('test.range', { count: 2 }); // [0, 1]
    const add = g.addNode('test.add');
    g.connect(r3, 'list', add, 'a');
    g.connect(r2, 'list', add, 'b');
    const ev = new Evaluator();
    await ev.run(g);

    const out = ev.getOutput(add, 'sum');
    expect(out!.kind).toBe('list');
    // longest list = 3; shorter wraps: [0,1,0]
    expect(out!.value).toEqual([0, 2, 2]);
  });

  it('cascades auto-iteration through downstream nodes', async () => {
    const g = new Graph(buildRegistry());
    const range = g.addNode('test.range', { count: 3 }); // [0, 1, 2]
    const add = g.addNode('test.add', { b: 1 });     // auto-iterate: [1, 2, 3]
    const mul = g.addNode('test.mul', { b: 10 });     // auto-iterate: [10, 20, 30]
    g.connect(range, 'list', add, 'a');
    g.connect(add, 'sum', mul, 'a');
    const ev = new Evaluator();
    await ev.run(g);

    const out = ev.getOutput(mul, 'product');
    expect(out!.kind).toBe('list');
    expect(out!.value).toEqual([10, 20, 30]);
  });
});

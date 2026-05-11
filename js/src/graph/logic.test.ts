/**
 * Logic node smoke tests — drive each comparison, boolean operator,
 * and conditional select through the evaluator to confirm they
 * register, evaluate, and wire correctly.
 */
import { describe, it, expect } from 'vitest';
import { Graph, Registry } from './graph';
import { Evaluator } from './evaluator';
import type { InputMap, OutputMap, NodeDef } from './types';
import {
  EqualsNode, NotEqualsNode, GreaterNode, LessNode,
  GreaterEqualNode, LessEqualNode,
  AndNode, OrNode, NotNode, XorNode,
  IfNumberNode, IfBoolNode,
} from './nodes/logic';

const registry = () => {
  const r = new Registry();
  const defs: ReadonlyArray<NodeDef<InputMap, OutputMap>> = [
    EqualsNode, NotEqualsNode, GreaterNode, LessNode,
    GreaterEqualNode, LessEqualNode,
    AndNode, OrNode, NotNode, XorNode,
    IfNumberNode, IfBoolNode,
  ];
  for (const n of defs) r.register(n);
  return r;
};

async function evalBool(nodeDefId: string, constants: Record<string, unknown>): Promise<boolean> {
  const g = new Graph(registry());
  const n = g.addNode(nodeDefId, constants);
  const ev = new Evaluator();
  await ev.run(g);
  const out = ev.getOutput(n, 'result');
  if (out?.kind !== 'bool') throw new Error(`expected bool, got ${out?.kind}`);
  return out.value;
}

async function evalNumber(nodeDefId: string, constants: Record<string, unknown>): Promise<number> {
  const g = new Graph(registry());
  const n = g.addNode(nodeDefId, constants);
  const ev = new Evaluator();
  await ev.run(g);
  const out = ev.getOutput(n, 'result');
  if (out?.kind !== 'number') throw new Error(`expected number, got ${out?.kind}`);
  return out.value;
}

describe('Comparison nodes', () => {
  it('equals: numeric equality', async () => {
    expect(await evalBool('logic.equals', { a: 1, b: 1 })).toBe(true);
    expect(await evalBool('logic.equals', { a: 1, b: 2 })).toBe(false);
  });

  it('notEquals: complement of equals', async () => {
    expect(await evalBool('logic.notEquals', { a: 1, b: 2 })).toBe(true);
    expect(await evalBool('logic.notEquals', { a: 1, b: 1 })).toBe(false);
  });

  it('greater / less / greaterEqual / lessEqual: strict and non-strict ordering', async () => {
    expect(await evalBool('logic.greater', { a: 2, b: 1 })).toBe(true);
    expect(await evalBool('logic.greater', { a: 1, b: 1 })).toBe(false);
    expect(await evalBool('logic.less', { a: 1, b: 2 })).toBe(true);
    expect(await evalBool('logic.less', { a: 2, b: 2 })).toBe(false);
    expect(await evalBool('logic.greaterEqual', { a: 1, b: 1 })).toBe(true);
    expect(await evalBool('logic.lessEqual', { a: 1, b: 1 })).toBe(true);
  });
});

describe('Boolean logic nodes', () => {
  it('and: short-circuits on first false', async () => {
    expect(await evalBool('logic.and', { a: true, b: true })).toBe(true);
    expect(await evalBool('logic.and', { a: true, b: false })).toBe(false);
    expect(await evalBool('logic.and', { a: false, b: true })).toBe(false);
  });

  it('or: short-circuits on first true', async () => {
    expect(await evalBool('logic.or', { a: false, b: false })).toBe(false);
    expect(await evalBool('logic.or', { a: false, b: true })).toBe(true);
    expect(await evalBool('logic.or', { a: true, b: false })).toBe(true);
  });

  it('not: negates input', async () => {
    expect(await evalBool('logic.not', { value: true })).toBe(false);
    expect(await evalBool('logic.not', { value: false })).toBe(true);
  });

  it('xor: true when inputs differ', async () => {
    expect(await evalBool('logic.xor', { a: true, b: false })).toBe(true);
    expect(await evalBool('logic.xor', { a: false, b: true })).toBe(true);
    expect(await evalBool('logic.xor', { a: true, b: true })).toBe(false);
    expect(await evalBool('logic.xor', { a: false, b: false })).toBe(false);
  });
});

describe('Conditional select', () => {
  it('ifNumber: returns whenTrue branch when condition is true', async () => {
    expect(await evalNumber('logic.ifNumber', {
      condition: true, whenTrue: 42, whenFalse: -1,
    })).toBe(42);
    expect(await evalNumber('logic.ifNumber', {
      condition: false, whenTrue: 42, whenFalse: -1,
    })).toBe(-1);
  });

  it('ifBool: cascades booleans', async () => {
    expect(await evalBool('logic.ifBool', {
      condition: true, whenTrue: true, whenFalse: false,
    })).toBe(true);
    expect(await evalBool('logic.ifBool', {
      condition: false, whenTrue: true, whenFalse: false,
    })).toBe(false);
  });
});

describe('End-to-end: comparison → if', () => {
  it('clamp-like pattern: x > limit ? limit : x', async () => {
    // Build: input=5, limit=3, greater(input, limit) → if(condition, limit, input)
    const g = new Graph(registry());
    const gt = g.addNode('logic.greater', { a: 5, b: 3 });
    const sel = g.addNode('logic.ifNumber', { whenTrue: 3, whenFalse: 5 });
    g.connect(gt, 'result', sel, 'condition');
    const ev = new Evaluator();
    await ev.run(g);
    const out = ev.getOutput(sel, 'result');
    expect(out).toEqual({ kind: 'number', value: 3 });
  });
});

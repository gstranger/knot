/**
 * Form-mode extraction tests. Pure-JS, no WASM — we register only
 * the input-primitive nodes the form helper recognizes plus an
 * arbitrary downstream node to test the "skip wired nodes" rule.
 */
import { describe, it, expect } from 'vitest';
import { Graph, Registry } from './graph';
import { extractFormFields, setFormValue } from './form';
import { NumberNode } from './nodes/scalar';
import { Vec3Node } from './nodes/vec3';
import { SliderNode, ToggleNode, AddNode } from './nodes/math';

const buildRegistry = () => {
  const r = new Registry();
  r.register(NumberNode);
  r.register(Vec3Node);
  r.register(SliderNode);
  r.register(ToggleNode);
  r.register(AddNode);
  return r;
};

describe('extractFormFields', () => {
  it('returns an empty list for an empty graph', () => {
    const g = new Graph(buildRegistry());
    expect(extractFormFields(g)).toEqual([]);
  });

  it('surfaces NumberNode as a number field with its current value', () => {
    const g = new Graph(buildRegistry());
    const id = g.addNode('core.number', { value: 3.14 });
    const fields = extractFormFields(g);
    expect(fields).toHaveLength(1);
    expect(fields[0]).toMatchObject({
      kind: 'number',
      nodeId: id,
      defId: 'core.number',
      value: 3.14,
    });
  });

  it('surfaces Slider with min/max/step hints', () => {
    const g = new Graph(buildRegistry());
    g.addNode('core.slider', { value: 0.5, min: 0, max: 1, step: 0.05 });
    const fields = extractFormFields(g);
    expect(fields[0]).toMatchObject({
      kind: 'number',
      value: 0.5,
      min: 0,
      max: 1,
      step: 0.05,
    });
  });

  it('surfaces Toggle as a bool field', () => {
    const g = new Graph(buildRegistry());
    g.addNode('core.toggle', { value: true });
    expect(extractFormFields(g)[0]).toMatchObject({ kind: 'bool', value: true });
  });

  it('surfaces Vec3 as a vec3 field reconstructed from x/y/z constants', () => {
    const g = new Graph(buildRegistry());
    g.addNode('core.vec3', { x: 1, y: 2, z: 3 });
    const f = extractFormFields(g)[0]!;
    expect(f.kind).toBe('vec3');
    expect((f as { value: { x: number; y: number; z: number } }).value).toEqual({ x: 1, y: 2, z: 3 });
  });

  it('skips intermediate nodes that have incoming wires', () => {
    // NumberNode(.value) → AddNode(.a): the Number node still has
    // no incoming wires, so it shows up. AddNode has incoming wires
    // and isn't an input-primitive anyway, so it shouldn't appear.
    const g = new Graph(buildRegistry());
    const num = g.addNode('core.number', { value: 5 });
    const add = g.addNode('math.add', { b: 1 });
    g.connect(num, 'value', add, 'a');
    const fields = extractFormFields(g);
    expect(fields).toHaveLength(1);
    expect(fields[0]!.nodeId).toBe(num);
  });

  it('skips an input-primitive whose value port is wired (rare but valid)', () => {
    // If someone wires another node into Number.value, the Number
    // is no longer a pure parameter source — treat it as
    // intermediate and don't expose it.
    const g = new Graph(buildRegistry());
    const src = g.addNode('core.number', { value: 7 });
    const proxy = g.addNode('core.number', { value: 0 });
    g.connect(src, 'value', proxy, 'value');
    const fields = extractFormFields(g);
    expect(fields).toHaveLength(1);
    expect(fields[0]!.nodeId).toBe(src);
  });

  it('preserves graph insertion order', () => {
    const g = new Graph(buildRegistry());
    const a = g.addNode('core.number', { value: 1 });
    const b = g.addNode('core.toggle', { value: false });
    const c = g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    const order = extractFormFields(g).map((f) => f.nodeId);
    expect(order).toEqual([a, b, c]);
  });
});

describe('setFormValue', () => {
  it('updates Number/Slider value constant', () => {
    const g = new Graph(buildRegistry());
    g.addNode('core.number', { value: 1 });
    const f = extractFormFields(g)[0]!;
    setFormValue(g, f, 42);
    expect(extractFormFields(g)[0]!.value).toBe(42);
  });

  it('updates Toggle value constant', () => {
    const g = new Graph(buildRegistry());
    g.addNode('core.toggle', { value: false });
    const f = extractFormFields(g)[0]!;
    setFormValue(g, f, true);
    expect(extractFormFields(g)[0]!.value).toBe(true);
  });

  it('updates each component of a Vec3', () => {
    const g = new Graph(buildRegistry());
    g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    const f = extractFormFields(g)[0]!;
    setFormValue(g, f, { x: 10, y: 20, z: 30 });
    expect((extractFormFields(g)[0]! as { value: object }).value).toEqual({ x: 10, y: 20, z: 30 });
  });

  it('round-trips through Graph.toJSON', () => {
    // Mutating via the form helper must show up in the serialized
    // graph just like Graph.setConstant — proves form mode is just
    // a thin lens over the graph state.
    const g = new Graph(buildRegistry());
    g.addNode('core.vec3', { x: 0, y: 0, z: 0 });
    setFormValue(g, extractFormFields(g)[0]!, { x: 1, y: 2, z: 3 });
    const data = g.toJSON();
    expect(data.nodes[0]!.constants).toEqual({ x: 1, y: 2, z: 3 });
  });
});

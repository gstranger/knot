import { defineNode } from './define';

/** Number slider with min/max/step metadata in constants. */
export const SliderNode = defineNode({
  id: 'core.slider',
  label: 'Slider',
  inputs: {
    value: { kind: 'number', default: 0.5 },
  },
  outputs: { value: { kind: 'number' } },
  evaluate: ({ value }) => ({ value }),
});

/** Boolean toggle. */
export const ToggleNode = defineNode({
  id: 'core.toggle',
  label: 'Toggle',
  inputs: { value: { kind: 'bool', default: false } },
  outputs: { value: { kind: 'bool' } },
  evaluate: ({ value }) => ({ value }),
});

export const AddNode = defineNode({
  id: 'math.add',
  label: 'Add',
  inputs: {
    a: { kind: 'number', default: 0 },
    b: { kind: 'number', default: 0 },
  },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ a, b }) => ({ result: a + b }),
});

export const SubtractNode = defineNode({
  id: 'math.subtract',
  label: 'Subtract',
  inputs: {
    a: { kind: 'number', default: 0 },
    b: { kind: 'number', default: 0 },
  },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ a, b }) => ({ result: a - b }),
});

export const MultiplyNode = defineNode({
  id: 'math.multiply',
  label: 'Multiply',
  inputs: {
    a: { kind: 'number', default: 0 },
    b: { kind: 'number', default: 1 },
  },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ a, b }) => ({ result: a * b }),
});

export const DivideNode = defineNode({
  id: 'math.divide',
  label: 'Divide',
  inputs: {
    a: { kind: 'number', default: 0 },
    b: { kind: 'number', default: 1 },
  },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ a, b }) => {
    if (b === 0) throw new Error('division by zero');
    return { result: a / b };
  },
});

export const NegateNode = defineNode({
  id: 'math.negate',
  label: 'Negate',
  inputs: { value: { kind: 'number', default: 0 } },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ value }) => ({ result: -value }),
});

export const AbsNode = defineNode({
  id: 'math.abs',
  label: 'Abs',
  inputs: { value: { kind: 'number', default: 0 } },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ value }) => ({ result: Math.abs(value) }),
});

export const SinNode = defineNode({
  id: 'math.sin',
  label: 'Sin',
  inputs: { angle: { kind: 'number', default: 0 } },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ angle }) => ({ result: Math.sin(angle) }),
});

export const CosNode = defineNode({
  id: 'math.cos',
  label: 'Cos',
  inputs: { angle: { kind: 'number', default: 0 } },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ angle }) => ({ result: Math.cos(angle) }),
});

export const RemapNode = defineNode({
  id: 'math.remap',
  label: 'Remap',
  inputs: {
    value: { kind: 'number', default: 0.5 },
    fromMin: { kind: 'number', default: 0 },
    fromMax: { kind: 'number', default: 1 },
    toMin: { kind: 'number', default: 0 },
    toMax: { kind: 'number', default: 10 },
  },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ value, fromMin, fromMax, toMin, toMax }) => {
    const t = (value - fromMin) / (fromMax - fromMin || 1);
    return { result: toMin + t * (toMax - toMin) };
  },
});

/**
 * Expression node: evaluates a math expression string from the `expr`
 * constant using inputs a, b, c as variables.
 *
 * Example expressions: `a * sin(b)`, `sqrt(a*a + b*b)`, `a > 0 ? b : c`
 *
 * The expression is a JS expression with Math.* available as globals.
 */
export const ExpressionNode = defineNode({
  id: 'math.expression',
  label: 'Expression',
  inputs: {
    a: { kind: 'number', default: 0 },
    b: { kind: 'number', default: 0 },
    c: { kind: 'number', default: 0 },
  },
  outputs: { result: { kind: 'number' } },
  evaluate: ({ a, b, c }, ctx) => {
    const expr = ((ctx as any).constants?.expr as string) ?? 'a';
    // Build a function with Math globals exposed as bare names.
    const fn = new Function(
      'a', 'b', 'c',
      'sin', 'cos', 'tan', 'sqrt', 'abs', 'pow', 'min', 'max',
      'floor', 'ceil', 'round', 'PI', 'E',
      `return (${expr});`,
    );
    const result = fn(
      a, b, c,
      Math.sin, Math.cos, Math.tan, Math.sqrt, Math.abs, Math.pow,
      Math.min, Math.max, Math.floor, Math.ceil, Math.round,
      Math.PI, Math.E,
    );
    if (typeof result !== 'number' || !isFinite(result)) {
      throw new Error(`expression "${expr}" did not produce a finite number`);
    }
    return { result };
  },
});

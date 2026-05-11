import { defineNode } from './define';

/** Generate a list of numbers from start to end (exclusive) with a step. */
export const RangeNode = defineNode({
  id: 'list.range',
  label: 'Range',
  inputs: {
    start: { kind: 'number' as const, default: 0 },
    end:   { kind: 'number' as const, default: 10 },
    step:  { kind: 'number' as const, default: 1 },
  },
  outputs: {
    list: { kind: 'list' as const },
  },
  evaluate({ start, end, step }) {
    if (step === 0) throw new Error('Range: step cannot be 0');
    const result: number[] = [];
    if (step > 0) {
      for (let v = start; v < end; v += step) result.push(v);
    } else {
      for (let v = start; v > end; v += step) result.push(v);
    }
    if (result.length > 10_000) throw new Error('Range: exceeded 10,000 element limit');
    return { list: result };
  },
});

/** Generate a list of N numbers: start, start+step, start+2*step, ... */
export const SeriesNode = defineNode({
  id: 'list.series',
  label: 'Series',
  inputs: {
    start: { kind: 'number' as const, default: 0 },
    step:  { kind: 'number' as const, default: 1 },
    count: { kind: 'number' as const, default: 10 },
  },
  outputs: {
    list: { kind: 'list' as const },
  },
  evaluate({ start, step, count }) {
    const n = Math.max(0, Math.round(count));
    if (n > 10_000) throw new Error('Series: exceeded 10,000 element limit');
    const result: number[] = [];
    for (let i = 0; i < n; i++) result.push(start + i * step);
    return { list: result };
  },
});

/** Get the i-th element from a list (outputs as a number). */
export const ListItemNode = defineNode({
  id: 'list.item',
  label: 'List Item',
  inputs: {
    list:  { kind: 'list' as const, default: [] },
    index: { kind: 'number' as const, default: 0 },
  },
  outputs: {
    value: { kind: 'number' as const },
  },
  evaluate({ list, index }) {
    const i = Math.round(index);
    const arr = list as unknown[];
    if (arr.length === 0) throw new Error('List Item: empty list');
    const clamped = Math.max(0, Math.min(i, arr.length - 1));
    return { value: arr[clamped] as number };
  },
});

/** Return the number of items in a list. */
export const ListLengthNode = defineNode({
  id: 'list.length',
  label: 'List Length',
  inputs: {
    list: { kind: 'list' as const, default: [] },
  },
  outputs: {
    count: { kind: 'number' as const },
  },
  evaluate({ list }) {
    return { count: (list as unknown[]).length };
  },
});

/** Repeat a number N times to produce a list. */
export const RepeatNode = defineNode({
  id: 'list.repeat',
  label: 'Repeat',
  inputs: {
    value: { kind: 'number' as const, default: 0 },
    count: { kind: 'number' as const, default: 5 },
  },
  outputs: {
    list: { kind: 'list' as const },
  },
  evaluate({ value, count }) {
    const n = Math.max(0, Math.round(count));
    if (n > 10_000) throw new Error('Repeat: exceeded 10,000 element limit');
    return { list: Array(n).fill(value) };
  },
});

/** Flatten nested lists one level deep. */
export const FlattenNode = defineNode({
  id: 'list.flatten',
  label: 'Flatten',
  inputs: {
    list: { kind: 'list' as const, default: [] },
  },
  outputs: {
    list: { kind: 'list' as const },
  },
  evaluate({ list }) {
    const arr = list as unknown[];
    const result: unknown[] = [];
    for (const item of arr) {
      if (Array.isArray(item)) result.push(...item);
      else result.push(item);
    }
    return { list: result };
  },
});

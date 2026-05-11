/**
 * Form mode: surface a graph's input-primitive nodes (Number, Slider,
 * Toggle, Vec3) as named form parameters. This lets the editor present
 * a "definition" UI to non-graph users — they tune the inputs, the
 * graph re-evaluates, the viewport updates, but the node canvas is
 * hidden.
 *
 * What counts as a form field today:
 *
 *   - `core.number` / `core.slider`           → number control
 *   - `core.toggle`                            → checkbox
 *   - `core.vec3`                              → 3-component number control
 *
 * Always: the node must have NO incoming wires. Nodes with upstream
 * inputs are intermediate computations, not parameter sources, and
 * surfacing them would let the user override a computed value in
 * confusing ways.
 *
 * Later: we can extend with an explicit per-node "expose this" tag,
 * or with custom labels/min/max metadata. Today the auto-detect rule
 * covers the canonical Grasshopper-style "definition" workflow.
 */
import type { Graph } from './graph';
import type { Vec3 } from '../math/vec3';

/** One control's worth of state to render in the form UI. */
export type FormField =
  | NumberFormField
  | BoolFormField
  | Vec3FormField;

interface FormFieldCommon {
  readonly nodeId: string;
  readonly defId: string;
  /** Human-facing label — defaults to the node's def label. */
  readonly label: string;
}

export interface NumberFormField extends FormFieldCommon {
  readonly kind: 'number';
  readonly value: number;
  /** Hints for slider rendering; absent on bare Number nodes. */
  readonly min?: number;
  readonly max?: number;
  readonly step?: number;
}

export interface BoolFormField extends FormFieldCommon {
  readonly kind: 'bool';
  readonly value: boolean;
}

export interface Vec3FormField extends FormFieldCommon {
  readonly kind: 'vec3';
  readonly value: Vec3;
}

const SUPPORTED: Record<string, 'number' | 'bool' | 'vec3'> = {
  'core.number': 'number',
  'core.slider': 'number',
  'core.toggle': 'bool',
  'core.vec3': 'vec3',
};

/**
 * Return the form fields exposed by `graph`. Order is stable: nodes
 * are visited in insertion order (`graph.nodes` is a Map and Maps
 * preserve insertion order), so the UI's field order matches the
 * order nodes were added to the graph.
 */
export function extractFormFields(graph: Graph): FormField[] {
  const out: FormField[] = [];
  for (const [nodeId, inst] of graph.nodes) {
    const kind = SUPPORTED[inst.defId];
    if (!kind) continue;
    if (nodeHasIncomingWires(graph, nodeId)) continue;
    const def = graph.getDef(nodeId);
    const label = def.label ?? inst.defId;
    out.push(buildField(kind, nodeId, inst.defId, label, inst.constants));
  }
  return out;
}

function nodeHasIncomingWires(graph: Graph, nodeId: string): boolean {
  for (const w of graph.wires) {
    if (w.toNode === nodeId) return true;
  }
  return false;
}

function buildField(
  kind: 'number' | 'bool' | 'vec3',
  nodeId: string,
  defId: string,
  label: string,
  constants: Readonly<Record<string, unknown>>,
): FormField {
  switch (kind) {
    case 'number': {
      const value = typeof constants.value === 'number' ? constants.value : 0;
      // Slider hints (when the form designer pre-set them as constants).
      const min = typeof constants.min === 'number' ? constants.min : undefined;
      const max = typeof constants.max === 'number' ? constants.max : undefined;
      const step = typeof constants.step === 'number' ? constants.step : undefined;
      return { kind: 'number', nodeId, defId, label, value, min, max, step };
    }
    case 'bool': {
      const value = constants.value === true;
      return { kind: 'bool', nodeId, defId, label, value };
    }
    case 'vec3': {
      const c = constants as { x?: unknown; y?: unknown; z?: unknown };
      const value: Vec3 = {
        x: typeof c.x === 'number' ? c.x : 0,
        y: typeof c.y === 'number' ? c.y : 0,
        z: typeof c.z === 'number' ? c.z : 0,
      };
      return { kind: 'vec3', nodeId, defId, label, value };
    }
  }
}

/**
 * Write a new value for `field` back into `graph`. Mirrors
 * `Graph.setConstant` but takes a typed field shape so callers don't
 * have to know whether a vec3 lives as `{ x, y, z }` constants or a
 * single `value`.
 */
export function setFormValue(
  graph: Graph,
  field: FormField,
  value: FormField['value'],
): void {
  switch (field.kind) {
    case 'number':
      graph.setConstant(field.nodeId, 'value', value as number);
      return;
    case 'bool':
      graph.setConstant(field.nodeId, 'value', value as boolean);
      return;
    case 'vec3': {
      const v = value as Vec3;
      graph.setConstant(field.nodeId, 'x', v.x);
      graph.setConstant(field.nodeId, 'y', v.y);
      graph.setConstant(field.nodeId, 'z', v.z);
      return;
    }
  }
}

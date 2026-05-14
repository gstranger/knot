import type { GraphData } from 'knot-cad/graph';

export interface ExampleFile {
  graph: GraphData;
  layout: { positions: Record<string, { x: number; y: number }>; nextPos: { x: number; y: number } };
}

export interface ExampleEntry {
  id: string;
  label: string;
  description: string;
  file: ExampleFile;
}

// ── Box minus Cylinder ───────────────────────────────────────────
// The CSG "hello world": a unit-ish box with a cylindrical hole
// drilled through it. Boolean op constant = 2 (subtraction).
const BOX_MINUS_CYLINDER: ExampleFile = {
  graph: {
    schemaVersion: 1,
    nextId: 4,
    nodes: [
      { id: 'n1', defId: 'knot.box',      constants: { sx: 2, sy: 2, sz: 2 } },
      { id: 'n2', defId: 'knot.cylinder', constants: { center: { x: 0, y: 0, z: 0 }, radius: 0.6, height: 3 } },
      { id: 'n3', defId: 'knot.boolean',  constants: { op: 2 } },
    ],
    wires: [
      { fromNode: 'n1', fromPort: 'brep', toNode: 'n3', toPort: 'a' },
      { fromNode: 'n2', fromPort: 'brep', toNode: 'n3', toPort: 'b' },
    ],
  },
  layout: {
    positions: { n1: { x: 40, y: 40 }, n2: { x: 40, y: 220 }, n3: { x: 320, y: 130 } },
    nextPos: { x: 360, y: 320 },
  },
};

// ── Two-sphere union ─────────────────────────────────────────────
// Smallest possible boolean: two offset spheres unioned.
const TWO_SPHERE_UNION: ExampleFile = {
  graph: {
    schemaVersion: 1,
    nextId: 4,
    nodes: [
      { id: 'n1', defId: 'knot.sphere',  constants: { center: { x: 0,   y: 0, z: 0 }, radius: 1 } },
      { id: 'n2', defId: 'knot.sphere',  constants: { center: { x: 1.2, y: 0, z: 0 }, radius: 1 } },
      { id: 'n3', defId: 'knot.boolean', constants: { op: 0 } },
    ],
    wires: [
      { fromNode: 'n1', fromPort: 'brep', toNode: 'n3', toPort: 'a' },
      { fromNode: 'n2', fromPort: 'brep', toNode: 'n3', toPort: 'b' },
    ],
  },
  layout: {
    positions: { n1: { x: 40, y: 40 }, n2: { x: 40, y: 220 }, n3: { x: 320, y: 130 } },
    nextPos: { x: 360, y: 320 },
  },
};

// ── Row of translated boxes (auto-iteration showcase) ────────────
// `list.series` produces a list of x-positions. Feeding that into
// Vec3.x auto-iterates the Vec3 node, producing a list of vec3s.
// Translate then auto-iterates with one box → many translated breps.
const SERIES_OF_BOXES: ExampleFile = {
  graph: {
    schemaVersion: 1,
    nextId: 5,
    nodes: [
      { id: 'n1', defId: 'list.series',    constants: { start: -3, step: 1.5, count: 5 } },
      { id: 'n2', defId: 'core.vec3',      constants: { x: 0, y: 0, z: 0 } },
      { id: 'n3', defId: 'knot.box',       constants: { sx: 1, sy: 1, sz: 1 } },
      { id: 'n4', defId: 'core.translate', constants: { offset: { x: 0, y: 0, z: 0 } } },
    ],
    wires: [
      { fromNode: 'n1', fromPort: 'list',   toNode: 'n2', toPort: 'x' },
      { fromNode: 'n2', fromPort: 'vec3',   toNode: 'n4', toPort: 'offset' },
      { fromNode: 'n3', fromPort: 'brep',   toNode: 'n4', toPort: 'brep' },
    ],
  },
  layout: {
    positions: {
      n1: { x: 40,  y: 40  },
      n2: { x: 280, y: 40  },
      n3: { x: 40,  y: 240 },
      n4: { x: 520, y: 140 },
    },
    nextPos: { x: 560, y: 340 },
  },
};

export const EXAMPLES: ExampleEntry[] = [
  { id: 'box-minus-cyl',  label: 'Box ∖ Cylinder',      description: 'Drill a hole through a box (boolean subtraction).', file: BOX_MINUS_CYLINDER },
  { id: 'sphere-union',   label: 'Two-Sphere Union',    description: 'Union two offset spheres.',                        file: TWO_SPHERE_UNION   },
  { id: 'box-series',     label: 'Row of Boxes',        description: 'List auto-iteration: one box → five boxes.',        file: SERIES_OF_BOXES    },
];

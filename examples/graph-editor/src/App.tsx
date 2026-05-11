import { useCallback, useEffect, useRef, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  useNodesState,
  useEdgesState,
  addEdge,
  type Node,
  type Edge,
  type Connection,
  type NodeTypes,
  type OnNodesDelete,
  type OnEdgesDelete,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { Canvas } from '@react-three/fiber';
import { OrbitControls } from '@react-three/drei';
import { Joyride, type Step, type EventData, STATUS } from 'react-joyride';

import { createKnot, type Knot, type MeshData } from 'knot-cad';
import { Graph, Evaluator, buildDefaultRegistry } from 'knot-cad/graph';
import type { NodeRegistry } from 'knot-cad/graph';

import { CadNode, type CadNodeData } from './CadNode';

// ── Tour ────────────────────────────────────────────────────────
const TOUR_STEPS: Step[] = [
  {
    target: 'body',
    placement: 'center',
    title: 'Welcome to the Knot Graph Editor',
    content: 'This is a node-based parametric CAD tool. You build 3D geometry by connecting nodes in a visual graph \u2014 similar to Grasshopper or Geometry Nodes.',
    skipBeacon: true,
  },
  {
    target: '[data-tour="palette"]',
    placement: 'right',
    title: 'Node Palette',
    content: 'This sidebar lists all available nodes, organized by category. Click any button to add that node to the graph canvas. Click a category header to expand or collapse it.',
  },
  {
    target: '[data-tour="palette-primitives"]',
    placement: 'right',
    title: 'Primitives',
    content: 'Start here. Box, Sphere, and Cylinder create solid 3D shapes that appear in the viewport.',
  },
  {
    target: '[data-tour="palette-operations"]',
    placement: 'right',
    title: 'Operations',
    content: 'Boolean (union / intersection / subtraction), Extrude, Revolve, Sweep, and Loft let you combine and transform shapes into complex geometry.',
  },
  {
    target: '[data-tour="graph-canvas"]',
    placement: 'left',
    title: 'Graph Canvas',
    content: 'Nodes you add appear here. Each node has colored input ports on the left and output ports on the right. Drag from an output to an input to connect them. The graph re-evaluates automatically.',
  },
  {
    target: '[data-tour="graph-canvas"]',
    placement: 'left',
    title: 'Editing Tips',
    content: 'Pan: drag the background. Zoom: scroll wheel. Select a node and press Backspace to delete it. Number inputs on nodes can be edited directly \u2014 just click and type.',
  },
  {
    target: '[data-tour="viewport"]',
    placement: 'left',
    title: '3D Viewport',
    content: 'The result of your graph renders here in real time. Drag to orbit, scroll to zoom. Every node that produces a solid shows up as blue geometry.',
  },
  {
    target: 'body',
    placement: 'center',
    title: 'Try It: Subtract a Cylinder from a Box',
    content: '1. Add a Box (Primitives \u203A Box)\n2. Add a Cylinder\n3. Add a Boolean (Operations \u203A Boolean)\n4. Connect the Box brep output \u2192 Boolean input "a"\n5. Connect the Cylinder brep output \u2192 Boolean input "b"\n6. Set Boolean "op" to 2 (subtraction)\n\nThe cylinder is cut from the box in the viewport!',
  },
];

// ── Types ────────────────────────────────────────────────────────
const nodeTypes: NodeTypes = { cad: CadNode as any };

interface PaletteEntry { defId: string; label: string }

interface PaletteGroup { label: string; entries: PaletteEntry[] }

const PALETTE: PaletteGroup[] = [
  { label: 'Input', entries: [
    { defId: 'core.number', label: 'Number' },
    { defId: 'core.slider', label: 'Slider' },
    { defId: 'core.toggle', label: 'Toggle' },
    { defId: 'core.vec3', label: 'Vec3' },
  ]},
  { label: 'Math', entries: [
    { defId: 'math.add', label: 'Add' },
    { defId: 'math.subtract', label: 'Subtract' },
    { defId: 'math.multiply', label: 'Multiply' },
    { defId: 'math.divide', label: 'Divide' },
    { defId: 'math.negate', label: 'Negate' },
    { defId: 'math.sin', label: 'Sin' },
    { defId: 'math.cos', label: 'Cos' },
    { defId: 'math.remap', label: 'Remap' },
    { defId: 'math.expression', label: 'Expression' },
  ]},
  { label: 'Vector', entries: [
    { defId: 'core.deconstructVec3', label: 'Deconstruct' },
    { defId: 'vec3.add', label: 'Add Vec3' },
    { defId: 'vec3.scale', label: 'Scale Vec3' },
    { defId: 'vec3.cross', label: 'Cross' },
    { defId: 'vec3.dot', label: 'Dot' },
    { defId: 'vec3.length', label: 'Length' },
  ]},
  { label: 'Primitives', entries: [
    { defId: 'knot.box', label: 'Box' },
    { defId: 'knot.sphere', label: 'Sphere' },
    { defId: 'knot.cylinder', label: 'Cylinder' },
    { defId: 'knot.triangle', label: 'Triangle' },
  ]},
  { label: 'Transform', entries: [
    { defId: 'core.translate', label: 'Translate' },
    { defId: 'core.rotate', label: 'Rotate' },
    { defId: 'core.scale', label: 'Scale' },
  ]},
  { label: 'Operations', entries: [
    { defId: 'knot.boolean', label: 'Boolean' },
    { defId: 'knot.extrude', label: 'Extrude' },
    { defId: 'knot.revolve', label: 'Revolve' },
    { defId: 'knot.sweep', label: 'Sweep' },
    { defId: 'knot.loft2', label: 'Loft (2)' },
  ]},
  { label: 'Curve', entries: [
    { defId: 'knot.line', label: 'Line' },
    { defId: 'knot.arc', label: 'Arc' },
    { defId: 'core.curve.pointAt', label: 'Point At' },
    { defId: 'core.curve.divide', label: 'Divide' },
    { defId: 'core.curve.offset', label: 'Offset' },
  ]},
  { label: 'List', entries: [
    { defId: 'list.range', label: 'Range' },
    { defId: 'list.series', label: 'Series' },
    { defId: 'list.item', label: 'List Item' },
    { defId: 'list.length', label: 'List Length' },
    { defId: 'list.repeat', label: 'Repeat' },
    { defId: 'list.flatten', label: 'Flatten' },
  ]},
];

// ── App ──────────────────────────────────────────────────────────
export function App() {
  const [loading, setLoading] = useState(true);
  const [meshes, setMeshes] = useState<MeshData[]>([]);

  const registryRef = useRef<NodeRegistry | null>(null);
  const graphRef = useRef<Graph | null>(null);
  const evalRef = useRef<Evaluator | null>(null);

  const [rfNodes, setRfNodes, onNodesChange] = useNodesState([] as Node[]);
  const [rfEdges, setRfEdges, onEdgesChange] = useEdgesState([] as Edge[]);
  const nextPos = useRef({ x: 50, y: 50 });

  // ── Init ─────────────────────────────────────────────────────
  useEffect(() => {
    let disposed = false;
    (async () => {
      const k = await createKnot();
      if (disposed) return;

      const reg = buildDefaultRegistry(k);
      registryRef.current = reg;
      graphRef.current = new Graph(reg);
      evalRef.current = new Evaluator({
        onEvaluate: (id, defId) => console.log(`[eval] ${id} (${defId})`),
      });
      setLoading(false);
    })();
    return () => { disposed = true; evalRef.current?.dispose(); };
  }, []);

  // ── Evaluate + collect meshes ────────────────────────────────
  const runEval = useCallback(async () => {
    const graph = graphRef.current;
    const ev = evalRef.current;
    if (!graph || !ev) return;

    await ev.run(graph);

    const newMeshes: MeshData[] = [];
    for (const [id] of graph.nodes) {
      const def = graph.getDef(id);
      for (const port of Object.keys(def.outputs)) {
        const out = ev.getOutput(id, port);
        if (!out) continue;
        if (out.kind === 'brep') {
          try { newMeshes.push((out.value as any).tessellate()); } catch { /* skip */ }
        } else if (out.kind === 'list' && Array.isArray(out.value)) {
          for (const item of out.value as any[]) {
            if (item && typeof item === 'object' && typeof item.tessellate === 'function') {
              try { newMeshes.push(item.tessellate()); } catch { /* skip */ }
            }
          }
        }
      }
    }
    setMeshes(newMeshes);
  }, []);

  // ── Graph node → RF node ─────────────────────────────────────
  const toRfNode = useCallback(
    (nodeId: string, pos: { x: number; y: number }): Node => {
      const graph = graphRef.current!;
      const inst = graph.getNode(nodeId)!;
      const def = graph.getDef(nodeId);
      const data: CadNodeData = {
        label: def.label ?? def.id,
        defId: def.id,
        inputs: Object.entries(def.inputs).map(([name, spec]: [string, any]) => ({ name, kind: spec.kind })),
        outputs: Object.entries(def.outputs).map(([name, spec]: [string, any]) => ({ name, kind: spec.kind })),
        constants: { ...inst.constants },
        onConstantChange: (port, value) => {
          graph.setConstant(nodeId, port, value);
          evalRef.current?.markDirty(nodeId);
          runEval();
          setRfNodes((nds) =>
            nds.map((n) =>
              n.id === nodeId
                ? { ...n, data: { ...n.data, constants: { ...graph.getNode(nodeId)!.constants } } }
                : n,
            ),
          );
        },
      };
      return { id: nodeId, type: 'cad', position: pos, data } as unknown as Node;
    },
    [runEval],
  );

  // ── Add node ─────────────────────────────────────────────────
  const addNode = useCallback(
    (defId: string) => {
      const graph = graphRef.current;
      if (!graph) return;
      const def = graph.registry.get(defId);
      if (!def) return;
      const constants: Record<string, unknown> = {};
      for (const [name, spec] of Object.entries(def.inputs) as [string, any][]) {
        if (spec.default !== undefined) constants[name] = spec.default;
      }
      // Slider metadata defaults.
      if (defId === 'core.slider') {
        constants._min ??= 0;
        constants._max ??= 10;
        constants._step ??= 0.1;
      }
      // Expression default.
      if (defId === 'math.expression') {
        constants.expr ??= 'a';
      }
      const nodeId = graph.addNode(defId, constants);
      const pos = { ...nextPos.current };
      nextPos.current = { x: pos.x + 30, y: pos.y + 40 };
      setRfNodes((nds) => [...nds, toRfNode(nodeId, pos)]);
      runEval();
    },
    [toRfNode, runEval],
  );

  // ── Connect ──────────────────────────────────────────────────
  const onConnect = useCallback(
    (conn: Connection) => {
      const graph = graphRef.current;
      if (!graph || !conn.source || !conn.target || !conn.sourceHandle || !conn.targetHandle) return;
      try { graph.connect(conn.source, conn.sourceHandle, conn.target, conn.targetHandle); }
      catch (e) { console.warn('connect rejected:', (e as Error).message); return; }
      setRfEdges((eds) => addEdge(conn, eds));
      evalRef.current?.markDirty(conn.target);
      runEval();
    },
    [runEval],
  );

  // ── Delete nodes ─────────────────────────────────────────────
  const onNodesDelete: OnNodesDelete = useCallback(
    (deleted) => {
      const graph = graphRef.current;
      if (!graph) return;
      for (const n of deleted) { evalRef.current?.evict(n.id); graph.removeNode(n.id); }
      setRfEdges(graph.wires.map((w) => ({
        id: `${w.fromNode}.${w.fromPort}-${w.toNode}.${w.toPort}`,
        source: w.fromNode, sourceHandle: w.fromPort,
        target: w.toNode, targetHandle: w.toPort,
      })));
      runEval();
    },
    [runEval],
  );

  // ── Delete edges ─────────────────────────────────────────────
  const onEdgesDelete: OnEdgesDelete = useCallback(
    (deleted) => {
      const graph = graphRef.current;
      if (!graph) return;
      for (const e of deleted) {
        if (e.targetHandle) { graph.disconnect(e.target, e.targetHandle); evalRef.current?.markDirty(e.target); }
      }
      runEval();
    },
    [runEval],
  );

  // Accordion state — all groups open by default
  const [openGroups, setOpenGroups] = useState<Set<string>>(() => new Set(PALETTE.map((g) => g.label)));
  const toggleGroup = useCallback((label: string) => {
    setOpenGroups((prev) => {
      const next = new Set(prev);
      if (next.has(label)) next.delete(label); else next.add(label);
      return next;
    });
  }, []);

  // Tour state
  const [runTour, setRunTour] = useState(false);
  const handleTourEvent = useCallback((data: EventData) => {
    if (data.status === STATUS.FINISHED || data.status === STATUS.SKIPPED) {
      setRunTour(false);
    }
  }, []);

  if (loading) {
    return <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100vh', width: '100vw' }}>Loading kernel…</div>;
  }

  return (
    <div style={{ display: 'flex', height: '100vh', width: '100vw' }}>
      <Joyride
        steps={TOUR_STEPS}
        run={runTour}
        continuous
        buttons={['back', 'primary', 'skip']}
        onEvent={handleTourEvent}
        primaryColor="#4a9eff"
        backgroundColor="#2a2a3e"
        textColor="#e0e0e0"
        overlayColor="rgba(0, 0, 0, 0.7)"
      />
      {/* Palette */}
      <div data-tour="palette" style={{ width: 180, background: '#16162a', borderRight: '1px solid #333', padding: 12, display: 'flex', flexDirection: 'column', gap: 6 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
          <span style={{ fontWeight: 700, fontSize: 14 }}>Add Node</span>
          <button
            onClick={() => setRunTour(true)}
            style={{ background: '#2a2a3e', border: '1px solid #555', borderRadius: 4, color: '#888', padding: '2px 8px', cursor: 'pointer', fontSize: 11 }}
            title="Take a guided tour"
          >?</button>
        </div>
        <div style={{ overflowY: 'auto', flex: 1, display: 'flex', flexDirection: 'column', gap: 2 }}>
          {PALETTE.map((group) => {
            const isOpen = openGroups.has(group.label);
            return (
              <div key={group.label} data-tour={`palette-${group.label.toLowerCase()}`}>
                <button
                  onClick={() => toggleGroup(group.label)}
                  style={{
                    width: '100%', background: 'none', border: 'none', color: '#888',
                    fontSize: 10, textTransform: 'uppercase', letterSpacing: 1,
                    padding: '6px 0 4px', cursor: 'pointer', textAlign: 'left',
                    display: 'flex', alignItems: 'center', gap: 4,
                  }}
                >
                  <span style={{ display: 'inline-block', width: 10, fontSize: 8, transition: 'transform 0.15s', transform: isOpen ? 'rotate(90deg)' : 'rotate(0deg)' }}>&#9654;</span>
                  {group.label}
                </button>
                {isOpen && (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 2, paddingLeft: 2 }}>
                    {group.entries.map((p) => (
                      <button
                        key={p.defId}
                        onClick={() => addNode(p.defId)}
                        style={{ background: '#2a2a3e', border: '1px solid #444', borderRadius: 4, color: '#e0e0e0', padding: '4px 8px', cursor: 'pointer', textAlign: 'left', fontSize: 11 }}
                      >
                        {p.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Graph canvas */}
      <div data-tour="graph-canvas" style={{ flex: 1 }}>
        <ReactFlow
          nodes={rfNodes} edges={rfEdges}
          onNodesChange={onNodesChange} onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          onNodesDelete={onNodesDelete} onEdgesDelete={onEdgesDelete}
          nodeTypes={nodeTypes}
          defaultEdgeOptions={{ style: { strokeWidth: 2 } }}
          fitView colorMode="dark" deleteKeyCode="Backspace"
        >
          <Background color="#333" gap={20} />
          <Controls />
          <MiniMap style={{ background: '#1a1a2e' }} nodeColor="#2a2a3e" maskColor="rgba(0,0,0,0.6)" />
        </ReactFlow>
      </div>

      {/* 3D viewport */}
      <div data-tour="viewport" style={{ width: 400, borderLeft: '1px solid #333' }}>
        <Canvas camera={{ position: [5, 5, 5], fov: 50 }}>
          <ambientLight intensity={0.4} />
          <directionalLight position={[5, 10, 5]} intensity={0.8} />
          <OrbitControls />
          {meshes.map((mesh, i) => <BrepMesh key={i} mesh={mesh} />)}
          <gridHelper args={[10, 10, '#444', '#333']} />
        </Canvas>
      </div>
    </div>
  );
}

// ── Mesh renderer ────────────────────────────────────────────────
function BrepMesh({ mesh }: { mesh: MeshData }) {
  const positions = new Float32Array(mesh.positions);
  const normals = new Float32Array(mesh.normals);
  const indices = new Uint32Array(mesh.indices);
  return (
    <mesh>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[positions, 3]} />
        <bufferAttribute attach="attributes-normal" args={[normals, 3]} />
        <bufferAttribute attach="index" args={[indices, 1]} />
      </bufferGeometry>
      <meshStandardMaterial color="#6a9eff" flatShading />
    </mesh>
  );
}

import { useState, useCallback } from 'react';
import { Canvas } from '@react-three/fiber';
import { OrbitControls, Grid } from '@react-three/drei';
import { useKnot, useBrep, KnotMesh } from 'knot-cad/react';
import type { Brep, Knot } from 'knot-cad';
import { Sidebar } from './Sidebar';

export type DemoMode = 'box' | 'extrude' | 'boolean' | 'revolve';

export interface DemoParams {
  mode: DemoMode;
  size: number;
  extrudeHeight: number;
  boolOffset: number;
  revolveAngle: number;
}

const defaultParams: DemoParams = {
  mode: 'boolean',
  size: 2,
  extrudeHeight: 1.5,
  boolOffset: 1.0,
  revolveAngle: 270,
};

export function App() {
  const { knot, loading, error: knotError } = useKnot();
  const [params, setParams] = useState<DemoParams>(defaultParams);

  const { brep, error: brepError } = useBrep(
    knot,
    (k) => buildModel(k, params),
    [params.mode, params.size, params.extrudeHeight, params.boolOffset, params.revolveAngle],
  );

  const handleExport = useCallback(
    (format: 'stl' | 'glb' | 'step') => {
      if (!brep) return;
      let data: Uint8Array | string;
      let filename: string;
      let mime: string;
      if (format === 'stl') {
        data = brep.toSTL();
        filename = 'model.stl';
        mime = 'model/stl';
      } else if (format === 'glb') {
        data = brep.toGLB();
        filename = 'model.glb';
        mime = 'model/gltf-binary';
      } else {
        data = brep.toSTEP();
        filename = 'model.step';
        mime = 'text/plain';
      }
      const blob = new Blob(
        [typeof data === 'string' ? new TextEncoder().encode(data) : data],
        { type: mime },
      );
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = filename;
      a.click();
      URL.revokeObjectURL(a.href);
    },
    [brep],
  );

  if (knotError) {
    return (
      <div style={{ padding: 40, color: '#ff6b6b' }}>
        <h2>Failed to load kernel</h2>
        <pre>{knotError.message}</pre>
      </div>
    );
  }

  return (
    <>
      <Sidebar
        params={params}
        onChange={setParams}
        onExport={handleExport}
        faceCount={brep?.faceCount ?? null}
        loading={loading}
        error={brepError}
      />
      <div style={{ flex: 1 }}>
        <Canvas camera={{ position: [6, 6, 6], fov: 45 }}>
          <color attach="background" args={['#1a1a2e']} />
          <ambientLight intensity={0.4} />
          <directionalLight position={[10, 10, 5]} intensity={0.8} />
          <directionalLight position={[-5, -3, -5]} intensity={0.3} />
          <KnotMesh brep={brep} color="#4499dd" />
          <Grid
            infiniteGrid
            cellSize={1}
            sectionSize={5}
            fadeDistance={30}
            cellColor="#2a2a4e"
            sectionColor="#3a3a5e"
          />
          <OrbitControls makeDefault />
        </Canvas>
      </div>
    </>
  );
}

function buildModel(k: Knot, p: DemoParams): Brep {
  switch (p.mode) {
    case 'box':
      return k.box(p.size, p.size, p.size);

    case 'extrude': {
      const s = p.size / 2;
      const profile = k.profile([[-s, -s], [s, -s], [s, s], [-s, s]]);
      const solid = profile.extrude({ distance: p.extrudeHeight });
      profile.free();
      return solid;
    }

    case 'boolean': {
      const a = k.box(p.size, p.size, p.size);
      const b = k.cylinder({
        center: { x: p.boolOffset, y: 0, z: 0 },
        radius: p.size * 0.3,
        height: p.size + 0.5,
      });
      const result = a.subtract(b);
      a.free();
      b.free();
      return result;
    }

    case 'revolve': {
      const s = p.size / 4;
      const inner = 0.5;
      const outer = inner + s;
      const profile = k.profile([
        { x: inner, y: 0, z: 0 },
        { x: outer, y: 0, z: 0 },
        { x: outer, y: 0, z: s },
        { x: inner, y: 0, z: s },
      ]);
      const angle = (p.revolveAngle / 180) * Math.PI;
      const solid = profile.revolve({
        axisOrigin: { x: 0, y: 0, z: 0 },
        axisDirection: { x: 0, y: 0, z: 1 },
        angle,
      });
      profile.free();
      return solid;
    }
  }
}

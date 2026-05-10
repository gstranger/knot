/// <reference types="@react-three/fiber" />
import { useMemo, useEffect, useRef } from 'react';
import * as THREE from 'three';
import { type Brep, type TessellateOptions } from '../kernel';

export interface KnotMeshProps {
  /** The BRep to render. Renders nothing if null. */
  brep: Brep | null;
  /** Material color. Defaults to #4488cc. */
  color?: THREE.ColorRepresentation;
  /** Tessellation quality options. */
  tessellateOptions?: TessellateOptions;
  /** Additional props forwarded to the <mesh> element. */
  [key: string]: unknown;
}

/**
 * React Three Fiber component that renders a Brep as a triangle mesh.
 *
 * Tessellates the BRep into a BufferGeometry with positions, normals,
 * and indexed triangles.  Properly disposes GPU resources when the
 * geometry changes or the component unmounts.
 *
 * If tessellation fails (e.g. freed Brep), renders nothing instead of crashing.
 */
export function KnotMesh({
  brep,
  color = '#4488cc',
  tessellateOptions,
  ...meshProps
}: KnotMeshProps) {
  const geoRef = useRef<THREE.BufferGeometry | null>(null);

  const geometry = useMemo(() => {
    // Dispose previous geometry before creating a new one
    if (geoRef.current) {
      geoRef.current.dispose();
      geoRef.current = null;
    }

    if (!brep) return null;

    try {
      const mesh = brep.tessellate(tessellateOptions);
      const geo = new THREE.BufferGeometry();

      geo.setAttribute('position', new THREE.BufferAttribute(mesh.positions, 3));
      geo.setAttribute('normal', new THREE.BufferAttribute(mesh.normals, 3));
      geo.setIndex(new THREE.BufferAttribute(mesh.indices, 1));

      geoRef.current = geo;
      return geo;
    } catch {
      // Brep was freed or tessellation failed — render nothing
      return null;
    }
  }, [brep, tessellateOptions]);

  // Dispose on unmount
  useEffect(() => {
    return () => {
      if (geoRef.current) {
        geoRef.current.dispose();
        geoRef.current = null;
      }
    };
  }, []);

  if (!geometry) return null;

  return (
    <mesh geometry={geometry} {...(meshProps as any)}>
      <meshStandardMaterial color={color} />
    </mesh>
  );
}

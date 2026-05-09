/// <reference types="@react-three/fiber" />
import { useMemo } from 'react';
import * as THREE from 'three';
import { type Brep } from '../kernel';

export interface KnotMeshProps {
  /** The BRep to render. */
  brep: Brep | null;
  /** Material color. Defaults to #4488cc. */
  color?: THREE.ColorRepresentation;
  /** Additional props forwarded to the <mesh> element. */
  [key: string]: unknown;
}

/**
 * React Three Fiber component that renders a Brep as a triangle mesh.
 *
 * Tessellates the BRep into a BufferGeometry with positions, normals,
 * and indexed triangles. Re-tessellates when the brep reference changes.
 *
 * @example
 * ```tsx
 * <Canvas>
 *   <KnotMesh brep={myBrep} color="orange" />
 * </Canvas>
 * ```
 */
export function KnotMesh({ brep, color = '#4488cc', ...meshProps }: KnotMeshProps) {
  const geometry = useMemo(() => {
    if (!brep) return null;

    const mesh = brep.tessellate();
    const geo = new THREE.BufferGeometry();

    geo.setAttribute('position', new THREE.BufferAttribute(mesh.positions, 3));
    geo.setAttribute('normal', new THREE.BufferAttribute(mesh.normals, 3));
    geo.setIndex(new THREE.BufferAttribute(mesh.indices, 1));

    return geo;
  }, [brep]);

  if (!geometry) return null;

  return (
    <mesh geometry={geometry} {...(meshProps as any)}>
      <meshStandardMaterial color={color} />
    </mesh>
  );
}

import type { MeshData, TessellateOptions } from '../../kernel';
import { defineNode } from './define';

/**
 * A sink node: reads a brep, tessellates it, and pushes the mesh to a
 * callback stored as the node's `onMesh` constant.
 *
 * The View has no output ports — it terminates a flow. The mesh data is
 * a copy (ArrayBuffers), so there are no ownership concerns; the brep
 * handle stays owned by the upstream cache.
 */
export interface ViewConstants {
  onMesh?: (mesh: MeshData) => void;
  tessellate?: TessellateOptions;
}

export const ViewNode = defineNode({
  id: 'view.brep',
  label: 'View',
  inputs: { brep: { kind: 'brep' } },
  outputs: {},
  evaluate: ({ brep }, ctx) => {
    const { onMesh, tessellate } = ctx.constants as ViewConstants;
    if (onMesh) onMesh(brep.tessellate(tessellate));
    return {};
  },
});

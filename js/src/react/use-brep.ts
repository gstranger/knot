import { useState, useEffect, useRef } from 'react';
import { type Brep, type Knot } from '../kernel';

export interface UseBrepResult {
  /** The computed Brep, or null while knot is loading / if factory errored. */
  brep: Brep | null;
  /** Set if the factory threw an error. */
  error: Error | null;
}

/**
 * React hook that creates a Brep from a factory function and
 * automatically frees it when dependencies change or on unmount.
 *
 * @param knot    - The kernel instance (from `useKnot().knot`).
 *                  When null, the factory is not called and the hook returns null.
 * @param factory - Receives the kernel and returns a Brep.
 *                  Intermediate Breps created inside should be freed manually.
 * @param deps    - Re-runs the factory when these change (same semantics as useEffect).
 *
 * @example
 * ```tsx
 * const { knot } = useKnot();
 * const { brep, error } = useBrep(knot, (k) => {
 *   const a = k.box(2, 2, 2);
 *   const b = k.cylinder({ radius: 0.8, height: 3 }).translate(offset, 0, 0);
 *   const result = a.subtract(b);
 *   a.free(); b.free();
 *   return result;
 * }, [offset]);
 * ```
 */
export function useBrep(
  knot: Knot | null,
  factory: (knot: Knot) => Brep,
  deps: React.DependencyList,
): UseBrepResult {
  const [state, setState] = useState<UseBrepResult>({ brep: null, error: null });

  // Track which breps have been freed so we never return one.
  // This is a Set rather than a single ref because React may batch
  // multiple state updates, leaving old references in state temporarily.
  const freedSet = useRef(new WeakSet<Brep>());

  useEffect(() => {
    if (!knot) {
      setState({ brep: null, error: null });
      return;
    }

    let result: Brep;
    try {
      result = factory(knot);
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setState({ brep: null, error });
      return;
    }

    setState({ brep: result, error: null });

    return () => {
      // Mark as freed BEFORE freeing — any render that sees this brep
      // in state will filter it out via the freedSet check below.
      freedSet.current.add(result);
      try { result.free(); } catch { /* already freed */ }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [knot, ...deps]);

  // If the brep in state was freed by cleanup but React hasn't re-rendered
  // with the new state yet, return null instead of the freed handle.
  const brep = state.brep && !freedSet.current.has(state.brep) ? state.brep : null;
  return { brep, error: state.error };
}

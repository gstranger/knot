import { useMemo, useEffect, useRef } from 'react';
import { type Brep } from '../kernel';

/**
 * React hook that creates a Brep from a factory function and
 * automatically frees it on unmount or when deps change.
 *
 * @param factory - Function that creates and returns a Brep.
 *                  Called inside useMemo, so it re-runs when deps change.
 * @param deps    - Dependency array (same semantics as useMemo).
 *
 * @example
 * ```tsx
 * const brep = useBrep((knot) => {
 *   const a = knot.box(2, 2, 2);
 *   const b = knot.cylinder({ radius: 0.8, height: 3 }).translate(offset, 0, 0);
 *   const result = a.subtract(b);
 *   a.free(); b.free();
 *   return result;
 * }, [offset]);
 * ```
 */
export function useBrep(
  factory: () => Brep | null,
  deps: React.DependencyList,
): Brep | null {
  const prevRef = useRef<Brep | null>(null);

  const brep = useMemo(() => {
    // Free the previous brep before creating a new one
    if (prevRef.current) {
      try { prevRef.current.free(); } catch { /* already freed */ }
    }
    const result = factory();
    prevRef.current = result;
    return result;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  // Clean up on unmount
  useEffect(() => {
    return () => {
      if (prevRef.current) {
        try { prevRef.current.free(); } catch { /* already freed */ }
        prevRef.current = null;
      }
    };
  }, []);

  return brep;
}

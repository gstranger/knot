import { useState, useEffect } from 'react';
import { createKnot, type Knot, type InitInput } from '../kernel';

let cachedKnot: Knot | null = null;
let initPromise: Promise<Knot> | null = null;

/**
 * React hook that initializes the WASM kernel.
 * Returns null while loading, then the Knot modeling API.
 *
 * Safe to call from multiple components — WASM loads once.
 */
export function useKnot(wasmPath?: InitInput): Knot | null {
  const [knot, setKnot] = useState<Knot | null>(cachedKnot);

  useEffect(() => {
    if (cachedKnot) {
      setKnot(cachedKnot);
      return;
    }

    if (!initPromise) {
      initPromise = createKnot(wasmPath);
    }

    let cancelled = false;
    initPromise.then((k) => {
      cachedKnot = k;
      if (!cancelled) setKnot(k);
    });

    return () => { cancelled = true; };
  }, [wasmPath]);

  return knot;
}

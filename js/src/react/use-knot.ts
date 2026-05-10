import { useState, useEffect } from 'react';
import { createKnot, type Knot, type InitInput } from '../kernel';

export interface UseKnotResult {
  /** The kernel API, or null while loading. */
  knot: Knot | null;
  /** True while the WASM module is loading. */
  loading: boolean;
  /** Set if WASM initialization failed. */
  error: Error | null;
}

let cached: UseKnotResult = { knot: null, loading: true, error: null };
let initPromise: Promise<Knot> | null = null;

/**
 * React hook that initializes the WASM kernel.
 *
 * Returns `{ knot, loading, error }` so components can render
 * loading and error states.  Safe to call from multiple components —
 * the WASM module loads once.
 */
export function useKnot(wasmPath?: InitInput): UseKnotResult {
  const [state, setState] = useState<UseKnotResult>(cached);

  useEffect(() => {
    // Already resolved from a previous render / component
    if (cached.knot) {
      setState(cached);
      return;
    }

    if (!initPromise) {
      initPromise = createKnot(wasmPath);
    }

    let cancelled = false;

    initPromise
      .then((k) => {
        cached = { knot: k, loading: false, error: null };
        if (!cancelled) setState(cached);
      })
      .catch((err) => {
        const error = err instanceof Error ? err : new Error(String(err));
        cached = { knot: null, loading: false, error };
        if (!cancelled) setState(cached);
      });

    return () => { cancelled = true; };
  }, [wasmPath]);

  return state;
}

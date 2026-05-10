import { createRoot } from 'react-dom/client';
import { App } from './App';

// Note: StrictMode is intentionally not used here. React 18 StrictMode
// double-invokes effects to detect cleanup bugs, but WASM objects freed
// during the simulated unmount leave the module in a corrupted state
// (Rust panics compile to WASM traps, which are unrecoverable).
// Our useBrep hook handles cleanup correctly without StrictMode's help.
createRoot(document.getElementById('root')!).render(<App />);

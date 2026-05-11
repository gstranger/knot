/**
 * Unit tests for the typed error layer. End-to-end tests (triggering
 * an actual kernel error and asserting it parses) live in
 * `error.e2e.test.ts` since they need the WASM module loaded.
 */
import { describe, expect, it } from 'vitest';
import { KnotError, parseKnotError, isKnotError } from './error';

describe('parseKnotError', () => {
  it('parses a well-formed kernel message', () => {
    const e = new Error('E404:OperationFailed:boolean budget exceeded at stage SSI');
    const ke = parseKnotError(e);
    expect(ke).toBeInstanceOf(KnotError);
    expect(ke!.code).toBe('E404');
    expect(ke!.kind).toBe('OperationFailed');
    expect(ke!.detail).toBe('boolean budget exceeded at stage SSI');
  });

  it('parses a multi-line detail (colon in detail)', () => {
    const e = new Error('E204:TopoInconsistency:Euler violation: expected 0, got -2');
    const ke = parseKnotError(e);
    expect(ke).not.toBeNull();
    expect(ke!.detail).toBe('Euler violation: expected 0, got -2');
  });

  it('returns null for non-kernel-format messages', () => {
    expect(parseKnotError(new Error('something bad happened'))).toBeNull();
    expect(parseKnotError(new Error('not a knot error'))).toBeNull();
    expect(parseKnotError(new Error('E12:OperationFailed:short code'))).toBeNull(); // not 3-digit
  });

  it('returns null for non-Error inputs', () => {
    expect(parseKnotError(null)).toBeNull();
    expect(parseKnotError(undefined)).toBeNull();
    expect(parseKnotError('string error')).toBeNull();
    expect(parseKnotError({ message: 'E404:Operation:fake' })).toBeNull();
  });
});

describe('isKnotError', () => {
  it('agrees with parseKnotError', () => {
    expect(isKnotError(new Error('E404:OperationFailed:x'))).toBe(true);
    expect(isKnotError(new Error('regular error'))).toBe(false);
    expect(isKnotError(null)).toBe(false);
  });
});

describe('KnotError class', () => {
  it('extends Error and stringifies to the canonical format', () => {
    const ke = new KnotError('E404', 'OperationFailed', 'boom');
    expect(ke).toBeInstanceOf(Error);
    expect(ke.message).toBe('E404:OperationFailed:boom');
    expect(ke.name).toBe('KnotError');
  });

  it('preserves structured fields after re-parse', () => {
    const original = new KnotError('E201', 'TopoInconsistency', 'non-manifold edge');
    const reparsed = parseKnotError(original);
    expect(reparsed!.code).toBe(original.code);
    expect(reparsed!.kind).toBe(original.kind);
    expect(reparsed!.detail).toBe(original.detail);
  });
});

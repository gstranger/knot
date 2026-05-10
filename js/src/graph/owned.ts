/**
 * Refcounted handle over a resource that needs explicit disposal
 * (typically a WASM-allocated value with a `free()` method).
 *
 * The cache holds one ref. Anyone reading the value calls `retain()`
 * to keep it alive past cache eviction, and `release()` when done.
 *
 * The underlying value is freed exactly once, when the refcount hits zero.
 */
export interface Disposable {
  free(): void;
}

export class Owned<T extends Disposable> {
  private refs: number;
  private freed: boolean = false;

  constructor(private readonly _value: T) {
    this.refs = 1;
  }

  get value(): T {
    if (this.freed) throw new Error('Owned: access after free');
    return this._value;
  }

  retain(): this {
    if (this.freed) throw new Error('Owned: retain after free');
    this.refs++;
    return this;
  }

  release(): void {
    if (this.freed) return;
    if (--this.refs <= 0) {
      this.freed = true;
      this._value.free();
    }
  }

  get refCount(): number {
    return this.refs;
  }

  get isFreed(): boolean {
    return this.freed;
  }
}

export const own = <T extends Disposable>(v: T): Owned<T> => new Owned(v);

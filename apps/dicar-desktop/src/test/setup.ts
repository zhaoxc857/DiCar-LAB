import "@testing-library/jest-dom/vitest";
import "fake-indexeddb/auto";

// Node >= 25 predefines `localStorage`/`sessionStorage` getters on globalThis
// that yield undefined unless --localstorage-file is set, which makes Vitest's
// jsdom environment skip installing jsdom's Storage globals. Install an
// in-memory Web Storage implementation instead; methods live on the global
// `Storage` prototype so `vi.spyOn(Storage.prototype, ...)` keeps working.
if (typeof globalThis.localStorage === "undefined") {
  class MemoryStorage {
    #data = new Map<string, string>();

    get length(): number {
      return this.#data.size;
    }

    clear(): void {
      this.#data.clear();
    }

    getItem(key: string): string | null {
      return this.#data.get(String(key)) ?? null;
    }

    key(index: number): string | null {
      return [...this.#data.keys()][index] ?? null;
    }

    removeItem(key: string): void {
      this.#data.delete(String(key));
    }

    setItem(key: string, value: string): void {
      this.#data.set(String(key), String(value));
    }
  }

  Object.defineProperty(globalThis, "Storage", { value: MemoryStorage, configurable: true, writable: true });
  Object.defineProperty(globalThis, "localStorage", { value: new MemoryStorage(), configurable: true });
  Object.defineProperty(globalThis, "sessionStorage", { value: new MemoryStorage(), configurable: true });
}

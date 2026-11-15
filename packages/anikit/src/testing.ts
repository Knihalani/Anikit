/**
 * Deterministic testing helpers (for unit tests, visual regression, video
 * export, and coding agents verifying animations headlessly).
 *
 * ```ts
 * import { testing } from 'anikit';
 * testing.useFakeTime();
 * animate(el, { x: 100 }, { duration: 1 });
 * testing.advance(0.5);
 * expect(el.style.transform).toBe('translate(50px,0px)');   // with ease: 'linear'
 * ```
 */
import { engine, clearRecords } from './core';
import { core, ready, W } from './wasm';
import { onWarning, setWarnings, resetWarnings } from './debug';

export const testing = {
  /** Wait for the WebAssembly core (needed once in async test setups). */
  ready,
  /** Drive time manually: no requestAnimationFrame, no lag smoothing. */
  useFakeTime(): void {
    engine.manual(true);
    engine.lagSmoothing(0);
  },
  /** Advance `seconds` in frames of `1 / fps`. */
  advance(seconds: number, fps = 60): void {
    const step = 1000 / fps;
    let ms = seconds * 1000;
    while (ms > 1e-9) {
      const d = Math.min(step, ms);
      engine.tick(d);
      ms -= d;
    }
  },
  /** Advance until nothing is running (or `maxSeconds` passes). Returns seconds advanced. */
  finishAll(maxSeconds = 60, fps = 60): number {
    let t = 0;
    while (t < maxSeconds && W && W.ak_active_count() > 0) {
      engine.tick(1000 / fps);
      t += 1 / fps;
    }
    return t;
  },
  /** Stop and forget everything (between tests), including which warnings were shown. Values stay where they are. */
  reset(): void {
    resetWarnings();
    if (!W) return;
    core().ak_clear();
    clearRecords();
  },
  /**
   * Collect warnings instead of printing them; returns the live array. Call it
   * *before* the code you want to check. Each call starts a fresh list and
   * forgets which messages were already shown, so repeats are captured again.
   * `onWarning(null)` restores console output.
   */
  captureWarnings(): string[] {
    const list: string[] = [];
    setWarnings(true);
    resetWarnings();
    onWarning((m) => list.push(m));
    return list;
  },
  /** JSON snapshot of running animations (same as `engine.inspect()`). */
  inspect: () => engine.inspect(),
};

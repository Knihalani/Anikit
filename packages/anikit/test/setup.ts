import { engine, ready } from '../src/index';
export async function setup(): Promise<void> {
  await ready;
  engine.manual(true);
  engine.lagSmoothing(0);
}
/** Advance `seconds` in frames of ~16ms. */
export function advance(seconds: number, step = 1000 / 60): void {
  let ms = seconds * 1000;
  while (ms > 1e-9) {
    const d = Math.min(step, ms);
    engine.tick(d);
    ms -= d;
  }
}
export function box(styles: Partial<CSSStyleDeclaration> = {}): HTMLDivElement {
  const el = document.createElement('div');
  Object.assign(el.style, styles);
  document.body.appendChild(el);
  return el;
}

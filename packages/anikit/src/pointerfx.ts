/**
 * Two small, common interactions:
 *
 * ```ts
 * magnetic('.cta', { strength: 0.35, radius: 120 });   // leans towards the pointer, springs back
 * parallax('.layer-back', { speed: -0.3 });            // moves slower than the page while it scrolls past
 * ```
 */
import { spring, PhysicsOptions, PhysicsAnimation } from './spring';
import { animate } from './animate';
import { scroll } from './scroll';
import { prefersReduced } from './core';
import { track } from './context';
import { checkKeys, checkTargets } from './debug';
import type { OptionKeys } from './debug';
import { toTargets, Targets, isElement, stateOf, snapshot, Kind } from './props';

export interface MagneticOptions {
  /** How far the element follows the pointer, as a fraction of the pointer's offset (default 0.3). */
  strength?: number;
  /** Distance in px from the element's edge within which it reacts (default 80). */
  radius?: number;
  /** Spring for following and returning (default `{ stiffness: 220, damping: 16 }`). */
  spring?: PhysicsOptions;
}
const MAGNETIC_KEYS: OptionKeys<MagneticOptions> = { strength: 0, radius: 0, spring: 0 };

/** What `magnetic()` and `parallax()` return. */
export interface PointerFx {
  /** Stop and put the elements back where they were. */
  revert(): void;
  /** Stop, leaving the elements where they are. */
  kill(): void;
}

/**
 * Elements that lean towards the pointer when it comes near (buttons, icons),
 * springing back when it leaves. Uses the `x`/`y` transforms. Does nothing
 * under reduced motion. Registered with `context()`.
 */
export function magnetic(targets: Targets, options: MagneticOptions = {}): PointerFx {
  checkKeys('magnetic()', options, MAGNETIC_KEYS);
  const els = toTargets(targets).filter(isElement) as HTMLElement[];
  checkTargets('magnetic()', targets, els.length);
  const strength = options.strength ?? 0.3, radius = options.radius ?? 80;
  const sp: PhysicsOptions = { stiffness: 220, damping: 16, ...options.spring };
  const pulled = new Set<HTMLElement>();
  // The transform before any pull (revert() puts it back, inline style and
  // the engine's cached transform both), and each element's running spring.
  const restores = els.map((el) => snapshot(el, 'x', Kind.Transform));
  const springs = new Map<HTMLElement, PhysicsAnimation>();
  const pull = (el: HTMLElement, x: number, y: number) => springs.set(el, spring(el, { x, y }, sp));
  const move = (e: PointerEvent) => {
    if (prefersReduced()) return;
    for (const el of els) {
      // The element's resting box: its rect minus the offset we applied.
      const st = stateOf(el);
      const ox = st.t[0], oy = st.t[1];
      const r = el.getBoundingClientRect();
      const cx = r.left + r.width / 2 - ox, cy = r.top + r.height / 2 - oy;
      const dx = e.clientX - cx, dy = e.clientY - cy;
      const near = Math.abs(dx) < r.width / 2 + radius && Math.abs(dy) < r.height / 2 + radius;
      if (near) {
        pulled.add(el);
        pull(el, dx * strength, dy * strength);
      } else if (pulled.delete(el)) pull(el, 0, 0);
    }
  };
  const leave = () => {
    for (const el of pulled) pull(el, 0, 0);
    pulled.clear();
  };
  const win = typeof window !== 'undefined' ? window : null;
  win?.addEventListener('pointermove', move, { passive: true });
  document.documentElement.addEventListener('pointerleave', leave);
  const kill = () => {
    win?.removeEventListener('pointermove', move);
    document.documentElement.removeEventListener('pointerleave', leave);
  };
  return track({
    kill,
    revert() {
      kill();
      for (const a of springs.values()) a.stop();
      springs.clear();
      pulled.clear();
      for (const r of restores) r();
    },
  });
}

export interface ParallaxOptions {
  /**
   * Speed relative to the page while the element crosses the viewport: 0 moves
   * with the page, −0.5 lags behind (background feel), 0.5 runs ahead (default −0.3).
   */
  speed?: number;
  /** Axis (default `'y'`). */
  axis?: 'x' | 'y';
  /** Smoothing in seconds (default 0: locked to the scroll). */
  smooth?: number;
}
const PARALLAX_KEYS: OptionKeys<ParallaxOptions> = { speed: 0, axis: 0, smooth: 0 };

/**
 * Parallax: each element drifts at `speed` relative to the page while it
 * scrolls through the viewport (scroll-linked, so it scrubs both ways).
 * Registered with `context()`; does nothing under reduced motion.
 */
export function parallax(targets: Targets, options: ParallaxOptions = {}): PointerFx {
  checkKeys('parallax()', options, PARALLAX_KEYS);
  const els = toTargets(targets).filter(isElement) as HTMLElement[];
  checkTargets('parallax()', targets, els.length);
  const speed = options.speed ?? -0.3, axis = options.axis ?? 'y';
  const offs: Array<() => void> = [];
  if (!prefersReduced()) {
    // Read every size before creating any animation (no layout per element).
    const spans = els.map((el) => (axis === 'y' ? innerHeight + el.offsetHeight : innerWidth + el.offsetWidth) || 1000);
    for (let i = 0; i < els.length; i++) {
      const el = els[i];
      // Over the scroll range (viewport + element), shift by −speed × that distance.
      const d = (-speed * spans[i]) / 2;
      const a = animate(el, { [axis]: [-d, d] }, { ease: 'linear', autoplay: false });
      offs.push(scroll(a, { target: el, axis, smooth: options.smooth }), () => a.revert());
    }
  }
  const kill = () => { for (const f of offs.splice(0)) f(); };
  return track({ kill, revert: kill });
}

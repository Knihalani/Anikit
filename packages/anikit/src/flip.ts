/**
 * FLIP layout animations: animate elements between two layouts using only
 * transforms (compositor-friendly), no matter how the layout changed.
 *
 * Beyond moving elements, `flip()` handles the three things layout changes
 * usually come with:
 * - **entering** elements (in the new layout only) play `enter` keyframes;
 * - **leaving** elements (removed or hidden by the change) are popped out of
 *   the flow at their last position, play `leave` keyframes and are then
 *   removed/hidden for real, so their siblings reflow immediately;
 * - **shared elements**: different elements with the same `data-flip-id`
 *   (a thumbnail and its expanded view) morph into each other.
 *
 * ```ts
 * flip('.item', () => list.classList.toggle('grid'), { duration: 0.6, ease: 'spring(bounce: 0.2)' });
 * flip('.card', () => filterCards(), { fade: true });                        // enter/leave with a fade
 * flip('[data-flip-id]', () => { thumb.hidden = true; modal.hidden = false; }); // shared element
 * // or capture manually
 * const state = flip.capture('.item'); reorder(); flip.from(state);
 * ```
 */
import { Animation, AnimateOptions, Props, ANIMATE_KEYS } from './animate';
import { checkTargets, checkKeys } from './debug';
import type { OptionKeys } from './debug';
import { Timeline } from './timeline';
import { Playable } from './playable';
import { Targets, toTargets, stateOf, flushTransforms, computed, isElement, markDirty } from './props';
import { r3, s3 } from './values';

export interface FlipState {
  /** The captured elements. */
  els: Element[];
  /** Their boxes at capture time. */
  rects: DOMRect[];
  /** Their inline opacity at capture time. */
  opacity: string[];
  /** Whether each element was rendered (not `display: none` / detached) at capture. */
  visible: boolean[];
  /** Shared-element ids (`data-flip-id` by default). */
  ids: Array<string | null>;
  /** Where each element lived, so removed elements can animate out in place. */
  parents: Array<Node | null>;
  /** Their next siblings at capture time (to put removed elements back for their exit). */
  nexts: Array<Node | null>;
  /** Computed `display` at capture (used to show hidden leaving elements). */
  display: string[];
  /** The targets as given (a selector is queried again after the change). */
  targets?: Targets;
  /** Page scroll at capture (the rects are in viewport coordinates). */
  scrollX?: number;
  /** Page scroll at capture. */
  scrollY?: number;
}

export interface FlipOptions extends AnimateOptions {
  /** Animate size changes with scale (default true). */
  scale?: boolean;
  /** Fade entering and leaving elements (shorthand for `enter: { opacity: [0, 1] }, leave: { opacity: 0 }`). */
  fade?: boolean;
  /** Keyframes for elements that appear with the change. */
  enter?: Props;
  /** Keyframes for elements that disappear with the change (played at their last position before removal). */
  leave?: Props;
  /** Custom entrance: return an animation for the entering elements. */
  onEnter?: (els: Element[]) => Playable | void;
  /** Custom exit: return an animation; leaving elements are removed/hidden when it completes. */
  onLeave?: (els: Element[]) => Playable | void;
  /** Elements to consider after the change (default: the selector re-queried, or the captured elements). */
  targets?: Targets;
  /** Shared-element id of an element (default: its `data-flip-id` attribute). */
  id?: (el: Element) => string | null | undefined;
  /** Cross-fade when both shared elements stay visible (default true). */
  crossfade?: boolean;
  /**
   * Keep `border-radius` undistorted while scaling, by counter-scaling it every
   * frame (default false: that's a style recalc and repaint per element per
   * frame, where transforms alone stay on the compositor).
   */
  correctRadius?: boolean;
}

const FLIP_KEYS: OptionKeys<Omit<FlipOptions, keyof AnimateOptions>> = {
  scale: 0, fade: 0, enter: 0, leave: 0, onEnter: 0, onLeave: 0, targets: 0, id: 0, crossfade: 0, correctRadius: 0,
};

const defaultId = (el: Element) => el.getAttribute('data-flip-id');

/** Current opacity (inline first), or the stylesheet's with `natural`; 1 when unknown. */
function opacityOf(el: Element, natural = false): number {
  const v = parseFloat((!natural && (el as HTMLElement).style.opacity) || computed(el, 'opacity'));
  return Number.isFinite(v) ? v : 1;
}

function isRendered(el: Element): boolean {
  if (!el.isConnected) return false;
  // Native check first (it also sees hidden ancestors), then the element itself.
  const cv = (el as any).checkVisibility;
  if (typeof cv === 'function' && !cv.call(el)) return false;
  if (computed(el, 'display') === 'none') return false;
  // `hidden` without an author `display` (environments without a UA sheet).
  return !((el as HTMLElement).hidden && !(el as HTMLElement).style.display);
}

/** Rendered, judged from its box first (no style query for the common case). */
const shown = (el: Element, r: DOMRect) => (r.width > 0 || r.height > 0 ? el.isConnected : isRendered(el));

function capture(targets: Targets, options: Pick<FlipOptions, 'id' | 'fade' | 'leave' | 'onLeave'> = {}): FlipState {
  const els = toTargets(targets).filter(isElement);
  const id = options.id ?? defaultId;
  // Reads only (one layout pass).
  const rects = els.map((e) => e.getBoundingClientRect());
  const state: FlipState = {
    els,
    rects,
    opacity: els.map((e) => (e as HTMLElement).style.opacity),
    visible: els.map((e, i) => shown(e, rects[i])),
    ids: els.map((e) => id(e) ?? null),
    parents: els.map((e) => e.parentNode),
    nexts: els.map((e) => e.nextSibling),
    // Only needed to show hidden elements while they animate out.
    display: options.fade || options.leave || options.onLeave ? els.map((e) => computed(e, 'display') || 'block') : [],
    targets: typeof targets === 'string' ? targets : undefined,
    scrollX: typeof window !== 'undefined' ? window.scrollX : 0,
    scrollY: typeof window !== 'undefined' ? window.scrollY : 0,
  };
  watchInsertions(state);
  return state;
}

/** Transform each moving element should end on (survives interrupted flips). */
const homes = new WeakMap<Element, { x: number; y: number; sx: number; sy: number }>();

/** Elements currently animating out (popped out of the flow). */
interface Leaving {
  restore: () => void;
  anim: Playable | null;
  /** It had been removed from the DOM (and was put back to animate out). */
  removed: boolean;
}
const leaving = new WeakMap<Element, Leaving>();

/**
 * Take an element out of the flow, pinned at `rect` (viewport coordinates),
 * with `position: absolute`. Writes only; call `place()` after the layout
 * reads of the same batch. `restore()` undoes everything.
 */
export function popOut(el: HTMLElement, rect: DOMRect, display?: string): { restore: () => void; place: () => void } {
  const st = el.style;
  const prevStyle = el.getAttribute('style');
  const s = stateOf(el);
  const prevT = s.t.slice();
  const prevU = s.u.slice();
  st.position = 'absolute';
  st.margin = '0px';
  st.boxSizing = 'border-box';
  st.width = rect.width + 'px';
  st.height = rect.height + 'px';
  st.left = '0px';
  st.top = '0px';
  st.pointerEvents = 'none';
  if (display) st.display = display;
  // Exit keyframes start from an untransformed box at the old position.
  s.t = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0];
  s.u = ['px', 'px', 'px'];
  st.transform = 'none';
  return {
    place: () => {
      const m = el.getBoundingClientRect();
      st.left = rect.left - m.left + 'px';
      st.top = rect.top - m.top + 'px';
    },
    restore: () => {
      if (prevStyle === null) el.removeAttribute('style');
      else el.setAttribute('style', prevStyle);
      s.t = prevT;
      s.u = prevU;
    },
  };
}

/** Whether `el` is currently animating out. */
export function isLeaving(el: Element): boolean {
  return leaving.has(el);
}

/**
 * Cancel an element's pending exit: its animation stops and its styles are
 * restored. Returns its opacity at that moment and whether it had been removed.
 */
export function cancelLeave(el: Element): { opacity: string; removed: boolean } | null {
  const l = leaving.get(el);
  if (!l) return null;
  leaving.delete(el);
  const opacity = (el as HTMLElement).style.opacity;
  l.anim?.kill();
  l.restore();
  return { opacity, removed: l.removed };
}

/**
 * Register a popped-out element. Returns `attach(anim)`, which returns the
 * completion handler: it restores the styles and runs `done` unless the exit
 * was cancelled (or replaced) in the meantime.
 */
export function trackLeave(el: Element, restore: () => void, removed: boolean, done: () => void): (anim: Playable | null) => () => void {
  const rec: Leaving = { restore, anim: null, removed };
  leaving.set(el, rec);
  return (anim) => {
    rec.anim = anim;
    return () => {
      if (leaving.get(el) !== rec) return;
      leaving.delete(el);
      restore();
      done();
    };
  };
}

/** Nodes inserted between capture and `from()` (to tell re-added elements from ones we put back). */
const observers = new WeakMap<FlipState, MutationObserver>();
function watchInsertions(state: FlipState): void {
  if (typeof MutationObserver === 'undefined' || typeof document === 'undefined') return;
  if (!state.els.some((e) => leaving.get(e)?.removed)) return;
  const mo = new MutationObserver(() => {});
  mo.observe(document, { childList: true, subtree: true });
  observers.set(state, mo);
}
function insertedSince(state: FlipState): Set<Node> {
  const out = new Set<Node>();
  const mo = observers.get(state);
  if (!mo) return out;
  for (const r of mo.takeRecords()) r.addedNodes.forEach((n) => out.add(n));
  mo.disconnect();
  observers.delete(state);
  return out;
}

/** Where an element's content is drawn at the start of a flip: x -> ax + bx·x, y -> ay + by·y. */
interface StartMap {
  ax: number;
  bx: number;
  ay: number;
  by: number;
}

function isFixed(el: Element): boolean {
  for (let e: Element | null = el; e; e = (e as HTMLElement).offsetParent) {
    if (computed(e, 'position') === 'fixed') return true;
  }
  return false;
}

interface Radius {
  el: HTMLElement;
  t: number[];
  /** Corner radii in px: [h, v] × 4 (top-left, top-right, bottom-right, bottom-left). */
  r: number[];
  /** All corners equal (the common case: a shorter string to write). */
  same: boolean;
  prev: string;
  /** Last scale written (skip unchanged frames). */
  fx: number;
  fy: number;
}

const RADII = ['border-top-left-radius', 'border-top-right-radius', 'border-bottom-right-radius', 'border-bottom-left-radius'];

function readRadius(el: Element): number[] | null {
  if (typeof getComputedStyle !== 'function') return null;
  const cs = getComputedStyle(el);
  const out: number[] = [];
  let any = false;
  for (const k of RADII) {
    // "12px" or "12px 8px" (horizontal, vertical); percentages are left alone.
    const v = cs.getPropertyValue(k);
    if (!v || v.includes('%')) return null;
    const p = v.split(/\s+/).map(parseFloat);
    const h = p[0] || 0;
    const vv = p.length > 1 ? p[1] || 0 : h;
    if (h || vv) any = true;
    out.push(h, vv);
  }
  return any ? out : null;
}

function fromState(state: FlipState, options: FlipOptions = {}): Timeline {
  const {
    scale = true, fade, enter, leave, onEnter, onLeave, targets, id: idFn, crossfade = true, correctRadius = false,
    ...anim
  } = options;
  const id = idFn ?? defaultId;
  const before = new Map<Element, number>();
  state.els.forEach((el, i) => before.set(el, i));

  // Elements after the change.
  const afterSet = new Set<Element>(state.els);
  const again = targets ?? state.targets;
  if (again !== undefined) for (const el of toTargets(again)) if (isElement(el)) afterSet.add(el);
  const after = Array.from(afterSet);

  // Elements still animating out from an earlier change: stop that exit and
  // let the new layout decide. Ones we had put back go again, unless the
  // change re-inserted them.
  const inserted = insertedSince(state);
  const carried = new Map<Element, string>();
  for (const el of after) {
    const c = cancelLeave(el);
    if (!c) continue;
    if (c.removed && !inserted.has(el)) el.remove();
    carried.set(el, c.opacity);
  }

  // --- one write pass, one read pass ---------------------------------------
  const html = after.filter((e) => e.isConnected) as HTMLElement[];
  const prevTransforms = html.map((e) => e.style.transform);
  for (const e of html) e.style.transform = 'none';
  const last = html.map((e) => e.getBoundingClientRect());
  const vis = html.map((e, i) => shown(e, last[i]));
  const radiusOf = scale && correctRadius ? html.map((e, i) => (vis[i] ? readRadius(e) : null)) : null;
  for (let i = 0; i < html.length; i++) html[i].style.transform = prevTransforms[i];
  const visibleAfter: Element[] = [];
  const lastOf = new Map<Element, DOMRect>();
  const radiusAt = new Map<Element, number[]>();
  html.forEach((e, i) => {
    if (!vis[i]) return;
    visibleAfter.push(e);
    lastOf.set(e, last[i]);
    const r = radiusOf?.[i];
    if (r) radiusAt.set(e, r);
  });

  // The page scrolled since the capture (flip.capture, then flip.from later):
  // the old boxes are where they were on the page, not in the viewport.
  // Fixed-position elements stay where they were on screen.
  let rects = state.rects;
  const sdx = typeof window !== 'undefined' && state.scrollX !== undefined ? state.scrollX - window.scrollX : 0;
  const sdy = typeof window !== 'undefined' && state.scrollY !== undefined ? state.scrollY - window.scrollY : 0;
  if (sdx || sdy) {
    rects = state.rects.map((r, i) =>
      isFixed(state.els[i]) ? r : new DOMRect(r.left + sdx, r.top + sdy, r.width, r.height));
  }

  // Shared ids: the visible "before" element of each id.
  const byId = new Map<string, number>();
  state.ids.forEach((v, i) => {
    if (v !== null && state.visible[i]) byId.set(v, i);
  });
  const visibleSet = new Set(visibleAfter);

  /** `from` → `to` are visual boxes; `box` is the element's untransformed layout box. */
  type Move = { el: Element; from: DOMRect; to: DOMRect; box: DOMRect; fade?: [number, number]; outgoing?: boolean };
  const moves: Move[] = [];
  const entering: Element[] = [];
  const consumed = new Set<number>();
  for (const el of visibleAfter) {
    const box = lastOf.get(el)!;
    const bi = before.get(el);
    if (bi !== undefined && state.visible[bi]) {
      consumed.add(bi);
      moves.push({ el, from: rects[bi], to: box, box });
      continue;
    }
    const key = id(el);
    const si = key != null ? byId.get(key) : undefined;
    if (si !== undefined && state.els[si] !== el && !consumed.has(si)) {
      // Shared element: start from the twin's old box.
      consumed.add(si);
      const other = state.els[si];
      const both = crossfade && visibleSet.has(other);
      moves.push({ el, from: rects[si], to: box, box, fade: both ? [0, 1] : undefined });
      if (both) {
        // The outgoing twin travels to the new box while fading out.
        moves.push({ el: other, from: rects[si], to: box, box: lastOf.get(other)!, fade: [1, 0], outgoing: true });
      }
      continue;
    }
    entering.push(el);
  }

  const leavingIdx: number[] = [];
  state.els.forEach((el, i) => {
    if (state.visible[i] && !consumed.has(i) && !visibleSet.has(el)) leavingIdx.push(i);
  });

  // --- animations -----------------------------------------------------------
  const tl = new Timeline({ autoplay: false });
  // Elements are grouped by the set of properties they animate, so nothing
  // claims (and stops) a property it doesn't actually change.
  const groups = new Map<string, { els: Element[]; kf: Array<Record<string, [number, number]>> }>();
  const radii: Radius[] = [];
  const outgoing: Array<[Element, { x: number; y: number; sx: number; sy: number }]> = [];
  const cx = (r: DOMRect) => r.left + r.width / 2;
  const cy = (r: DOMRect) => r.top + r.height / 2;
  // Nested moves: an element inside another moving element is carried by its
  // ancestor's inverted transform at the start. Its own start transform
  // undoes that, so it starts at its old box on screen. Each start is an
  // axis-aligned map q -> a + b·q (viewport coordinates of the new layout).
  const starts = new Map<Element, StartMap>();
  const startOf = (el: Element): StartMap | null => {
    for (let p = el.parentElement; p; p = p.parentElement) {
      const m = starts.get(p);
      if (m) return m;
    }
    return null;
  };
  for (const m of moves) {
    const { el, from, to, box } = m;
    const s = stateOf(el);
    const home = homes.get(el) ?? { x: s.t[0], y: s.t[1], sx: s.t[8], sy: s.t[9] };
    const settled = to === box;
    const P = starts.size && !m.outgoing ? startOf(el) : null;
    const bx = P ? P.bx : 1;
    const by = P ? P.by : 1;
    const sx0 = scale && box.width ? from.width / (box.width * bx) : 1;
    const sy0 = scale && box.height ? from.height / (box.height * by) : 1;
    const x0 = P ? (cx(from) - P.ax) / bx - cx(box) : cx(from) - cx(box);
    const y0 = P ? (cy(from) - P.ay) / by - cy(box) : cy(from) - cy(box);
    if (!m.outgoing && settled) {
      // This element's start map, for its descendants.
      const ox = cx(box), oy = cy(box);
      const ax0 = P ? P.ax : 0, ay0 = P ? P.ay : 0;
      starts.set(el, {
        ax: ax0 + bx * (ox + x0 - sx0 * ox), bx: bx * sx0,
        ay: ay0 + by * (oy + y0 - sy0 * oy), by: by * sy0,
      });
    }
    const x1 = settled ? home.x : cx(to) - cx(box);
    const y1 = settled ? home.y : cy(to) - cy(box);
    const sx1 = settled ? home.sx : scale && box.width ? to.width / box.width : 1;
    const sy1 = settled ? home.sy : scale && box.height ? to.height / box.height : 1;
    const re = carried.get(el);
    if (!m.fade && re === undefined && Math.abs(x0 - x1) < 0.5 && Math.abs(y0 - y1) < 0.5 && Math.abs(sx0 - sx1) < 1e-3 && Math.abs(sy0 - sy1) < 1e-3) {
      continue;
    }
    const k: Record<string, [number, number]> = { x: [x0, x1], y: [y0, y1] };
    if (scale) {
      k.scaleX = [sx0, sx1];
      k.scaleY = [sy0, sy1];
    }
    if (m.fade) k.opacity = m.fade;
    else if (re !== undefined) {
      const cur = parseFloat(re);
      const target = opacityOf(el, true);
      if (Number.isFinite(cur) && cur !== target) k.opacity = [cur, target];
    }
    if (m.outgoing) outgoing.push([el, home]);
    else homes.set(el, home);
    if (scale && correctRadius && (Math.abs(sx0 - sx1) > 1e-3 || Math.abs(sy0 - sy1) > 1e-3)) {
      const r = radiusAt.get(el);
      if (r) {
        const same = r.every((v) => v === r[0]);
        radii.push({ el: el as HTMLElement, t: s.t, r, same, prev: (el as HTMLElement).style.borderRadius, fx: NaN, fy: NaN });
      }
    }
    const sig = Object.keys(k).join();
    let g = groups.get(sig);
    if (!g) groups.set(sig, (g = { els: [], kf: [] }));
    g.els.push(el);
    g.kf.push(k);
  }

  const fix = radii.length
    ? () => {
        for (let i = 0; i < radii.length; i++) {
          const r = radii[i];
          const t = r.el === undefined ? null : stateOf(r.el).t;
          if (t === null) continue;
          const fx = Math.abs(t[8]) > 1e-6 ? 1 / Math.abs(t[8]) : 1;
          const fy = Math.abs(t[9]) > 1e-6 ? 1 / Math.abs(t[9]) : 1;
          if (fx === r.fx && fy === r.fy) continue;
          r.fx = fx;
          r.fy = fy;
          const v = r.r;
          r.el.style.borderRadius = r.same
            ? (fx === fy ? s3(v[0] * fx) + 'px' : s3(v[0] * fx) + 'px / ' + s3(v[0] * fy) + 'px')
            : s3(v[0] * fx) + 'px ' + s3(v[2] * fx) + 'px ' + s3(v[4] * fx) + 'px ' + s3(v[6] * fx) + 'px / ' +
              s3(v[1] * fy) + 'px ' + s3(v[3] * fy) + 'px ' + s3(v[5] * fy) + 'px ' + s3(v[7] * fy) + 'px';
        }
      }
    : null;
  let first = true;
  for (const g of groups.values()) {
    const kf = g.kf;
    const keys = Object.keys(kf[0]);
    // Function values are resolved per target at build time.
    const props: Props = {};
    for (const p of keys) {
      props[p] = [((_t: unknown, i: number) => kf[i][p][0]) as any, ((_t: unknown, i: number) => kf[i][p][1]) as any];
    }
    // User callbacks go on the first group; radius correction runs on every group.
    const user: AnimateOptions = first ? anim : {};
    const own = first;
    first = false;
    const moveAnim = new Animation(g.els, props, {
      duration: 0.5, ease: 'outCubic', immediateRender: true, ...anim,
      onStart: user.onStart, onRepeat: user.onRepeat, onReverseComplete: user.onReverseComplete,
      autoplay: false,
      onUpdate: fix || user.onUpdate ? (a) => {
        fix?.();
        user.onUpdate?.(a);
      } : undefined,
      onComplete: (a) => {
        if (own) {
          for (const r of radii) r.el.style.borderRadius = r.prev;
          for (const [el, home] of outgoing) {
            // The outgoing twin stays faded out, back on its own box.
            const t = stateOf(el);
            t.t[0] = home.x;
            t.t[1] = home.y;
            t.t[8] = home.sx;
            t.t[9] = home.sy;
            markDirty(t);
          }
        }
        for (const el of g.els) homes.delete(el);
        user.onComplete?.(a);
      },
    });
    tl.add(moveAnim, 0);
  }
  fix?.();

  const timing: AnimateOptions = {};
  for (const k of ['duration', 'ease', 'delay'] as const) if (anim[k] !== undefined) (timing as any)[k] = anim[k];

  // Entering elements.
  if (entering.length) {
    const p = onEnter ? onEnter(entering) : enter || fade ? new Animation(entering, enter ?? { opacity: [0, (el: Element) => opacityOf(el, true)] }, { duration: 0.4, ease: 'outQuad', immediateRender: true, ...timing, autoplay: false }) : null;
    if (p) tl.add(p, 0);
  }

  // Leaving elements: pop them out at their last box, animate, then let them go.
  if (leavingIdx.length && (leave || fade || onLeave)) {
    const popped: Array<{ el: HTMLElement; place: () => void; finish: (a: Playable | null) => () => void }> = [];
    for (const i of leavingIdx) {
      const el = state.els[i] as HTMLElement;
      let removed = false;
      if (!el.isConnected) {
        const parent = state.parents[i];
        if (!parent || !(parent as Element).isConnected) continue;
        const next = state.nexts[i];
        parent.insertBefore(el, next && next.parentNode === parent ? next : null);
        removed = true;
      }
      const hidden = !removed && !isRendered(el);
      const pop = popOut(el, rects[i], hidden ? state.display[i] || 'block' : undefined);
      const op = carried.get(el);
      if (op !== undefined) el.style.opacity = op;
      const finish = trackLeave(el, pop.restore, removed, () => {
        if (removed) el.remove();
      });
      popped.push({ el, place: pop.place, finish });
    }
    for (const p of popped) p.place();
    const list = popped.map((p) => p.el);
    if (list.length) {
      const p = onLeave
        ? onLeave(list)
        : new Animation(list, leave ?? { opacity: [(el: Element) => opacityOf(el), 0] }, { duration: 0.3, ease: 'inQuad', ...timing, autoplay: false });
      const dones = popped.map((x) => x.finish(p ?? null));
      const done = () => dones.forEach((d) => d());
      if (p) {
        // Children of a timeline don't settle their own promise when they
        // complete: hook completion. A kill (the timeline killed, a context
        // reverted, opacity taken over by another animation) does settle it;
        // either way the element must not be left popped out.
        const cb = p._cb;
        const user = cb.onComplete;
        cb.onComplete = (a: Playable) => {
          done();
          user?.(a);
        };
        p.then(done);
        tl.add(p, 0);
      } else done();
    }
  }

  // Render the inverted state before the next paint.
  tl.seek(0);
  if (anim.autoplay !== false) tl.play();
  flushTransforms();
  return tl;
}

/**
 * Record positions, run `mutate`, then animate from the old layout to the new
 * one. Returns a {@link Timeline} holding the moves and any enter/leave
 * animations (`await flip(...)` waits for all of them).
 */
export function flip(targets: Targets, mutate: () => void, options?: FlipOptions): Timeline {
  if (typeof mutate !== 'function') throw new Error('anikit: flip(targets, mutate, options?) needs a function that changes the layout as its second argument, e.g. flip(\'.item\', () => grid.classList.toggle(\'dense\')). For a captured state use flip.from(state, options).');
  checkKeys('flip()', options, ANIMATE_KEYS, FLIP_KEYS);
  const state = capture(targets, options);
  checkTargets('flip()', targets, state.els.length);
  mutate();
  return fromState(state, options);
}
flip.capture = capture;
flip.from = fromState;

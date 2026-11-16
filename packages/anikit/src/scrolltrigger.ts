/**
 * `scrollTrigger()` — scroll-driven animation: trigger on enter/leave, scrub
 * progress (optionally smoothed), pin elements, snap, toggle classes, and
 * debug markers. One passive listener per scroller drives every trigger;
 * layout is measured only on refresh (load / resize), never per scroll.
 *
 * ```ts
 * scrollTrigger({ trigger: '.hero', start: 'top top', end: '+=600', pin: true, scrub: 0.5,
 *                 animation: timeline().add('.hero h1', { scale: 0.6 }).add('.hero p', { opacity: 0 }) });
 * scrollTrigger({ trigger: '.card', start: 'top 80%', toggleActions: 'play none none reverse',
 *                 animation: animate('.card', { y: [50, 0], opacity: [0, 1] }, { autoplay: false }) });
 * scrollTrigger({ trigger: '#section', toggleClass: 'active', onEnter: () => nav.highlight('section') });
 * ```
 */
import { Playable } from './playable';
import { Targets, toTargets } from './props';
import { spring, PhysicsAnimation } from './spring';
import { animate, AnimateOptions, Animation } from './animate';
import { track } from './context';
import { warn, warningsOn, checkKeys, checkSeconds, checkTargets, didYouMean, selectorHint } from './debug';
import type { OptionKeys } from './debug';
import { Timeline } from './timeline';

type Action = 'play' | 'pause' | 'resume' | 'reset' | 'restart' | 'complete' | 'reverse' | 'none';
type Edge = 'top' | 'center' | 'bottom';
/** `"<trigger edge> <viewport edge>"` (autocompletes the common pairs; offsets such as `'top+=20 80%'` or `'+=600'` work too), px, or a function. */
type Pos = `${Edge} ${Edge}` | `${Edge} ${number}%` | (string & {}) | number | ((self: ScrollTrigger) => number);
/** Four actions for onEnter, onLeave, onEnterBack, onLeaveBack. */
type ToggleActions = `${Action} ${Action} ${Action} ${Action}` | (string & {});

export interface ScrollTriggerOptions {
  /** Element whose position defines start/end (default: the scroller itself, i.e. page progress). */
  trigger?: Element | string;
  /** Scroll container (default: the page). */
  scroller?: Element | Window | string;
  /** Measure along the x axis (horizontal scrollers). Default false. */
  horizontal?: boolean;
  /**
   * `"<trigger edge> <viewport edge>"`, edges: top/center/bottom (left/right
   * when horizontal), `%`, `px` or a number of px. A plain number is an
   * absolute scroll offset; a function may compute it. Default `'top bottom'`.
   */
  start?: Pos;
  /** As `start`, or `'+=500'` relative to the start. Default `'bottom top'`. */
  end?: Pos;
  /**
   * Animation or timeline controlled by this trigger (it is paused for you; creating it with `autoplay: false` avoids a first-frame flash).
   * One per trigger; for one trigger per element use `scrollTriggerBatch()`.
   */
  animation?: Playable;
  /** Link progress to scroll: `true`, or a smoothing lag in seconds. */
  scrub?: boolean | number;
  /**
   * Four actions for `onEnter onLeave onEnterBack onLeaveBack`, each one of
   * play / pause / resume / reverse / restart / reset / complete / none.
   * Default `'play none none none'`. Ignored with `scrub`.
   * @example toggleActions: 'play none none reverse'   // reverse when scrolling back above the start
   */
  toggleActions?: ToggleActions;
  /** Pin an element (default the trigger) while active. */
  pin?: boolean | Element | string;
  /** Add padding so following content waits for the pin (default true). */
  pinSpacing?: boolean;
  /** Snap progress: step (0.25), list of progress points, `'labels'` of the timeline, or a function. */
  snap?: number | number[] | 'labels' | ((progress: number) => number) | { to: number | number[] | 'labels' | ((p: number) => number); duration?: number; ease?: AnimateOptions['ease']; delay?: number };
  /** Toggle a class while active: `'active'` (on the trigger) or `{ targets, className }`. */
  toggleClass?: string | { targets: Targets; className: string };
  /** Show start/end markers (debugging). */
  markers?: boolean | { color?: string };
  /** Run once, then kill the trigger after the first enter. */
  once?: boolean;
  /** Scrolling down past `start`. */
  onEnter?: (self: ScrollTrigger) => void;
  /** Scrolling down past `end`. */
  onLeave?: (self: ScrollTrigger) => void;
  /** Scrolling up past `end` (back into the range). */
  onEnterBack?: (self: ScrollTrigger) => void;
  /** Scrolling up past `start` (out of the range). */
  onLeaveBack?: (self: ScrollTrigger) => void;
  /** `isActive` changed. */
  onToggle?: (self: ScrollTrigger) => void;
  /** Progress changed (`self.progress`, 0–1; `self.direction` 1 or -1). */
  onUpdate?: (self: ScrollTrigger) => void;
  /** Positions were re-measured (load, resize, `refreshScrollTriggers()`). */
  onRefresh?: (self: ScrollTrigger) => void;
}

// ---------------------------------------------------------------------------
// Scroller abstraction & shared listeners
// ---------------------------------------------------------------------------

function resolveScroller(s: ScrollTriggerOptions['scroller']): Element | Window {
  if (s === undefined || s === window) return window;
  if (typeof s === 'string') return document.querySelector(s) ?? window;
  return s as Element;
}
const scrollPos = (s: Element | Window, horizontal: boolean): number =>
  s === window
    ? horizontal ? window.scrollX : window.scrollY
    : horizontal ? (s as Element).scrollLeft : (s as Element).scrollTop;
const viewportSize = (s: Element | Window, horizontal: boolean): number =>
  s === window
    ? horizontal ? window.innerWidth : window.innerHeight
    : horizontal ? (s as Element).clientWidth : (s as Element).clientHeight;
const maxScroll = (s: Element | Window, horizontal: boolean): number => {
  const el = (s === window ? document.scrollingElement || document.documentElement : s) as Element;
  return Math.max(0, horizontal ? el.scrollWidth - viewportSize(s, true) : el.scrollHeight - viewportSize(s, false));
};

const groups = new Map<Element | Window, Set<ScrollTrigger>>();
/** Number of running programmatic scrolls (snapping waits for them). */
let programmatic = 0;
let resizeBound = false;
let refreshQueued = false;

function listen(st: ScrollTrigger): void {
  let set = groups.get(st.scroller);
  if (!set) {
    set = new Set();
    groups.set(st.scroller, set);
    const s = set;
    const sc = st.scroller;
    // Read the position once per event: a read after the previous trigger's
    // style writes would make the browser lay the page out again, per trigger.
    sc.addEventListener('scroll', () => {
      let y = NaN;
      let x = NaN;
      for (const t of s) {
        if (t.horizontal) t.update(x === x ? x : (x = scrollPos(sc, true)));
        else t.update(y === y ? y : (y = scrollPos(sc, false)));
      }
    }, { passive: true });
  }
  set.add(st);
  if (!resizeBound && typeof window !== 'undefined') {
    resizeBound = true;
    window.addEventListener('resize', () => queueRefresh(), { passive: true });
    window.addEventListener('load', () => refreshAll());
  }
}

function queueRefresh(): void {
  if (refreshQueued) return;
  refreshQueued = true;
  setTimeout(() => {
    refreshQueued = false;
    refreshAll();
  }, 120);
}

/** Recalculate every trigger's start/end (call after layout changes). */
export function refreshAll(): void {
  for (const set of groups.values()) for (const t of set) t.refresh();
}

// ---------------------------------------------------------------------------
// Position parsing
// ---------------------------------------------------------------------------

const EDGE: Record<string, number> = { top: 0, left: 0, center: 0.5, bottom: 1, right: 1 };

function edgeOffset(token: string, size: number): number {
  const t = token.trim();
  if (t in EDGE) return EDGE[t] * size;
  if (t.endsWith('%')) return (parseFloat(t) / 100) * size;
  if (t.endsWith('px')) return parseFloat(t);
  const n = parseFloat(t);
  return Number.isFinite(n) ? n : 0;
}

/** Split "top+=20 center-=10%" into [["top", "+20"], ["center", "-10%"]]. */
function parseEdge(tok: string, size: number): number {
  const m = /^([a-z]+|-?[\d.]+(?:px|%)?)(?:([+-])=?([\d.]+(?:px|%)?))?$/i.exec(tok.trim());
  if (!m) return edgeOffset(tok, size);
  let v = edgeOffset(m[1], size);
  if (m[2]) v += (m[2] === '-' ? -1 : 1) * edgeOffset(m[3], size);
  return v;
}

const SCROLLTRIGGER_KEYS: OptionKeys<ScrollTriggerOptions> = {
  trigger: 0, scroller: 0, horizontal: 0, start: 0, end: 0, animation: 0, scrub: 0, toggleActions: 0, pin: 0,
  pinSpacing: 0, snap: 0, toggleClass: 0, markers: 0, once: 0, onEnter: 0, onLeave: 0, onEnterBack: 0,
  onLeaveBack: 0, onToggle: 0, onUpdate: 0, onRefresh: 0,
};
const ACTIONS = ['play', 'pause', 'resume', 'reverse', 'restart', 'reset', 'complete', 'none'];
const POS_TOKEN = /^(?:[+-]=)?(?:top|center|bottom|left|right|-?[\d.]+(?:px|%)?)(?:[+-]=?[\d.]+(?:px|%)?)?$/i;

/** Construction-time checks with actionable messages (warnings on only). */
function checkScrollTrigger(o: ScrollTriggerOptions, api = 'scrollTrigger()'): void {
  checkKeys(api, o, SCROLLTRIGGER_KEYS);
  for (const name of ['start', 'end'] as const) {
    const v = o[name];
    if (typeof v !== 'string') continue;
    for (const tok of v.trim().split(/\s+/)) {
      if (!POS_TOKEN.test(tok)) {
        warn(`${api}: "${tok}" in ${name}: "${v}" isn't understood. Write "<trigger edge> <viewport edge>" with top / center / bottom (left / right when horizontal), px or %, e.g. "top 80%", "center center", "bottom+=100 top", or "+=600" for an end relative to the start.${didYouMean(tok.replace(/[+-]=?.*$/, ''), Object.keys(EDGE))}`);
      }
    }
  }
  if (o.toggleActions !== undefined) {
    const parts = o.toggleActions.trim().split(/\s+/);
    const bad = parts.filter((p) => !ACTIONS.includes(p));
    if (parts.length > 4 || bad.length) {
      warn(`${api}: toggleActions "${o.toggleActions}" should be four of ${ACTIONS.join(' / ')} (for onEnter, onLeave, onEnterBack, onLeaveBack), e.g. "play none none reverse".${bad[0] ? didYouMean(bad[0], ACTIONS) : ''}`);
    }
    if (o.scrub) warn(`${api}: toggleActions is ignored when scrub is set (the scroll position drives the animation).`);
  }
  if (typeof o.scrub === 'number') checkSeconds(api, 'scrub', o.scrub, 10);
}

// ---------------------------------------------------------------------------
// ScrollTrigger
// ---------------------------------------------------------------------------

export class ScrollTrigger {
  readonly scroller: Element | Window;
  readonly trigger: Element | null;
  readonly horizontal: boolean;
  /** Scroll offsets (px) where the trigger starts / ends. */
  start = 0;
  end = 0;
  /** 0–1 position between start and end. */
  progress = 0;
  /** 1 scrolling forward, -1 backward. */
  direction = 1;
  isActive = false;
  private last = -1;
  /** Progress last given to a scrubbed animation (NaN: none yet / after a refresh). */
  private scrubbed = NaN;
  private o: ScrollTriggerOptions;
  private smoother: PhysicsAnimation | null = null;
  private proxy = { p: 0 };
  private pinEl: HTMLElement | null = null;
  private spacer: HTMLElement | null = null;
  private pinRestore: (() => void) | null = null;
  private markersEls: HTMLElement[] = [];
  private snapTimer: ReturnType<typeof setTimeout> | null = null;
  private snapping: Animation | null = null;
  private enabled = true;
  private dead = false;
  private pinTop = 0;
  private pinMargin = '';
  private pinItem: Record<string, string> = {};
  private pinSize = { w: 0, h: 0 };
  /** The pinned element's inline values of the properties pinning writes. */
  private pinOrig = { position: '', top: '', left: '', width: '', height: '', transform: '', margin: '', boxSizing: '' };

  /** Size the spacer like the pinned element's natural box. */
  private sizeSpacer(): void {
    const sp = this.spacer;
    const el = this.pinEl;
    if (!sp || !el) return;
    const st = sp.style;
    // Measure the element's own box: its inline size and margin as the page set them.
    const o = this.pinOrig;
    el.style.margin = o.margin;
    el.style.boxSizing = o.boxSizing;
    el.style.width = o.width;
    el.style.height = o.height;
    st.display = 'contents';
    // The layout box: without the element's own transform (a rotation would
    // grow the rect, and the pinned box with it).
    const tf = el.style.transform;
    el.style.transform = 'none';
    const r = el.getBoundingClientRect();
    el.style.transform = tf;
    this.pinSize = { w: r.width, h: r.height };
    st.display = 'block';
    st.boxSizing = 'content-box';
    st.width = r.width + 'px';
    st.height = r.height + 'px';
    st.margin = this.pinMargin;
    Object.assign(st, this.pinItem);
    el.style.margin = '0';
    el.style.boxSizing = 'border-box';
    el.style.width = r.width + 'px';
    el.style.height = r.height + 'px';
  }
  private smoothOpts: Parameters<typeof spring>[2];

  constructor(options: ScrollTriggerOptions) {
    if (warningsOn()) checkScrollTrigger(options);
    if (typeof options.animation === 'function') {
      throw new Error('anikit: scrollTrigger({ animation }) takes an animation or timeline (created with autoplay: false), not a function. For one trigger per element with a factory, use scrollTriggerBatch(targets, { animation: (el) => … }).');
    }
    this.o = options;
    this.scroller = resolveScroller(options.scroller);
    this.horizontal = !!options.horizontal;
    const tr = options.trigger;
    this.trigger = typeof tr === 'string' ? document.querySelector(tr) : tr ?? null;
    if (typeof tr === 'string' && !this.trigger) warn(`scrollTrigger: trigger "${tr}" matched no element; using the whole scroll range.${selectorHint(tr)}`);
    else if (tr === null) warn('scrollTrigger: trigger is null (a querySelector() or ref that found nothing?); using the whole scroll range.');
    if (options.pin === true && !this.trigger) warn('scrollTrigger: pin: true pins the trigger element, but there is no trigger; nothing is pinned. Pass trigger, or pin: element.');
    options.animation?.pause();
    this.smoothOpts = {
      duration: (typeof options.scrub === 'number' ? options.scrub : 0) * 4, bounce: 0, restDelta: 1e-4, restSpeed: 1e-3,
      onUpdate: () => this.applySmooth(),
    };
    if (options.pin) this.setupPin();
    listen(this);
    this.refresh();
    track(this);
  }

  /** Controlled animation. */
  get animation(): Playable | undefined {
    return this.o.animation;
  }

  private setupPin(): void {
    const p = this.o.pin;
    const el = (p === true ? this.trigger : typeof p === 'string' ? document.querySelector(p) : p) as HTMLElement | null;
    if (!el || !el.parentNode) return;
    this.pinEl = el;
    const spacer = document.createElement('div');
    spacer.className = 'anikit-pin-spacer';
    el.parentNode.insertBefore(spacer, el);
    spacer.appendChild(el);
    this.spacer = spacer;
    // The spacer takes over the element's box in the layout.
    const cs = getComputedStyle(el);
    this.pinMargin = cs.margin;
    this.pinItem = { flex: cs.flex, alignSelf: cs.alignSelf, justifySelf: cs.justifySelf, gridArea: cs.gridArea, order: cs.order };
    const prev = el.getAttribute('style');
    const o = this.pinOrig;
    for (const k of Object.keys(o) as (keyof typeof o)[]) o[k] = el.style[k];
    this.pinRestore = () => {
      if (prev === null) el.removeAttribute('style');
      else el.setAttribute('style', prev);
      spacer.parentNode?.insertBefore(el, spacer);
      spacer.remove();
    };
  }

  /** Recompute start/end from the current layout. */
  refresh(): void {
    if (this.dead) return;
    const h = this.horizontal;
    const size = viewportSize(this.scroller, h);
    // Measure with the pin un-pinned.
    if (this.pinEl) {
      const st = this.pinEl.style;
      const o = this.pinOrig;
      st.position = o.position;
      st.top = o.top;
      st.left = o.left;
      // The counter-translation inside a scrolling element is ours; in the
      // window the transform stays the page's (it may be animating it).
      if (this.scroller !== window) st.transform = o.transform;
      if (this.spacer) this.spacer.style.paddingBottom = this.spacer.style.paddingRight = '';
      this.sizeSpacer();
    }
    // Read after un-pinning: removing the spacing can make the browser's
    // scroll anchoring move the scroll position, and every rect below is
    // read in that same layout.
    const scroll = scrollPos(this.scroller, h);
    let tStart = 0;
    let tSize = 0;
    if (this.trigger) {
      const r = this.trigger.getBoundingClientRect();
      const base = this.scroller === window ? 0 : h ? (this.scroller as Element).getBoundingClientRect().left : (this.scroller as Element).getBoundingClientRect().top;
      tStart = (h ? r.left : r.top) - base + scroll;
      tSize = h ? r.width : r.height;
    }
    const resolvePos = (pos: Pos | undefined, def: string, rel: number): number => {
      const v = pos ?? def;
      if (typeof v === 'function') return v(this);
      if (typeof v === 'number') return v;
      const s = v.trim();
      if (/^[+-]=/.test(s)) return rel + (s[0] === '-' ? -1 : 1) * edgeOffset(s.slice(2), size);
      if (!this.trigger) {
        // No trigger: positions relative to the whole scroll range.
        const parts = s.split(/\s+/);
        return parseEdge(parts[0], maxScroll(this.scroller, h));
      }
      const [te, ve = 'top'] = s.split(/\s+/);
      return tStart + parseEdge(te, tSize) - parseEdge(ve, size);
    };
    this.start = resolvePos(this.o.start, this.trigger ? 'top bottom' : 'top', 0);
    this.end = resolvePos(this.o.end, this.trigger ? 'bottom top' : 'bottom', this.start);
    if (this.end < this.start) this.end = this.start;
    if (this.pinEl && this.spacer) {
      const r = this.pinEl.getBoundingClientRect();
      this.pinTop = (h ? r.left : r.top) - (this.scroller === window ? 0 : (h ? (this.scroller as Element).getBoundingClientRect().left : (this.scroller as Element).getBoundingClientRect().top)) + scroll - this.start;
      if (this.o.pinSpacing !== false) {
        if (h) this.spacer.style.paddingRight = this.end - this.start + 'px';
        else this.spacer.style.paddingBottom = this.end - this.start + 'px';
      }
    }
    this.drawMarkers();
    this.last = -1;
    this.scrubbed = NaN;
    this.o.onRefresh?.(this);
    this.update();
  }

  /** Apply the current scroll position (the shared listener passes the position it read once for all triggers). */
  update(pos?: number): void {
    if (!this.enabled || this.dead) return;
    const s = pos ?? scrollPos(this.scroller, this.horizontal);
    if (s === this.last) return;
    this.direction = s >= this.last ? 1 : -1;
    const span = this.end - this.start;
    const p = span > 0 ? Math.min(1, Math.max(0, (s - this.start) / span)) : s >= this.start ? 1 : 0;
    const wasActive = this.isActive;
    const firstUpdate = this.last < 0;
    this.isActive = s >= this.start && s <= this.end && span >= 0;
    const prevP = this.progress;
    this.progress = p;
    this.last = s;
    this.applyPin(s);
    const o = this.o;
    const anim = o.animation;
    if (anim && o.scrub) {
      if (typeof o.scrub === 'number' && o.scrub > 0 && !firstUpdate) {
        // Outside the range every scroll event clamps to the same 0 or 1: the
        // spring is already heading there, retargeting it again is wasted.
        if (p !== this.scrubbed) {
          this.scrubbed = p;
          this.smoother = spring(this.proxy, { p }, this.smoothOpts);
        }
      } else if (p !== this.scrubbed || firstUpdate) {
        // Off-screen triggers keep reading 0 or 1 while the page scrolls:
        // seeking the animation to the same progress again is a no-op.
        this.scrubbed = p;
        this.proxy.p = p;
        anim.progress = p;
      }
    }
    if (wasActive !== this.isActive || (firstUpdate && this.isActive) || (firstUpdate && p === 1)) {
      // Which transition happened?
      let kind: 0 | 1 | 2 | 3 | -1 = -1; // enter, leave, enterBack, leaveBack
      if (this.isActive) kind = firstUpdate || this.direction > 0 || prevP <= 0 ? 0 : 2;
      else if (p >= 1) kind = 1;
      else if (!firstUpdate) kind = 3;
      if (kind >= 0) this.fire(kind as 0 | 1 | 2 | 3);
    }
    o.onUpdate?.(this);
    if (o.snap) {
      if (this.isActive) this.scheduleSnap();
      else if (this.snapTimer) {
        // Scrolled out of range before the snap fired: don't pull the page back.
        clearTimeout(this.snapTimer);
        this.snapTimer = null;
      }
    }
  }

  private applySmooth(): void {
    const a = this.o.animation;
    if (a) a.progress = this.proxy.p;
  }

  private fire(kind: 0 | 1 | 2 | 3): void {
    const o = this.o;
    const cbs = [o.onEnter, o.onLeave, o.onEnterBack, o.onLeaveBack];
    cbs[kind]?.(this);
    o.onToggle?.(this);
    if (o.toggleClass) {
      const tc = o.toggleClass;
      const cls = typeof tc === 'string' ? tc : tc.className;
      const els = typeof tc === 'string' ? (this.trigger ? [this.trigger] : []) : (toTargets(tc.targets) as Element[]);
      for (const el of els) el.classList.toggle(cls, this.isActive);
    }
    const anim = o.animation;
    if (anim && !o.scrub) {
      const actions = (o.toggleActions ?? 'play none none none').split(/\s+/) as Action[];
      const act = actions[kind] ?? 'none';
      switch (act) {
        case 'play': anim.speed = Math.abs(anim.speed); anim.play(); break;
        case 'pause': anim.pause(); break;
        case 'resume': anim.play(); break;
        case 'reset': anim.cancel(); break;
        case 'restart': anim.restart(); break;
        case 'complete': anim.complete(); break;
        case 'reverse': anim.speed = -Math.abs(anim.speed); anim.play(); break;
      }
    }
    if (o.once && kind === 0) {
      this.kill(false);
    }
  }

  private applyPin(s: number): void {
    const el = this.pinEl;
    if (!el) return;
    const st = el.style;
    const h = this.horizontal;
    if (this.scroller === window) {
      if (s < this.start) {
        const o = this.pinOrig;
        st.position = o.position;
        st.top = o.top;
        st.left = o.left;
        st.width = this.pinSize.w + 'px';
      } else if (s <= this.end) {
        if (st.position !== 'fixed') {
          const r = this.spacer!.getBoundingClientRect();
          st.width = this.pinSize.w + 'px';
          st.height = this.pinSize.h + 'px';
          st.left = h ? '' : r.left + 'px';
          if (h) st.top = r.top + 'px';
          st.position = 'fixed';
        }
        if (h) st.left = this.pinTop + 'px';
        else st.top = this.pinTop + 'px';
      } else {
        st.position = 'relative';
        st.left = '';
        if (!h) st.top = '';
        if (h) st.left = this.end - this.start + 'px';
        else st.top = this.end - this.start + 'px';
      }
    } else {
      // Inside a scrolling element: counter-translate.
      const d = Math.min(Math.max(s - this.start, 0), this.end - this.start);
      st.transform = h ? `translateX(${d}px)` : `translateY(${d}px)`;
    }
  }

  private scheduleSnap(): void {
    if (this.snapTimer) clearTimeout(this.snapTimer);
    if (programmatic > 0) return;
    this.snapTimer = setTimeout(() => this.doSnap(), 150);
  }

  private doSnap(): void {
    this.snapTimer = null;
    // Only snap while the scroll position is inside the trigger's range.
    const pos0 = scrollPos(this.scroller, this.horizontal);
    if (pos0 < this.start || pos0 > this.end) return;
    const raw = this.o.snap!;
    const cfg = typeof raw === 'object' && !Array.isArray(raw) ? raw : { to: raw };
    const p = this.progress;
    let target: number;
    const to = cfg.to;
    if (typeof to === 'function') target = to(p);
    else if (to === 'labels') {
      const tl = this.o.animation as Timeline | undefined;
      const d = tl?.duration || 1;
      const pts = tl && tl.labels ? Array.from(tl.labels.values()).map((t) => t / d) : [0, 1];
      target = pts.reduce((a, b) => (Math.abs(b - p) < Math.abs(a - p) ? b : a), pts[0] ?? 0);
    } else if (Array.isArray(to)) target = to.reduce((a, b) => (Math.abs(b - p) < Math.abs(a - p) ? b : a));
    else target = Math.round(p / to) * to;
    const pos = this.start + target * (this.end - this.start);
    if (programmatic > 0 || Math.abs(pos - scrollPos(this.scroller, this.horizontal)) < 1) return;
    this.snapping?.kill();
    this.snapping = scrollTo(pos, { container: this.scroller, horizontal: this.horizontal, duration: cfg.duration ?? 0.5, ease: cfg.ease ?? 'inOutCubic' });
  }

  private drawMarkers(): void {
    for (const m of this.markersEls.splice(0)) m.remove();
    if (!this.o.markers || typeof document === 'undefined') return;
    const color = typeof this.o.markers === 'object' ? this.o.markers.color ?? '#e33' : '#e33';
    const mk = (label: string, pos: number, fixed: boolean, viewportOffset = 0) => {
      const d = document.createElement('div');
      d.textContent = label;
      d.style.cssText = `position:${fixed ? 'fixed' : 'absolute'};${this.horizontal ? 'top:0;left' : 'right:0;top'}:${pos}px;` +
        `font:11px monospace;color:${color};border-${this.horizontal ? 'left' : 'top'}:1px solid ${color};padding:2px 4px;z-index:9999;pointer-events:none`;
      void viewportOffset;
      (this.scroller === window ? document.body : (this.scroller as Element)).appendChild(d);
      this.markersEls.push(d);
    };
    const size = viewportSize(this.scroller, this.horizontal);
    const parse = (v: Pos | undefined, def: string) => (typeof v === 'string' ? parseEdge(v.split(/\s+/)[1] ?? 'top', size) : typeof v === 'number' ? 0 : parseEdge(def, size));
    mk('start', this.start + parse(this.o.start, 'bottom'), false);
    mk('end', this.end + parse(this.o.end, 'top'), false);
    mk('scroller-start', parse(this.o.start, 'bottom'), true);
    mk('scroller-end', parse(this.o.end, 'top'), true);
  }

  enable(): void {
    this.enabled = true;
    this.refresh();
  }
  disable(): void {
    this.enabled = false;
  }
  /** Remove the trigger (and optionally revert its animation and pin). */
  kill(revert = false): void {
    if (this.dead) return;
    this.dead = true;
    groups.get(this.scroller)?.delete(this);
    this.smoother?.stop();
    this.snapping?.kill();
    if (this.snapTimer) clearTimeout(this.snapTimer);
    for (const m of this.markersEls.splice(0)) m.remove();
    this.pinRestore?.();
    if (revert) this.o.animation?.revert();
  }
  revert(): void {
    this.kill(true);
  }
}

/** Create a scroll trigger (see {@link ScrollTriggerOptions}). */
export function scrollTrigger(options: ScrollTriggerOptions): ScrollTrigger {
  return new ScrollTrigger(options);
}

/**
 * Create one trigger per element (for lists of cards etc.), each controlling
 * its own animation built by `animation(el, index)`.
 */
export function scrollTriggerBatch(targets: Targets, options: Omit<ScrollTriggerOptions, 'trigger' | 'animation'> & { animation?: (el: Element, i: number) => Playable }): ScrollTrigger[] {
  if (options.animation !== undefined && typeof options.animation !== 'function') {
    throw new Error('anikit: scrollTriggerBatch({ animation }) takes a function (el, index) => animation, called once per element. To control one existing animation, use scrollTrigger({ trigger, animation }).');
  }
  const els = toTargets(targets) as Element[];
  checkTargets('scrollTriggerBatch()', targets, els.length);
  return els.map((el, i) => new ScrollTrigger({ ...options, trigger: el, animation: options.animation?.(el, i) }));
}

// ---------------------------------------------------------------------------
// scrollTo
// ---------------------------------------------------------------------------

export interface ScrollToOptions extends AnimateOptions {
  /** Element (or selector) to scroll. Default: the page. */
  container?: Element | Window | string;
  /** Scroll along x. Default false. */
  horizontal?: boolean;
  /** Pixels to subtract from an element target (e.g. a fixed header). */
  offset?: number;
  /** Stop when the user scrolls (wheel / touch / keys). Default true. */
  autoKill?: boolean;
}

/** Smoothly scroll a container (default: the page) to a position, element or selector. */
export function scrollTo(target: number | Element | string | 'max', options: ScrollToOptions = {}): Animation {
  const container = resolveScroller(options.container);
  const h = !!options.horizontal;
  const el = (container === window ? document.scrollingElement || document.documentElement : container) as Element;
  let pos: number;
  if (typeof target === 'number') pos = target;
  else if (target === 'max') pos = maxScroll(container, h);
  else {
    const t = typeof target === 'string' ? document.querySelector(target) : target;
    if (!t) warn(typeof target === 'string' ? `scrollTo(): "${target}" matched no element; not scrolling.${selectorHint(target)}` : 'scrollTo(): the target element is null; not scrolling.');
    if (!t) pos = scrollPos(container, h);
    else {
      const r = t.getBoundingClientRect();
      const base = container === window ? 0 : h ? el.getBoundingClientRect().left : el.getBoundingClientRect().top;
      pos = (h ? r.left : r.top) - base + scrollPos(container, h);
    }
    pos -= options.offset ?? 0;
  }
  pos = Math.max(0, Math.min(pos, maxScroll(container, h)));
  const { container: _c, horizontal: _h, offset: _o, autoKill, ...anim } = options;
  void _c; void _h; void _o;
  const prop = h ? 'scrollLeft' : 'scrollTop';
  const a = animate(el, { [prop]: [scrollPos(container, h), pos] }, { duration: 0.8, ease: 'inOutCubic', ...anim });
  programmatic++;
  let done = false;
  const release = () => {
    if (!done) {
      done = true;
      programmatic--;
    }
  };
  a.finished.then(release);
  if (autoKill !== false && typeof window !== 'undefined') {
    const stop = () => {
      a.kill();
      off();
    };
    const off = () => {
      window.removeEventListener('wheel', stop);
      window.removeEventListener('touchstart', stop);
      window.removeEventListener('keydown', stop);
    };
    window.addEventListener('wheel', stop, { passive: true });
    window.addEventListener('touchstart', stop, { passive: true });
    window.addEventListener('keydown', stop);
    a.finished.then(off);
  }
  return a;
}

// ---------------------------------------------------------------------------
// smoothScroll
// ---------------------------------------------------------------------------

export interface SmoothScrollOptions {
  /** Lag in seconds (≈ time to catch up). Default 0.6. */
  smooth?: number;
  /** Wheel delta multiplier. Default 1. */
  wheelMultiplier?: number;
  /** Also smooth touch scrolling (default false: native touch momentum is better). */
  touch?: boolean;
  /** Called with the scroll position (px) every frame it moves. */
  onUpdate?: (scroll: number) => void;
}

const SMOOTH_KEYS: OptionKeys<SmoothScrollOptions> = { smooth: 0, wheelMultiplier: 0, touch: 0, onUpdate: 0 };

/**
 * Inertial page scrolling for wheel/trackpad input (Lenis-style): the wheel
 * sets a target that the real scroll position follows with a critically
 * damped spring. Keyboard, scrollbar and anchor jumps keep working.
 */
export function smoothScroll(options: SmoothScrollOptions = {}) {
  checkKeys('smoothScroll()', options, SMOOTH_KEYS);
  if (typeof window === 'undefined') return { stop() {}, start() {}, scrollTo() {}, destroy() {}, revert() {} };
  const el = (document.scrollingElement || document.documentElement) as Element;
  const proxy = { y: window.scrollY };
  let target = window.scrollY;
  let ownWrite = false;
  let running = true;
  let sim: PhysicsAnimation | null = null;
  const lag = Math.max(0.05, options.smooth ?? 0.6);
  const write = () => {
    ownWrite = true;
    el.scrollTop = proxy.y;
    options.onUpdate?.(proxy.y);
  };
  // One options object for every retarget (a wheel event each).
  const sopts = { duration: lag, bounce: 0, restDelta: 0.5, restSpeed: 5, onUpdate: write };
  const go = () => {
    sim = spring(proxy, { y: target }, sopts);
  };
  const onWheel = (e: WheelEvent) => {
    if (!running || e.ctrlKey) return;
    e.preventDefault();
    const scale = e.deltaMode === 1 ? 40 : e.deltaMode === 2 ? window.innerHeight : 1;
    target = Math.max(0, Math.min(target + e.deltaY * scale * (options.wheelMultiplier ?? 1), maxScroll(window, false)));
    go();
  };
  const onScroll = () => {
    if (ownWrite) {
      ownWrite = false;
      return;
    }
    // Native scroll (keys, scrollbar, anchors): resync.
    target = proxy.y = window.scrollY;
    sim?.stop();
  };
  window.addEventListener('wheel', onWheel, { passive: false });
  window.addEventListener('scroll', onScroll, { passive: true });
  if (options.touch) window.addEventListener('touchmove', onScroll, { passive: true });
  const api = {
    stop: () => {
      running = false;
      sim?.stop();
    },
    start: () => {
      running = true;
    },
    /** Smoothly scroll to a pixel offset. */
    scrollTo: (y: number) => {
      target = Math.max(0, Math.min(y, maxScroll(window, false)));
      go();
    },
    destroy: () => {
      sim?.stop();
      window.removeEventListener('wheel', onWheel);
      window.removeEventListener('scroll', onScroll);
      window.removeEventListener('touchmove', onScroll);
    },
    revert: () => api.destroy(),
  };
  track(api);
  return api;
}

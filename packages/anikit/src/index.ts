/**
 * Anikit — hyper-performant web animation.
 *
 * The engine (timing, easing, springs, colours, paths, morphing, timelines)
 * runs in WebAssembly compiled from Rust; this layer maps results onto the
 * DOM with batched, dirty-only writes.
 */
export { animate, from, fromTo, set, Animation } from './animate';
export type { AnimateOptions, Props, PropValue, Keyframe, MotionPathOptions, FnValue, Scalar } from './animate';
export { timeline, Timeline } from './timeline';
export type { TimelineOptions, Position } from './timeline';
export { batch, Batch } from './batch';
export type { BatchOptions, BatchValue } from './batch';
export { stagger, staggerValues } from './stagger';
export type { StaggerOptions, StaggerFn } from './stagger';
export { spring, inertia, PhysicsAnimation } from './spring';
export type { PhysicsOptions, InertiaOptions, InertiaProp } from './spring';
export { scroll, inView } from './scroll';
export type { ScrollOptions, InViewOptions } from './scroll';
export { splitText } from './text';
export type { SplitOptions, SplitResult } from './text';
export { flip } from './flip';
export type { FlipOptions, FlipState } from './flip';
export { compositor, CompositorAnimation, cssEasing } from './compositor';
export { engine, flush } from './core';
export type { EasingInput, EasingFn, SpringOptions } from './core';
export { ready, init } from './wasm';
export {
  morph, morphPlan, shapeToPath, pathData, pathLength, pathBounds, normalizePath, MotionPath,
  circle, ellipse, rect, polygon, star,
} from './svg';
export type { PathSource } from './svg';
export { ease, easing, mixColor, parseColor, clamp, lerp, mapRange, wrap, snap, random } from './utils';
export type { Playable, PlayState } from './playable';

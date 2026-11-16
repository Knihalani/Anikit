/**
 * Tween-only entry (`anikit/tween`): animate, timelines, stagger and springs,
 * with a core module stripped to what they use (`dist/anikit.tween.wasm`,
 * loaded next to this file like the lean build). For physics worlds, 3D,
 * point clouds, video and the rest, import the full package.
 */
export { animate, from, fromTo, set, Animation } from './animate';
export type { AnimateOptions, Props, PropValue, Keyframe, FnValue, Scalar } from './animate';
export { timeline, Timeline, sequence, delayedCall } from './timeline';
export type { TimelineOptions, Position, SequenceStep } from './timeline';
export { stagger, staggerValues } from './stagger';
export type { StaggerOptions, StaggerFn } from './stagger';
export { spring, inertia, PhysicsAnimation } from './spring';
export type { PhysicsOptions, InertiaOptions } from './spring';
export { engine, flush } from './core';
export type { EasingInput, EasingName, EasingFn, SpringOptions } from './core';
export { ready, init } from './wasm';
export { setWarnings, onWarning } from './debug';

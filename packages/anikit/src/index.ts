/**
 * Anikit — hyper-performant web animation.
 *
 * The engine (timing, easing, springs, colours, paths, morphing, timelines)
 * runs in WebAssembly compiled from Rust; this layer maps results onto the
 * DOM with batched, dirty-only writes.
 */
export { animate, from, fromTo, set, Animation } from './animate';
export type { AnimateOptions, Props, PropValue, Keyframe, MotionPathOptions, FnValue, Scalar, TextOptions, TransformProps } from './animate';
export { resolveRandom } from './animate';
export { timeline, Timeline, sequence, delayedCall } from './timeline';
export type { TimelineOptions, Position, SequenceStep } from './timeline';
export { batch, Batch } from './batch';
export type { BatchOptions, BatchValue } from './batch';
export { stagger, staggerValues } from './stagger';
export type { StaggerOptions, StaggerFn } from './stagger';
export { spring, inertia, physics, PhysicsAnimation } from './spring';
export type { PhysicsOptions, InertiaOptions, InertiaProp, PhysicsProp, BallisticOptions } from './spring';
export { scroll, inView } from './scroll';
export type { ScrollOptions, InViewOptions } from './scroll';
export { splitText } from './text';
export type { SplitOptions, SplitResult } from './text';
export { flip } from './flip';
export type { FlipOptions, FlipState } from './flip';
export { compositor, CompositorAnimation, cssEasing } from './compositor';
export { engine, flush } from './core';
export type { EasingInput, EasingName, EasingFn, SpringOptions } from './core';
export { ready, init } from './wasm';
export {
  morph, morphPlan, morphSequence, shapeToPath, pathData, pathLength, pathBounds, normalizePath, MotionPath,
  circle, ellipse, rect, polygon, star, trimPath, pathThrough, simplifyPath, pathArea, flattenPath,
} from './svg';
export type { PathSource } from './svg';
export {
  ease, easing, mixColor, parseColor, clamp, lerp, mapRange, wrap, snap, random, interpolate, normalize, roundTo,
  pipe, shuffle, splitColor, getUnit, toArray, distribute, VelocityTracker,
} from './utils';
export { noise2, noise3, fbm, noiseMany, wander } from './noise';
export { context, matchMedia, Context } from './context';
export type { Revertible } from './context';
export { quickTo, quickSetter } from './quick';
export { motionValue, MotionValue, derived, bindValues, scrollProgress } from './motionvalue';
export type { ValueBinding } from './motionvalue';
export { marquee, Marquee } from './marquee';
export type { MarqueeOptions } from './marquee';
export { magnetic, parallax } from './pointerfx';
export type { MagneticOptions, ParallaxOptions, PointerFx } from './pointerfx';
export { effects, effect, registerEffect, effectNames } from './effects';
export type { EffectOptions, EffectFn, EffectName } from './effects';
export type { Playable, PlayState } from './playable';
export { scrollTrigger, scrollTriggerBatch, ScrollTrigger, scrollTo, smoothScroll, refreshAll as refreshScrollTriggers } from './scrolltrigger';
export type { ScrollTriggerOptions, ScrollToOptions, SmoothScrollOptions } from './scrolltrigger';
export { draggable, Draggable, observer, hover, press } from './interaction';
export type { DraggableOptions, Bounds, ObserverOptions, ObserverEvent } from './interaction';
export { inspector } from './inspector';
export type { InspectorOptions } from './inspector';
export { world, World } from './world';
export type { WorldOptions, BodyInit } from './world';
export { pointCloud, PointCloud, textPoints, glow, Glow } from './cloud';
export { blackHole, BlackHole } from './blackhole';
export type { BlackHoleOptions, BlackHoleFrame, BlackHoleStats } from './blackhole';
export { emitter, Emitter } from './emitter';
export { mesh, Mesh3D, instances, Instances, renderer3d, Renderer3D, texture, Texture3D } from './mesh';
export type { InstanceInit, MaterialOptions, LightOptions, DrawItem, PointsItem } from './mesh';
export type { PostOptions, PostValue, PostColor, ImpactOptions } from './video';
export type { EmitterOptions, SpawnShape } from './emitter';
export { analyzeAudio, decodeAudio } from './audio';
export type { AudioAnalysis } from './audio';
export { composition, Composition, FrameBuffer, typeFx, exportWebM, muxWebM } from './video';
export type { CompositionOptions, SceneHooks, SceneDef, FrameInfo, FrameTiming, RenderOptions, RenderedFrame, TypeFxOptions, Transition, TransitionFn } from './video';
export type { CloudOptions, ShapeInput, GlowOptions, GlowRenderOptions } from './cloud';
export { scene3d, Scene3D, tilt, parseTransform, toMatrix3d, mixTransforms } from './scene';
export { flightPath, FlightPath } from './flight';
export { shatter, Shards } from './shatter';
export type { ShatterOptions } from './shatter';
export type { FlightPathOptions, FlightPose, FlightShake, Point3 } from './flight';
export type { SceneOptions, Projection, DrawOptions, TiltOptions, Vec3, Matrix } from './scene';
export { testing } from './testing';
export { setWarnings, onWarning } from './debug';
export { screenshot, record, rasterize, encodeGif, GifWriter } from './capture';
export type { CaptureTarget, RasterOptions, RecordOptions, Recording } from './capture';
export { presence, Presence, enter, exit } from './presence';
export type { PresenceOptions } from './presence';
export { viewTransition } from './viewtransition';
export type { ViewTransitionOptions, ViewTransitionHandle } from './viewtransition';
export { gesture, Gesture } from './gesture';
export type { GestureOptions } from './gesture';
export { transformPath } from './svg';
export { htmlCanvas, htmlEffect, htmlCanvasSupported, htmlToSvg } from './htmlcanvas';
export type { HtmlCanvas, HtmlCanvasOptions, HtmlEffect, HtmlEffectOptions, HtmlEffectName } from './htmlcanvas';
export {
  scene, Scene, Object3D, InstancedMesh, Material, material, Orbit, Vector3, linearColor,
} from './gl';
export type {
  SceneInit, CameraOptions, BloomOptions, DofOptions, OrbitOptions, Camera, SceneMaterialOptions, MaterialPreset,
  ObjectOptions, InstanceState, EnvironmentName, LayerContext, SceneLayer, RayHit, SceneStats, ScenePointerEvent,
  ScenePointerType, PrimitiveName, Vec3Like,
} from './gl';
export {
  box, roundedBox, sphere, cylinder, capsule, torus, plane, lathe, extrude, tube, triangulate, smoothNormals, subdivide,
  sdfMesh, sdSphere, sdBox, sdTorus, smoothUnion, smoothSubtract,
} from './geometry';
export type { Geometry, SdfOptions } from './geometry';

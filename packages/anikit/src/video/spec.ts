/**
 * Types for the Anikit Video JSON spec (see docs/video/spec.md). Every key is
 * optional except `scenes` and each scene's `layers`.
 */

/** Seconds as a number, or `"1.5s"`, `"500ms"`, `"12f"` (frames), `"@marker"`, `"@marker+0.25"`. */
export type Time = number | string;
/** Pixels as a number, or `"50%"`, `"10vw"`, `"8vh"`, `"5vmin"`, `"5vmax"`, or an `=expression`. */
export type Length = number | string;
/** CSS colour (`"#6d5dfc"`, `"rgb(20 20 30)"`, `"oklch(70% 0.15 250)"`), a theme token (`"$primary"`) or a gradient (`"linear-gradient(135deg, $primary, $accent)"`). */
export type Color = string;
/** Output size preset or `[width, height]`. */
export type Size = '16:9' | '9:16' | '1:1' | '4:5' | '720p' | '1080p' | '1440p' | '4k' | 'vertical' | 'square' | 'portrait' | [number, number];

/** Named easing (`outCubic`, `inOutBack`, …), `cubicBezier(…)`, `steps(n)` or a spring name. */
export type Ease =
  | 'linear' | 'ease' | 'easeIn' | 'easeOut' | 'easeInOut'
  | `${'in' | 'out' | 'inOut'}${'Sine' | 'Quad' | 'Cubic' | 'Quart' | 'Quint' | 'Expo' | 'Circ' | 'Back' | 'Elastic' | 'Bounce'}`
  | SpringName | (string & {});
/** Spring presets. */
export type SpringName = 'gentle' | 'wobbly' | 'stiff' | 'slow' | 'molasses' | 'snappy' | 'bouncy';
/** A spring preset or physical parameters. */
export type Spring = SpringName | { stiffness?: number; damping?: number; mass?: number };

/** Entrance/exit preset names. */
export type PresetName =
  | 'fadeIn' | 'fadeOut' | 'fadeUp' | 'fadeDown' | 'fadeLeft' | 'fadeRight'
  | 'slideUp' | 'slideDown' | 'slideLeft' | 'slideRight'
  | 'popIn' | 'popOut' | 'scaleIn' | 'scaleOut' | 'zoomIn' | 'zoomOut' | 'blurIn' | 'blurOut'
  | 'rotateIn' | 'flipIn' | 'bounceIn' | 'drawOn' | 'drawOff' | 'typewriter' | 'wipeIn' | 'wipeOut' | 'countUp'
  | (string & {});
/** Looping preset names. */
export type LoopName = 'float' | 'pulse' | 'spin' | 'wiggle' | 'breathe' | 'blink' | (string & {});

/** Stagger: seconds per item, or a detailed form. */
export type Stagger = number | { each?: number; total?: number; from?: 'start' | 'center' | 'end' | 'random'; ease?: Ease };

/** A preset with options. */
export interface Preset {
  preset: PresetName;
  delay?: Time;
  duration?: Time;
  ease?: Ease;
  stagger?: Stagger;
  /** Move distance for fade/slide presets. */
  distance?: Length;
  /** Direction for wipes. */
  direction?: 'left' | 'right' | 'up' | 'down';
  /** Characters per second (typewriter). */
  cps?: number;
  [option: string]: unknown;
}
/** A looping motion with options. */
export interface LoopPreset { preset: LoopName; amount?: number; period?: number; [option: string]: unknown }

/** Animatable properties. */
export interface Props {
  x?: Length; y?: Length; width?: Length; height?: Length;
  scale?: number | string; scaleX?: number | string; scaleY?: number | string;
  rotation?: number | string; skewX?: number | string; skewY?: number | string;
  opacity?: number | string; blur?: number | string; radius?: Length;
  fill?: Color; color?: Color; stroke?: Color; strokeWidth?: number;
  trimStart?: number; trimEnd?: number; reveal?: number; value?: number;
  size?: Length; letterSpacing?: Length; progress?: number; zoom?: number;
}

/** One tween in `animate`. */
export interface Tween {
  /** Start, relative to the layer's start; `"+0.2"` = after the previous tween. */
  at?: Time;
  from?: Props;
  to?: Props;
  keyframes?: { [P in keyof Props]?: Array<Props[P] | [number, Props[P]]> };
  duration?: Time;
  ease?: Ease;
  spring?: Spring;
  repeat?: number;
  yoyo?: boolean;
}

/** Shadow options, or `true` for a soft default. */
export type Shadow = boolean | { x?: number; y?: number; blur?: number; color?: Color };
/** Glow options, or a colour. */
export type Glow = Color | { radius?: number; intensity?: number; color?: Color };
/** Stroke options, or a colour. */
export type StrokeSpec = Color | { color?: Color; width?: number; cap?: 'butt' | 'round' | 'square'; join?: 'miter' | 'round' | 'bevel'; dash?: number[] };
/** Named anchor or `[ax, ay]` in 0..1. */
export type Anchor = 'center' | 'top' | 'bottom' | 'left' | 'right' | 'topLeft' | 'topRight' | 'bottomLeft' | 'bottomRight' | [number, number];
/** Blend modes. */
export type Blend = 'normal' | 'add' | 'multiply' | 'screen' | 'overlay' | 'darken' | 'lighten' | 'softLight' | 'difference';

/** Keys every layer accepts. */
export interface LayerBase extends Omit<Props, 'fill' | 'color' | 'stroke' | 'reveal' | 'value' | 'progress' | 'zoom' | 'size' | 'trimStart' | 'trimEnd' | 'strokeWidth' | 'letterSpacing'> {
  id?: string;
  anchor?: Anchor;
  blend?: Blend;
  shadow?: Shadow;
  glow?: Glow;
  clip?: boolean | ShapeLayer;
  mask?: Layer;
  start?: Time;
  end?: Time;
  duration?: Time;
  enter?: PresetName | Preset;
  exit?: PresetName | Preset;
  animate?: Tween[];
  loop?: LoopName | LoopPreset | boolean;
  visible?: boolean;
  /** Delay each child's animations by index × stagger (groups, boxes, split text). */
  stagger?: Stagger;
}

/** Text. */
export interface TextLayer extends LayerBase {
  type?: 'text';
  /** `\n` breaks lines; `**bold**`, `*italic*`, `[coloured]{$accent}` spans; `{value:,}` shows `value`. */
  text: string;
  style?: 'title' | 'heading' | 'subtitle' | 'body' | 'caption' | 'label' | 'code' | (string & {});
  font?: string;
  size?: Length;
  weight?: number | 'thin' | 'light' | 'regular' | 'medium' | 'semibold' | 'bold' | 'extrabold' | 'black';
  italic?: boolean;
  color?: Color;
  align?: 'left' | 'center' | 'right';
  lineHeight?: number;
  letterSpacing?: Length;
  uppercase?: boolean;
  maxLines?: number;
  stroke?: StrokeSpec;
  split?: 'chars' | 'words' | 'lines';
  reveal?: number;
  value?: number;
}

/** Vector shape. */
export interface ShapeLayer extends LayerBase {
  type?: 'shape';
  shape: 'rect' | 'circle' | 'ellipse' | 'line' | 'arrow' | 'polygon' | 'star' | 'path';
  fill?: Color | 'none';
  stroke?: StrokeSpec;
  strokeWidth?: number;
  points?: Array<[number, number]>;
  sides?: number;
  innerRadius?: number;
  d?: string;
  headSize?: number;
  /** Draw part of the outline, 0..1 (animate for draw-on). */
  trimStart?: number;
  trimEnd?: number;
}

/** Image or video clip. */
export interface MediaLayer extends LayerBase {
  type?: 'image' | 'video';
  src: string;
  fit?: 'contain' | 'cover' | 'fill' | 'none';
  trimStart?: Time;
  speed?: number;
  volume?: number;
}

/** Transformable container. */
export interface GroupLayer extends LayerBase {
  type?: 'group';
  layers: Layer[];
}

/** Flexbox styling for `box`. */
export interface BoxStyle {
  direction?: 'row' | 'column';
  gap?: Length;
  padding?: Length | [Length, Length] | [Length, Length, Length, Length];
  align?: 'start' | 'center' | 'end' | 'stretch';
  justify?: 'start' | 'center' | 'end' | 'spaceBetween' | 'spaceAround' | 'spaceEvenly';
  wrap?: boolean;
  grow?: number;
  background?: Color;
  radius?: Length;
  border?: string;
  columns?: number;
}

/** Layout container. */
export interface BoxLayer extends LayerBase {
  type?: 'box';
  style?: BoxStyle;
  children: Layer[];
}

/** High-level component (charts, code, devices, …). */
export interface ComponentLayer extends LayerBase {
  type?: 'component';
  component: string;
  props?: Record<string, unknown>;
}

/** Any layer. */
export type Layer = TextLayer | ShapeLayer | MediaLayer | GroupLayer | BoxLayer | ComponentLayer;

/** Scene transitions. */
export type TransitionType = 'cut' | 'crossfade' | 'fade' | 'fadeBlack' | 'fadeWhite' | 'slide' | 'push' | 'zoom' | 'wipe' | 'iris' | 'blur' | 'glitch' | 'morphCut';
/** A transition name or options. */
export type Transition = TransitionType | {
  type: TransitionType; duration?: Time; ease?: Ease; direction?: 'left' | 'right' | 'up' | 'down';
  color?: Color; softness?: number; origin?: [Length, Length]; amount?: number; intensity?: number;
};

/** Scene camera. */
export interface Camera {
  zoom?: number; x?: number; y?: number; rotation?: number;
  animate?: Tween[];
  shake?: { amount?: number; frequency?: number; at?: Time; duration?: Time };
}

/** An audio cue. */
export interface Audio {
  src: string; at?: Time; volume?: number | string; trimStart?: Time; duration?: Time;
  fadeIn?: number; fadeOut?: number; loop?: boolean; duck?: number; voice?: boolean;
}

/** A scene. */
export interface Scene {
  id?: string;
  duration?: Time | 'auto';
  background?: Color;
  transition?: Transition;
  camera?: Camera;
  markers?: Record<string, Time>;
  layers: Layer[];
  audio?: Audio[];
}

/** A text style. */
export interface TextStyle {
  font?: string; size?: Length; weight?: TextLayer['weight']; color?: Color;
  lineHeight?: number; letterSpacing?: Length; uppercase?: boolean; italic?: boolean;
}

/** Theme: colour tokens, fonts and text styles. */
export interface Theme {
  colors?: { primary?: Color; secondary?: Color; accent?: Color; bg?: Color; surface?: Color; text?: Color; muted?: Color; [token: string]: Color | undefined };
  fonts?: { heading?: string; body?: string; mono?: string };
  radius?: number;
  styles?: Record<string, TextStyle>;
}

/** A caption cue. */
export interface Caption { text: string; at: Time; end: Time; words?: Array<[string, Time, Time]> }

/** A complete video spec. */
export interface VideoSpec {
  size?: Size;
  fps?: number;
  background?: Color;
  theme?: Theme;
  scenes: Scene[];
  transition?: Transition;
  markers?: Record<string, Time>;
  audio?: Audio[];
  captions?: Caption[] | { src: string; style?: string; y?: Length; highlight?: Color };
  fonts?: Record<string, string>;
  duration?: Time;
}

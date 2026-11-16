/**
 * `anikit/video`: build Anikit Video specs with a fluent, fully typed API and
 * render them with the native `anikit-video` renderer (no browser).
 *
 * ```ts
 * import { video } from 'anikit/video';
 *
 * await video({ size: '1080p' })
 *   .scene((s) => s.text('Hello', { style: 'title', animate: 'rise' }))
 *   .render('out.mp4');
 * ```
 *
 * The builder only produces JSON (`toJSON()` / `spec()`), so it runs anywhere.
 * `render`, `still`, `sheet`, `check`, `critique` and `template` call the
 * `anikit-video` CLI and need Node (set `ANIKIT_VIDEO_BIN` to point at the binary).
 */

import type {
  Audio, BoxLayer, BoxStyle, Camera, Caption, Color, ComponentLayer, GroupLayer, Layer, MediaLayer, PresetName,
  Scene, ShapeLayer, Size, TextLayer, Theme, Time, Transition, Tween, VideoSpec,
} from './spec';

export type * from './spec';

/** Friendly aliases accepted wherever a preset name is (`animate: 'rise'`). */
export const PRESET_ALIASES: Readonly<Record<string, PresetName>> = {
  rise: 'fadeUp', fade: 'fadeIn', pop: 'popIn', drop: 'fadeDown', type: 'typewriter', draw: 'drawOn', zoom: 'zoomIn', blur: 'blurIn', slide: 'slideUp', count: 'countUp',
};

/** `animate` accepts tweens, or a preset name as a shorthand for `enter`. */
type AnimateOpt = { animate?: Tween[] | PresetName };
/** Options for {@link LayerList.text}. */
export type TextOptions = Omit<TextLayer, 'text' | 'type' | 'animate'> & AnimateOpt;
/** Options for {@link LayerList.shape}. */
export type ShapeOptions = Omit<ShapeLayer, 'shape' | 'type' | 'animate'> & AnimateOpt;
/** Options for {@link LayerList.image} and {@link LayerList.video}. */
export type MediaOptions = Omit<MediaLayer, 'src' | 'type' | 'animate'> & AnimateOpt;
/** Options for {@link LayerList.box}. */
export type BoxOptions = Omit<BoxLayer, 'children' | 'type' | 'animate'> & AnimateOpt;
/** Options for {@link LayerList.group}. */
export type GroupOptions = Omit<GroupLayer, 'layers' | 'type' | 'animate'> & AnimateOpt;
/** Options for {@link LayerList.component}. */
export type ComponentOptions = Omit<ComponentLayer, 'component' | 'props' | 'type' | 'animate'> & AnimateOpt;
/** Options for {@link Video.scene}. */
export type SceneOptions = Omit<Scene, 'layers'>;

/** Options for {@link video}. */
export interface VideoOptions {
  /** Size preset (`'16:9'`, `'9:16'`, `'1:1'`, `'1080p'`, `'4k'`, …) or `[w, h]`. Default `'16:9'`. */
  size?: Size;
  /** Frames per second. Default 30. */
  fps?: number;
  /** Default background for every scene. */
  background?: Color;
  /** Colours, fonts and text styles. */
  theme?: Theme;
  /** Default transition between scenes. */
  transition?: Transition;
}

function normalize<T extends { animate?: Tween[] | PresetName; enter?: unknown }>(o: T | undefined): Omit<T, 'animate'> & { animate?: Tween[] } {
  const out: Record<string, unknown> = { ...(o ?? {}) };
  if (typeof out.animate === 'string') {
    if (out.enter === undefined) out.enter = PRESET_ALIASES[out.animate] ?? out.animate;
    delete out.animate;
  }
  if (typeof out.enter === 'string') out.enter = PRESET_ALIASES[out.enter] ?? out.enter;
  if (typeof out.exit === 'string') out.exit = PRESET_ALIASES[out.exit] ?? out.exit;
  return out as Omit<T, 'animate'> & { animate?: Tween[] };
}

/** Adds layers; shared by scenes, boxes and groups. */
export class LayerList {
  /** The layers added so far, bottom to top. */
  readonly layers: Layer[] = [];

  /** Add a text layer. */
  text(text: string, o?: TextOptions): this {
    this.layers.push({ text, ...normalize(o) } as TextLayer);
    return this;
  }
  /** Add a vector shape (`'rect'`, `'circle'`, `'line'`, `'arrow'`, `'polygon'`, `'star'`, `'path'`). */
  shape(shape: ShapeLayer['shape'], o?: ShapeOptions): this {
    this.layers.push({ type: 'shape', shape, ...normalize(o) } as ShapeLayer);
    return this;
  }
  /** Add an image (PNG, JPEG, WebP, GIF, SVG). */
  image(src: string, o?: MediaOptions): this {
    this.layers.push({ type: 'image', src, ...normalize(o) } as MediaLayer);
    return this;
  }
  /** Add a video clip (MP4, MOV, WebM). */
  video(src: string, o?: MediaOptions): this {
    this.layers.push({ type: 'video', src, ...normalize(o) } as MediaLayer);
    return this;
  }
  /** Add a flexbox container; `build` adds its children. Pass layer options (`{ style: {...}, width }`) or just a style (`{ direction: 'row', gap: 24 }`). */
  box(o: BoxOptions | BoxStyle | undefined, build: (b: LayerList) => unknown): this {
    const kids = new LayerList();
    build(kids);
    const opts: BoxOptions = isBoxStyle(o) ? { style: o as BoxStyle } : (o as BoxOptions) ?? {};
    this.layers.push({ type: 'box', ...normalize(opts), children: kids.layers } as BoxLayer);
    return this;
  }
  /** Add a transformable group; `build` adds its layers. */
  group(o: GroupOptions | undefined, build: (g: LayerList) => unknown): this {
    const kids = new LayerList();
    build(kids);
    this.layers.push({ type: 'group', ...normalize(o), layers: kids.layers } as GroupLayer);
    return this;
  }
  /** Add a component (`'barChart'`, `'code'`, `'browser'`, …; see `anikit-video info --components`). */
  component(component: string, props?: Record<string, unknown>, o?: ComponentOptions): this {
    this.layers.push({ type: 'component', component, ...(props ? { props } : {}), ...normalize(o) } as ComponentLayer);
    return this;
  }
  /** Add a raw layer object (escape hatch). */
  layer(layer: Layer): this {
    this.layers.push(layer);
    return this;
  }
}

const BOX_STYLE_KEYS = new Set(['direction', 'gap', 'padding', 'align', 'justify', 'wrap', 'grow', 'background', 'radius', 'border', 'columns']);
function isBoxStyle(a: unknown): boolean {
  return !!a && typeof a === 'object' && Object.keys(a).length > 0 && Object.keys(a).every((k) => BOX_STYLE_KEYS.has(k));
}

/** Builds one scene. */
export class SceneBuilder extends LayerList {
  /** Scene options (duration, transition, background, camera, markers, audio). */
  readonly options: SceneOptions;
  constructor(o?: SceneOptions) {
    super();
    this.options = { ...(o ?? {}) };
  }
  /** Scene length in seconds (or `'auto'`). */
  duration(d: Time | 'auto'): this {
    this.options.duration = d;
    return this;
  }
  /** How this scene comes in from the previous one. */
  transition(t: Transition): this {
    this.options.transition = t;
    return this;
  }
  /** Scene background colour or gradient. */
  background(c: Color): this {
    this.options.background = c;
    return this;
  }
  /** Scene camera. */
  camera(c: Camera): this {
    this.options.camera = c;
    return this;
  }
  /** A named time inside this scene, usable as `'@name'`. */
  marker(name: string, at: Time): this {
    this.options.markers = { ...(this.options.markers ?? {}), [name]: at };
    return this;
  }
  /** An audio cue relative to the scene start. */
  audio(src: string, o?: Omit<Audio, 'src'>): this {
    (this.options.audio ??= []).push({ src, ...(o ?? {}) });
    return this;
  }
  /** The scene as spec JSON. */
  toJSON(): Scene {
    return { ...this.options, layers: this.layers };
  }
}

/** Options for rendering commands. */
export interface RunOptions {
  /** Directory relative asset paths resolve against (default: the current directory). */
  baseDir?: string;
  /** Path to the `anikit-video` binary (default: `$ANIKIT_VIDEO_BIN` or `anikit-video` on PATH). */
  bin?: string;
  /** Extra CLI arguments. */
  args?: string[];
}

/** A video under construction. */
export class Video {
  private readonly spec_: Omit<VideoSpec, 'scenes'> & { scenes: Scene[] };
  constructor(o?: VideoOptions) {
    this.spec_ = { ...(o ?? {}), scenes: [] };
  }
  /** Add a scene: `scene(s => s.text('Hi'), { duration: 3 })` or `scene({ duration: 3 }, s => …)`. */
  scene(build: (s: SceneBuilder) => unknown, o?: SceneOptions): this;
  scene(o: SceneOptions, build: (s: SceneBuilder) => unknown): this;
  scene(a: SceneOptions | ((s: SceneBuilder) => unknown), b?: SceneOptions | ((s: SceneBuilder) => unknown)): this {
    const [build, o] = typeof a === 'function' ? [a, b as SceneOptions | undefined] : [b as (s: SceneBuilder) => unknown, a];
    const s = new SceneBuilder(o);
    build?.(s);
    this.spec_.scenes.push(s.toJSON());
    return this;
  }
  /** Default transition between scenes. */
  transition(t: Transition): this {
    this.spec_.transition = t;
    return this;
  }
  /** Theme colours, fonts and styles (merged with what is set). */
  theme(t: Theme): this {
    const cur = this.spec_.theme ?? {};
    this.spec_.theme = { ...cur, ...t, colors: { ...cur.colors, ...t.colors }, fonts: { ...cur.fonts, ...t.fonts }, styles: { ...cur.styles, ...t.styles } };
    for (const k of ['colors', 'fonts', 'styles'] as const) if (Object.keys(this.spec_.theme[k] ?? {}).length === 0) delete this.spec_.theme[k];
    return this;
  }
  /** A named time on the global timeline (`'@name'`). */
  marker(name: string, at: Time): this {
    this.spec_.markers = { ...(this.spec_.markers ?? {}), [name]: at };
    return this;
  }
  /** Music or a sound effect. */
  audio(src: string, o?: Omit<Audio, 'src'>): this {
    (this.spec_.audio ??= []).push({ src, ...(o ?? {}) });
    return this;
  }
  /** Burned-in captions: cues, or `{ src: 'subs.srt' }`. */
  captions(c: Caption[] | { src: string; style?: string; highlight?: Color }): this {
    this.spec_.captions = c;
    return this;
  }
  /** Register a font file under a family name. */
  font(family: string, path: string): this {
    this.spec_.fonts = { ...(this.spec_.fonts ?? {}), [family]: path };
    return this;
  }
  /** The spec as a plain object. */
  toJSON(): VideoSpec {
    return JSON.parse(JSON.stringify(this.spec_));
  }
  /** Alias of {@link Video.toJSON}. */
  spec(): VideoSpec {
    return this.toJSON();
  }
  /** Write the spec to a JSON file. */
  async save(path: string): Promise<string> {
    const fs = await node('node:fs/promises');
    await fs.writeFile(path, JSON.stringify(this.toJSON(), null, 2) + '\n');
    return path;
  }
  /** Render to a video file (format from the extension: .mp4, .webm, .mov, .gif). Resolves with the output path. */
  async render(out: string, o?: RunOptions & { quality?: 'draft' | 'standard' | 'high' }): Promise<string> {
    await runSpec(this.toJSON(), ['render', '{spec}', '-o', out, ...(o?.quality ? ['--quality', o.quality] : [])], o);
    return out;
  }
  /** Render one frame at `at` seconds to a PNG. */
  async still(at: number, out: string, o?: RunOptions): Promise<string> {
    await runSpec(this.toJSON(), ['still', '{spec}', '--at', String(at), '-o', out], o);
    return out;
  }
  /** Render a contact sheet (one frame per scene) to a PNG. */
  async sheet(out: string, o?: RunOptions): Promise<string> {
    await runSpec(this.toJSON(), ['sheet', '{spec}', '-o', out], o);
    return out;
  }
  /** Validate with the renderer: `{ ok, errors, warnings, duration, … }`. */
  async check(o?: RunOptions): Promise<CheckResult> {
    return JSON.parse(await runSpec(this.toJSON(), ['check', '{spec}', '--json'], o, true));
  }
  /** Heuristic design review: reading time, contrast, safe areas, pacing. */
  async critique(o?: RunOptions): Promise<CritiqueReport> {
    return JSON.parse(await runSpec(this.toJSON(), ['critique', '{spec}', '--json'], o));
  }
}

/** Result of {@link Video.check}. */
export interface CheckResult {
  /** True when the spec renders. */
  ok: boolean;
  /** Problems that stop rendering. */
  errors: Array<{ path: string; message: string; suggestion?: string }>;
  /** Probable mistakes. */
  warnings: Array<{ path: string; message: string; suggestion?: string }>;
  /** Total length in seconds. */
  duration?: number;
}

/** Result of {@link Video.critique}. */
export interface CritiqueReport {
  /** 0..100. */
  score: number;
  /** Seconds. */
  duration: number;
  /** Findings, each with a fix. */
  notes: Array<{ severity: 'error' | 'warning' | 'tip'; check: string; path: string; message: string; fix: string }>;
}

/** Start a video. */
export function video(o?: VideoOptions): Video {
  return new Video(o);
}

/** Options for {@link template}. */
export interface TemplateOptions extends RunOptions {
  /** Brand kit: an object (see `prompts/brand-kit.md`) or a path to brand.json. */
  brand?: string | Record<string, unknown>;
  /** `'16:9'` (default), `'9:16'`, `'1:1'`, `'4:5'` or `'WxH'`. */
  format?: string;
}

/** Expand a built-in template (launch, feature, changelog, demo, teaser, data-story, testimonial, tutorial, countdown, logo-reveal) into a spec. */
export async function template(name: string, params: Record<string, unknown>, o?: TemplateOptions): Promise<VideoSpec> {
  const { tmp, write } = await tempFiles();
  const p = await write(tmp('params.json'), JSON.stringify(params));
  const args = ['templates', 'expand', name, p, '--format', o?.format ?? '16:9'];
  if (o?.brand) args.push('--brand', typeof o.brand === 'string' ? o.brand : await write(tmp('brand.json'), JSON.stringify(o.brand)));
  return JSON.parse(await exec(args, o));
}

/** Render a spec object (from {@link template} or written by hand). */
export async function render(spec: VideoSpec, out: string, o?: RunOptions): Promise<string> {
  await runSpec(spec, ['render', '{spec}', '-o', out], o);
  return out;
}

// Node built-ins, loaded lazily so the builder stays importable in browsers
// (and type-checks without @types/node).
/* eslint-disable @typescript-eslint/no-explicit-any */
type Any = any;
const node = (m: string): Promise<Any> => import(/* @vite-ignore */ m);
const env = (k: string): string | undefined => (globalThis as Any).process?.env?.[k];
const pid = (): number => (globalThis as Any).process?.pid ?? 0;

let counter = 0;
async function tempFiles() {
  const [{ tmpdir }, { join }, fs] = await Promise.all([node('node:os'), node('node:path'), node('node:fs/promises')]);
  const dir = join(tmpdir(), 'anikit-video');
  await fs.mkdir(dir, { recursive: true });
  return {
    tmp: (name: string) => join(dir, `${pid()}-${++counter}-${name}`),
    write: async (path: string, text: string) => (await fs.writeFile(path, text), path),
  };
}

/** Make relative `src`/font paths absolute so the temp spec file finds them. */
export function absolutize(spec: VideoSpec, baseDir: string, join: (a: string, b: string) => string): VideoSpec {
  const rel = (s: string) => s.length > 0 && !s.startsWith('/') && !s.includes('://') && !s.startsWith('data:') && !s.startsWith('$') && !/^[A-Za-z]:[\\/]/.test(s);
  const walk = (v: unknown): unknown => {
    if (Array.isArray(v)) return v.map(walk);
    if (v && typeof v === 'object') {
      const o: Record<string, unknown> = {};
      for (const [k, x] of Object.entries(v)) {
        if ((k === 'src' || k === 'logo') && typeof x === 'string' && rel(x)) o[k] = join(baseDir, x);
        else if (k === 'fonts' && x && typeof x === 'object' && !Array.isArray(x)) {
          o[k] = Object.fromEntries(Object.entries(x).map(([f, p]) => [f, typeof p === 'string' && rel(p) && /[./]/.test(p) ? join(baseDir, p) : p]));
        } else o[k] = walk(x);
      }
      return o;
    }
    return v;
  };
  return walk(spec) as VideoSpec;
}

async function runSpec(spec: VideoSpec, args: string[], o?: RunOptions, allowFail = false): Promise<string> {
  const { join, resolve } = await node('node:path');
  const { tmp, write } = await tempFiles();
  const file = await write(tmp('spec.json'), JSON.stringify(absolutize(spec, resolve(o?.baseDir ?? '.'), join)));
  return exec(args.map((a) => (a === '{spec}' ? file : a)), o, allowFail);
}

async function exec(args: string[], o?: RunOptions, allowFail = false): Promise<string> {
  const { execFile } = await node('node:child_process');
  const bin = o?.bin ?? env('ANIKIT_VIDEO_BIN') ?? 'anikit-video';
  return new Promise((res, rej) => {
    execFile(bin, [...args, ...(o?.args ?? [])], { maxBuffer: 64 << 20 }, (err: (Error & { code?: string }) | null, stdout: string, stderr: string) => {
      if (err && !(allowFail && stdout)) {
        const missing = err?.code === 'ENOENT';
        rej(new Error(missing
          ? `anikit-video: binary '${bin}' not found. Build it with \`cargo build --release -p anikit-video\` and set ANIKIT_VIDEO_BIN, or put it on PATH.`
          : `anikit-video ${args[0]} failed:\n${stderr || stdout || err?.message}`));
      } else res(stdout);
    });
  });
}

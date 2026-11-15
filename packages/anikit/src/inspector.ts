/**
 * `inspector(animation)` — a floating dev panel to play, pause, scrub, loop,
 * change speed and jump between labels of any animation or timeline.
 *
 * ```ts
 * if (import.meta.env.DEV) inspector(tl);
 * ```
 */
import { Playable } from './playable';
import { engine } from './core';
import { Timeline } from './timeline';

export interface InspectorOptions {
  /** Panel title. */
  title?: string;
  /** Start paused at 0. */
  paused?: boolean;
  /** Container to append to (default: document.body). */
  container?: Element;
}

export function inspector(anim: Playable, options: InspectorOptions = {}) {
  if (typeof document === 'undefined') return { destroy() {}, revert() {} };
  const root = document.createElement('div');
  root.style.cssText =
    'position:fixed;left:16px;right:16px;bottom:16px;z-index:2147483647;background:rgba(18,20,26,.94);color:#e8ebf2;' +
    'font:12px/1.4 ui-monospace,Menlo,monospace;border-radius:10px;padding:10px 12px;box-shadow:0 8px 30px rgba(0,0,0,.35);' +
    'display:flex;gap:10px;align-items:center;flex-wrap:wrap';
  const btn = (label: string, title: string) => {
    const b = document.createElement('button');
    b.textContent = label;
    b.title = title;
    b.style.cssText = 'background:#2a3040;color:inherit;border:0;border-radius:6px;padding:4px 9px;font:inherit;cursor:pointer';
    return b;
  };
  const name = document.createElement('strong');
  name.textContent = options.title ?? 'anikit';
  const play = btn('❚❚', 'Play / pause (space)');
  const restart = btn('⟲', 'Restart');
  const loop = btn('loop', 'Loop');
  const range = document.createElement('input');
  range.type = 'range';
  range.min = '0';
  range.max = '1000';
  range.style.cssText = 'flex:1;min-width:160px';
  const time = document.createElement('span');
  time.style.minWidth = '110px';
  const speed = document.createElement('select');
  for (const s of [0.1, 0.25, 0.5, 1, 2, 4]) {
    const o = document.createElement('option');
    o.value = String(s);
    o.textContent = s + '×';
    if (s === 1) o.selected = true;
    speed.appendChild(o);
  }
  speed.style.cssText = 'background:#2a3040;color:inherit;border:0;border-radius:6px;padding:3px';
  const labels = document.createElement('select');
  labels.style.cssText = speed.style.cssText;
  root.append(name, play, restart, range, time, speed, loop);
  const isTl = anim instanceof Timeline;
  if (isTl) root.append(labels);
  (options.container ?? document.body).appendChild(root);

  let looping = false;
  let scrubbing = false;
  const refreshLabels = () => {
    if (!isTl) return;
    const tl = anim as Timeline;
    if (labels.options.length === tl.labels.size + 1) return;
    labels.innerHTML = '<option value="">labels…</option>';
    for (const [k, v] of tl.labels) {
      const o = document.createElement('option');
      o.value = k;
      o.textContent = `${k} (${v.toFixed(2)}s)`;
      labels.appendChild(o);
    }
  };
  play.onclick = () => (anim.state === 'running' ? anim.pause() : anim.play());
  restart.onclick = () => anim.restart();
  loop.onclick = () => {
    looping = !looping;
    loop.style.background = looping ? '#4a6cf7' : '#2a3040';
  };
  speed.onchange = () => (anim.speed = Number(speed.value) * Math.sign(anim.speed || 1));
  labels.onchange = () => labels.value && (anim as Timeline).seek(labels.value);
  range.oninput = () => {
    scrubbing = true;
    anim.pause();
    anim.progress = Number(range.value) / 1000;
  };
  range.onchange = () => (scrubbing = false);
  const key = (e: KeyboardEvent) => {
    if (e.code === 'Space' && !(e.target instanceof HTMLInputElement)) {
      e.preventDefault();
      play.click();
    }
  };
  window.addEventListener('keydown', key);
  if (options.paused) anim.pause().seek(0);
  const off = engine.onFrame(() => {
    if (!scrubbing) range.value = String(Math.round(anim.progress * 1000));
    time.textContent = `${anim.time.toFixed(2)} / ${anim.duration.toFixed(2)}s`;
    play.textContent = anim.state === 'running' ? '❚❚' : '▶';
    refreshLabels();
    if (looping && anim.state === 'finished') anim.restart();
  });
  const api = {
    destroy() {
      off();
      window.removeEventListener('keydown', key);
      root.remove();
    },
    revert() {
      api.destroy();
    },
  };
  return api;
}

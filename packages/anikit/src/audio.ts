/**
 * Audio analysis for syncing animation (and video cuts) to music.
 *
 * ```ts
 * const a = await analyzeAudio(await decodeAudio('/track.mp3'));
 * a.bpm;                     // 124.1
 * a.beats;                   // [0.48, 0.96, …] seconds
 * a.at(t);                   // loudness 0…1 at time t (for scale/glow)
 * a.onBeat(audioEl, (i) => effects.pulse('.logo'));   // live sync to an <audio>
 * ```
 *
 * The FFT, spectral-flux onset detection, tempo estimation and beat-grid fit
 * all run in the WebAssembly core.
 */
import { core, W } from './wasm';
import { engine } from './core';

export interface AudioAnalysis {
  /** Estimated tempo in beats per minute. */
  bpm: number;
  /** Beat times in seconds (a steady grid fitted to the onsets). */
  beats: number[];
  /** Onset (attack) times in seconds. */
  onsets: number[];
  /** Loudness per `hop` seconds, 0…1. */
  envelope: Float32Array;
  /** Onset strength per `hop` seconds, 0…1. */
  flux: Float32Array;
  /** Seconds between `envelope` / `flux` samples. */
  hop: number;
  /** Length of the audio in seconds. */
  duration: number;
  /** Interpolated loudness at `t` seconds. */
  at(t: number): number;
  /** Index of the beat at or before `t`, and the 0…1 phase toward the next. */
  beatAt(t: number): { index: number; phase: number };
  /** Call `fn(index)` on every beat while `media` plays. Returns a stop function. */
  onBeat(media: HTMLMediaElement, fn: (index: number) => void): () => void;
}

/** Mix an AudioBuffer (or channel arrays) down to mono. */
function mono(input: Float32Array | { numberOfChannels: number; getChannelData(c: number): Float32Array; length: number }): Float32Array {
  if (input instanceof Float32Array) return input;
  const n = input.length, ch = input.numberOfChannels, out = new Float32Array(n);
  for (let c = 0; c < ch; c++) {
    const d = input.getChannelData(c);
    for (let i = 0; i < n; i++) out[i] += d[i] / ch;
  }
  return out;
}

/** Decode audio (URL or bytes) to an AudioBuffer in the browser. */
export async function decodeAudio(src: string | ArrayBuffer): Promise<AudioBuffer> {
  const bytes = typeof src === 'string' ? await (await fetch(src)).arrayBuffer() : src;
  const AC = (globalThis as any).OfflineAudioContext ?? (globalThis as any).webkitOfflineAudioContext;
  const ctx = new AC(1, 1, 44100);
  return ctx.decodeAudioData(bytes);
}

/**
 * Analyse audio: an AudioBuffer (browser) or mono samples + sample rate.
 */
export function analyzeAudio(input: Float32Array | AudioBuffer, sampleRate?: number): AudioAnalysis {
  const samples = mono(input as any);
  const rate = sampleRate ?? (input as AudioBuffer).sampleRate ?? 44100;
  const w = core();
  const ptr = w.ak_audio_input(samples.length);
  new Float32Array(W!.memory.buffer, ptr, samples.length).set(samples);
  const bpm = w.ak_audio_analyze(rate);
  const series = (k: number) => new Float32Array(W!.memory.buffer, w.ak_audio_result(k), w.ak_audio_result_len(k)).slice();
  const envelope = series(0), flux = series(1), onsets = Array.from(series(2)), beats = Array.from(series(3));
  const hop = w.ak_audio_hop();
  const duration = samples.length / rate;
  const a: AudioAnalysis = {
    bpm, beats, onsets, envelope, flux, hop, duration,
    at(t) {
      const f = t / hop, i = Math.floor(f);
      if (i < 0) return envelope[0] ?? 0;
      if (i >= envelope.length - 1) return envelope[envelope.length - 1] ?? 0;
      return envelope[i] + (envelope[i + 1] - envelope[i]) * (f - i);
    },
    beatAt(t) {
      let lo = 0, hi = beats.length - 1, idx = -1;
      while (lo <= hi) {
        const m = (lo + hi) >> 1;
        if (beats[m] <= t) { idx = m; lo = m + 1; } else hi = m - 1;
      }
      if (idx < 0) return { index: -1, phase: 0 };
      const next = beats[idx + 1] ?? beats[idx] + 60 / (bpm || 120);
      return { index: idx, phase: (t - beats[idx]) / (next - beats[idx]) };
    },
    onBeat(media, fn) {
      let last = -1;
      return engine.onFrame(() => {
        const { index } = a.beatAt(media.currentTime);
        if (index !== last && index >= 0 && !media.paused) fn(index);
        last = index;
      });
    },
  };
  return a;
}

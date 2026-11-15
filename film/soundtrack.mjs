// A procedural soundtrack for the film: 60 s, 120 BPM, A minor, arranged to
// the film's movements. Pure JS synthesis (no samples): kick, clap, hats,
// bass, detuned-saw pads, Karplus–Strong plucks, risers and impacts, a
// feedback-delay "room" and a soft-clip master. Deterministic.
import { writeFileSync } from 'node:fs';

export const BPM = 120;
export const SR = 44100;
const BEAT = 60 / BPM;

export function synth(duration = 60) {
  const n = Math.round(duration * SR);
  const L = new Float32Array(n), R = new Float32Array(n);
  let seed = 7;
  const rnd = () => ((seed = (seed * 1664525 + 1013904223) >>> 0) / 4294967296) * 2 - 1;
  const add = (buf, i, v) => { if (i >= 0 && i < n) buf[i] += v; };
  const both = (i, v, pan = 0) => { add(L, i, v * (1 - pan) * 0.5 + v * 0.5); add(R, i, v * (1 + pan) * 0.5 + v * 0.5); };
  const hz = (midi) => 440 * Math.pow(2, (midi - 69) / 12);

  const kick = (t, g = 1) => {
    const s = Math.round(t * SR);
    let ph = 0;
    for (let i = 0; i < SR * 0.45; i++) {
      const x = i / SR, f = 45 + 110 * Math.exp(-x * 28);
      ph += (2 * Math.PI * f) / SR;
      both(s + i, Math.sin(ph) * Math.exp(-x * 7.5) * 0.9 * g + (i < 60 ? rnd() * 0.25 * g : 0));
    }
  };
  const clap = (t, g = 1) => {
    const s = Math.round(t * SR);
    let lp = 0;
    for (let i = 0; i < SR * 0.22; i++) {
      const x = i / SR, burst = x < 0.03 ? (Math.floor(x / 0.01) % 2 ? 0.6 : 1) : 1;
      const nz = rnd(); lp += (nz - lp) * 0.35;
      both(s + i, (nz - lp) * Math.exp(-x * 22) * 0.55 * g * burst + Math.sin(2 * Math.PI * 190 * x) * Math.exp(-x * 40) * 0.2 * g, 0.1);
    }
  };
  const hat = (t, g = 1, open = false) => {
    const s = Math.round(t * SR);
    let prev = 0;
    for (let i = 0; i < SR * (open ? 0.25 : 0.06); i++) {
      const x = i / SR, nz = rnd(), hp = nz - prev; prev = nz;
      both(s + i, hp * Math.exp(-x * (open ? 14 : 60)) * 0.16 * g, (i % 2 ? 0.4 : -0.4) * 0.5);
    }
  };
  const bass = (t, midi, len, g = 1) => {
    const s = Math.round(t * SR), f = hz(midi);
    let lp = 0;
    for (let i = 0; i < SR * len; i++) {
      const x = i / SR, saw = ((x * f) % 1) * 2 - 1;
      const cut = 0.04 + 0.25 * Math.exp(-x * 9);
      lp += (saw - lp) * cut;
      const env = Math.min(1, x * 200) * Math.exp(-x * 2.2);
      both(s + i, (lp * 0.5 + Math.sin(2 * Math.PI * f * x) * 0.5) * env * 0.38 * g);
    }
  };
  const pad = (t, notes, len, g = 1) => {
    const s = Math.round(t * SR);
    const voices = notes.flatMap((m, k) => [-0.12, 0, 0.11].map((d, j) => ({ f: hz(m + d), pan: (j - 1) * 0.6 + (k - 1) * 0.15 })));
    const lps = voices.map(() => 0);
    for (let i = 0; i < SR * len; i++) {
      const x = i / SR, env = Math.min(1, x / 0.6) * Math.min(1, (len - x) / 0.8);
      voices.forEach((v, k) => {
        const saw = ((x * v.f + k * 0.13) % 1) * 2 - 1;
        lps[k] += (saw - lps[k]) * 0.035;
        both(s + i, lps[k] * env * 0.05 * g, v.pan);
      });
    }
  };
  const pluck = (t, midi, g = 1, pan = 0) => {
    const s = Math.round(t * SR), p = Math.round(SR / hz(midi));
    const buf = new Float32Array(p).map(() => rnd());
    for (let i = 0; i < SR * 1.2; i++) {
      const k = i % p, nxt = (i + 1) % p;
      const v = buf[k];
      buf[k] = (buf[k] + buf[nxt]) * 0.4985;
      both(s + i, v * 0.22 * g * Math.exp(-(i / SR) * 1.6), pan);
    }
  };
  const riser = (t0, t1, g = 1) => {
    const s = Math.round(t0 * SR), len = Math.round((t1 - t0) * SR);
    let lp = 0;
    for (let i = 0; i < len; i++) {
      const u = i / len, nz = rnd();
      lp += (nz - lp) * (0.01 + 0.5 * u * u);
      both(s + i, lp * Math.pow(u, 2.2) * 0.5 * g + Math.sin(2 * Math.PI * (200 + 1800 * u * u) * (i / SR)) * u * u * 0.04 * g, Math.sin(u * 20) * 0.3);
    }
  };
  const impact = (t, g = 1) => {
    kick(t, 1.3 * g);
    const s = Math.round(t * SR);
    let lp = 0;
    for (let i = 0; i < SR * 2.2; i++) {
      const x = i / SR, nz = rnd(); lp += (nz - lp) * 0.08;
      both(s + i, (lp * Math.exp(-x * 1.8) * 0.5 + Math.sin(2 * Math.PI * 38 * x) * Math.exp(-x * 1.4) * 0.5) * g);
    }
  };
  const drone = (t0, t1, midi, g = 1) => {
    const s = Math.round(t0 * SR), len = Math.round((t1 - t0) * SR), f = hz(midi);
    for (let i = 0; i < len; i++) {
      const x = i / SR, env = Math.min(1, x / 1.2) * Math.min(1, (t1 - t0 - x) / 0.5);
      both(s + i, (Math.sin(2 * Math.PI * f * x) + 0.3 * Math.sin(2 * Math.PI * f * 2.003 * x)) * env * 0.22 * g);
    }
  };

  // Am – F – C – G, one chord per bar (2 s).
  const chords = [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]];
  const roots = [33, 29, 36, 31];
  const bar = (t) => Math.floor(t / (BEAT * 4)) % 4;
  const arp = [0, 2, 1, 2, 0, 1, 2, 1];

  // 0–4  Frozen: drone, shimmer, riser into an impact as time resumes.
  drone(0, 4.2, 33, 1);
  for (let k = 0; k < 8; k++) pluck(0.25 + k * 0.5, 81 + [0, 3, 7, 10][k % 4], 0.25, (k % 2 ? 0.6 : -0.6));
  riser(1.8, 4.0, 1);
  impact(4.0, 1);
  // 4–8  Title: half-time kick, pads.
  for (let b = 0; b < 8; b += 2) kick(4 + b * BEAT, 0.8);
  pad(4, chords[0], 2.1); pad(6, chords[1], 2.1);
  // 8–18 Sand: groove; a fill at the collapse; reverse sweep for the rewind.
  for (let b = 0; b < 20; b++) {
    const t = 8 + b * BEAT;
    if (t < 13.6 || t >= 17) kick(t, 0.95);
    hat(t + BEAT / 2, 0.8);
    if (b % 4 === 2) clap(t, 0.7);
    if (t < 13.6) bass(t, roots[bar(t)] + 12 * (b % 2), BEAT * 0.9, 0.8);
  }
  pad(8, chords[2], 2.1, 0.8); pad(10, chords[3], 2.1, 0.8);
  riser(13.2, 16.8, 0.8);
  impact(17, 0.8);
  // 18–28 Through: full groove, the drop.
  for (let b = 0; b < 20; b++) {
    const t = 18 + b * BEAT;
    kick(t, 1);
    if (b % 2 === 1) clap(t, 0.85);
    for (let k = 0; k < 4; k++) hat(t + k * BEAT / 4, k === 2 ? 1 : 0.55, k === 2 && b % 4 === 3);
    bass(t, roots[bar(t)], BEAT * 0.45, 1);
    bass(t + BEAT / 2, roots[bar(t)] + 12, BEAT * 0.45, 0.8);
  }
  for (let k = 0; k < 5; k++) pad(18 + k * 2, chords[(k + 0) % 4], 2.05, 1);
  riser(26, 28, 0.7);
  // 28–34 Springs: kicks with plucked arpeggios on every 8th.
  for (let b = 0; b < 12; b++) {
    const t = 28 + b * BEAT;
    kick(t, 0.9);
    if (b % 2 === 1) clap(t, 0.6);
    const c = chords[bar(t)];
    pluck(t, c[arp[(b * 2) % 8]] + 12, 0.9, -0.3);
    pluck(t + BEAT / 2, c[arp[(b * 2 + 1) % 8]] + 24, 0.7, 0.3);
  }
  pad(28, chords[0], 6.1, 0.7);
  // 34–40 Ease: breakdown, plucks and soft hats.
  for (let b = 0; b < 12; b++) {
    const t = 34 + b * BEAT, c = chords[bar(t)];
    pluck(t, c[arp[b % 8]] + 12, 0.8, Math.sin(b) * 0.5);
    hat(t + BEAT / 2, 0.45);
  }
  pad(34, chords[1], 3.05, 0.8); pad(37, chords[2], 3.05, 0.8);
  // 40–46 Tunnel: build — kick, accelerating snare roll, riser.
  for (let b = 0; b < 12; b++) kick(40 + b * BEAT, 0.95);
  for (let t = 40, step = BEAT; t < 46; t += step) { clap(t, 0.35 + (t - 40) / 9); step = Math.max(BEAT / 8, step * 0.9); }
  riser(42, 46, 1);
  // 46–54 Galaxy: the big section.
  impact(46, 1);
  for (let b = 0; b < 16; b++) {
    const t = 46 + b * BEAT;
    kick(t, 1);
    if (b % 2 === 1) clap(t, 0.8);
    hat(t + BEAT / 2, 0.9, b % 4 === 3);
    bass(t, roots[bar(t)], BEAT * 0.9, 1);
  }
  for (let k = 0; k < 4; k++) pad(46 + k * 2, chords[k], 2.05, 1.2);
  riser(52, 54, 0.9);
  // 54–60 Signature: final hit, ringing chord.
  impact(54, 1.1);
  pad(54, [45, 57, 60, 64], 6, 1.1);
  for (let k = 0; k < 6; k++) pluck(54.5 + k * 0.25, [69, 72, 76, 81, 84, 88][k], 0.6, (k - 2.5) / 3);

  // Room: two feedback delays, then a soft-clip master.
  const room = (buf, d1, d2) => {
    const a = Math.round(d1 * SR), b = Math.round(d2 * SR);
    for (let i = b; i < n; i++) buf[i] += buf[i - a] * 0.22 + buf[i - b] * 0.15;
  };
  room(L, 0.043, 0.071); room(R, 0.051, 0.083);
  for (let i = 0; i < n; i++) { L[i] = Math.tanh(L[i] * 1.1) * 0.9; R[i] = Math.tanh(R[i] * 1.1) * 0.9; }
  const mono = new Float32Array(n);
  for (let i = 0; i < n; i++) mono[i] = (L[i] + R[i]) / 2;
  return { L, R, mono, rate: SR };
}

/** Write a 16-bit stereo WAV. */
export function writeWav(path, { L, R, rate }) {
  const n = L.length, buf = Buffer.alloc(44 + n * 4);
  buf.write('RIFF', 0); buf.writeUInt32LE(36 + n * 4, 4); buf.write('WAVE', 8); buf.write('fmt ', 12);
  buf.writeUInt32LE(16, 16); buf.writeUInt16LE(1, 20); buf.writeUInt16LE(2, 22); buf.writeUInt32LE(rate, 24);
  buf.writeUInt32LE(rate * 4, 28); buf.writeUInt16LE(4, 32); buf.writeUInt16LE(16, 34); buf.write('data', 36); buf.writeUInt32LE(n * 4, 40);
  for (let i = 0; i < n; i++) {
    buf.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(L[i] * 32767))), 44 + i * 4);
    buf.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(R[i] * 32767))), 46 + i * 4);
  }
  writeFileSync(path, buf);
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const t0 = Date.now();
  const s = synth(60);
  writeWav(process.argv[2] || 'soundtrack.wav', s);
  console.log('soundtrack →', process.argv[2] || 'soundtrack.wav', `${((Date.now() - t0) / 1000).toFixed(1)} s`);
}

// hall.js — the hall of demos. A bright, walkable gallery in hand-written WebGL2.
// createHall(canvas, overlayEl) → { resize, preview, enter, exit, frame }  (see CONTRACT.md)
//
// Every moving thing is driven by anikit: world() physics, spring() followers and
// movement, batch() + stagger waves, timeline()/keyframes for the letters, animate()
// for the point-sculpture morphs, HUD fades and station wake-ups.

import { ready, world, spring, animate, batch, stagger, timeline } from '../../../packages/anikit/dist/anikit.lean.js';
import { program, cubeMesh, cylinderMesh, sphereMesh, group, camera, updateCamera, sees } from './hall-gl.js';
import * as S from './hall-shaders.js';
import { createControls } from './hall-controls.js';

const SUN = [0.72, -0.62, -0.3];
{ const l = Math.hypot(SUN[0], SUN[1], SUN[2]); SUN[0] /= l; SUN[1] /= l; SUN[2] /= l; }
const FOG = [0.55, 0.49, 0.42];
const SPAWN = [0, 1.7, 12];
const PI2 = Math.PI * 2;

// hall extents
const HX = 8, HZ0 = -28, HZ1 = 16, HY = 12;

// stations (positions in metres)
const ST = [
  { id: 'springs', name: 'Springs', hint: 'Walk past — they follow · E to scatter', x: -4.6, z: 4.2, r: 4.2 },
  { id: 'bench', name: 'Benchmarks', hint: 'Numbers from README.md · E to replay', x: 4.8, z: 4.2, r: 3.6 },
  { id: 'physics', name: 'Physics · world()', hint: 'Click or E to throw the balls', x: -5.2, z: -6.5, r: 3.8 },
  { id: 'blocks', name: 'Timeline + stagger', hint: 'Click or E for the next wave', x: 4.6, z: -6.5, r: 3.8 },
  { id: 'cloud', name: 'Particles · 100k points', hint: 'Click or E to morph', x: 0, z: -16, r: 4.8 },
  { id: 'text', name: 'Keyframes', hint: 'Click or E to play the letters', x: 0, z: -23.2, r: 4.6 },
];

export function createHall(canvas, overlayEl) {
  const gl = canvas.getContext('webgl2', { antialias: (globalThis.devicePixelRatio || 1) < 1.3, alpha: false, depth: true, powerPreference: 'high-performance', preserveDrawingBuffer: false });
  if (!gl) throw new Error('hall: WebGL2 unavailable');

  const progMesh = program(gl, S.MESH_VS, S.MESH_FS);
  const progFloor = program(gl, S.FLOOR_VS, S.FLOOR_FS);
  const progBeam = program(gl, S.BEAM_VS, S.BEAM_FS);
  const progPts = program(gl, S.PTS_VS, S.PTS_FS);
  const progDust = program(gl, S.DUST_VS, S.DUST_FS);
  const progTex = program(gl, S.TEX_VS, S.TEX_FS);

  const cube = cubeMesh(), cyl = cylinderMesh(22), sph = sphereMesh(14, 9), sphLo = sphereMesh(10, 6);
  const cam = camera();
  let W = 1, H = 1, pxScale = 1;

  // ---------- static architecture ----------
  const archBox = group(gl, cube, 256);
  const archCyl = group(gl, cyl, 128);
  const windows = group(gl, cube, 32);
  const colliders = []; // [x0, z0, x1, z1]
  const box = (g, x, y, z, sx, sy, sz, gloss, r, gg, b, e = 0, collide = false) => {
    g.push(x, y, z, 0, sx, sy, sz, gloss, r, gg, b, e);
    if (collide) colliders.push(x - sx / 2, z - sz / 2, x + sx / 2, z + sz / 2);
  };
  const WALL = [0.74, 0.54, 0.44], STONE = [0.84, 0.8, 0.74], GOLD = [0.8, 0.52, 0.2], CEIL = [0.6, 0.7, 0.86];
  // walls (thick boxes), ceiling
  box(archBox, -HX - 0.5, HY / 2, (HZ0 + HZ1) / 2, 1, HY, HZ1 - HZ0, 0.05, ...WALL);
  box(archBox, HX + 0.5, HY / 2, (HZ0 + HZ1) / 2, 1, HY, HZ1 - HZ0, 0.05, ...WALL);
  box(archBox, 0, HY / 2, HZ0 - 0.5, 2 * HX + 2, HY, 1, 0.05, ...WALL);
  box(archBox, 0, HY / 2, HZ1 + 0.5, 2 * HX + 2, HY, 1, 0.05, ...WALL);
  box(archBox, 0, HY + 0.5, (HZ0 + HZ1) / 2, 2 * HX + 2, 1, HZ1 - HZ0 + 2, 0, ...CEIL, 0.55);
  // skirting + cornice
  for (const s of [-1, 1]) {
    box(archBox, s * (HX - 0.06), 0.18, (HZ0 + HZ1) / 2, 0.14, 0.36, HZ1 - HZ0, 0.2, 0.62, 0.55, 0.47);
    box(archBox, s * (HX - 0.15), 11.6, (HZ0 + HZ1) / 2, 0.3, 0.3, HZ1 - HZ0, 0.3, ...GOLD, 0.05);
    box(archBox, s * (HX - 0.08), 5.9, (HZ0 + HZ1) / 2, 0.16, 0.2, HZ1 - HZ0, 0.2, ...STONE);
  }
  // coffered ceiling beams
  for (let z = HZ0 + 2.5; z < HZ1; z += 5) box(archBox, 0, HY - 0.35, z, 2 * HX, 0.7, 0.45, 0.1, ...STONE);
  for (const x of [-4, 0, 4]) box(archBox, x, HY - 0.25, (HZ0 + HZ1) / 2, 0.35, 0.5, HZ1 - HZ0, 0.1, ...STONE);
  // columns between the window bays
  for (let z = 11; z > HZ0; z -= 5) {
    for (const s of [-1, 1]) {
      const x = s * 7.0;
      archCyl.push(x, 5.6, z, 0, 0.82, 10.4, 0.82, 0.35, 0.93, 0.9, 0.85, 0);
      box(archBox, x, 0.3, z, 1.2, 0.6, 1.2, 0.2, ...STONE, 0, true);
      box(archBox, x, 10.95, z, 1.15, 0.5, 1.15, 0.6, ...GOLD, 0.06);
      archCyl.push(x, 0.7, z, 0, 1.0, 0.22, 1.0, 0.6, ...GOLD, 0.04);
    }
  }
  // windows: both walls, emissive sky (emis > 2 flags the window shader)
  for (let z = 8.5; z > HZ0; z -= 5) {
    for (const s of [-1, 1]) {
      windows.push(s * (HX - 0.02), 8.6, z, 0, 0.06, 4.2, 2.2, 0, 1, 1, 1, 3);
      box(archBox, s * (HX - 0.05), 6.35, z, 0.25, 0.25, 2.6, 0.4, ...GOLD, 0.05);
    }
  }
  // end wall: a great arch frame behind the letters
  box(archBox, 0, 9.3, HZ0 + 0.1, 14.4, 0.4, 0.3, 0.5, ...GOLD, 0.05);
  box(archBox, -7.2, 4.65, HZ0 + 0.1, 0.4, 9.3, 0.3, 0.5, ...GOLD, 0.05);
  box(archBox, 7.2, 4.65, HZ0 + 0.1, 0.4, 9.3, 0.3, 0.5, ...GOLD, 0.05);
  box(archBox, 0, 4.6, HZ0 + 0.04, 14, 9.2, 0.1, 0.05, 0.92, 0.84, 0.72);
  // door frame behind spawn (bright: where you came from)
  windows.push(0, 3.4, HZ1 - 0.02, 0, 3.6, 6.4, 0.06, 0, 1, 1, 1, 3);
  box(archBox, 0, 6.8, HZ1 - 0.1, 4.4, 0.5, 0.3, 0.5, ...GOLD, 0.05);
  box(archBox, -2.0, 3.4, HZ1 - 0.1, 0.4, 6.8, 0.3, 0.5, ...GOLD, 0.05);
  box(archBox, 2.0, 3.4, HZ1 - 0.1, 0.4, 6.8, 0.3, 0.5, ...GOLD, 0.05);

  // station furniture
  // springs: a low round dais
  archCyl.push(-4.6, 0.12, 4.2, 0, 2.6, 0.24, 2.6, 0.3, ...STONE, 0);
  archCyl.push(-4.6, 0.55, 4.2, 0, 0.5, 1.1, 0.5, 0.4, ...STONE, 0);
  colliders.push(-4.95, 3.85, -4.25, 4.55);
  // benchmarks: plinth
  box(archBox, 5.4, 0.45, 4.2, 1.3, 0.9, 3.8, 0.25, ...STONE, 0, true);
  box(archBox, 5.4, 0.92, 4.2, 1.36, 0.05, 3.86, 0.6, ...GOLD, 0.05);
  // physics: case base + frame + back
  const PC = { x: -6.35, z0: -8.3, z1: -4.7, y0: 0.7, y1: 3.3 };
  box(archBox, PC.x, PC.y0 / 2, (PC.z0 + PC.z1) / 2, 0.8, PC.y0, PC.z1 - PC.z0 + 0.3, 0.3, ...STONE, 0, true);
  box(archBox, PC.x - 0.25, (PC.y0 + PC.y1) / 2, (PC.z0 + PC.z1) / 2, 0.06, PC.y1 - PC.y0, PC.z1 - PC.z0, 0.1, 0.94, 0.92, 0.9, 0.12);
  box(archBox, PC.x, PC.y1 + 0.05, (PC.z0 + PC.z1) / 2, 0.6, 0.1, PC.z1 - PC.z0 + 0.3, 0.6, ...GOLD, 0.05);
  box(archBox, PC.x + 0.22, (PC.y0 + PC.y1) / 2, PC.z0 - 0.05, 0.08, PC.y1 - PC.y0, 0.1, 0.6, ...GOLD, 0.05);
  box(archBox, PC.x + 0.22, (PC.y0 + PC.y1) / 2, PC.z1 + 0.05, 0.08, PC.y1 - PC.y0, 0.1, 0.6, ...GOLD, 0.05);
  // blocks: platform
  box(archBox, 4.6, 0.12, -6.5, 4.0, 0.24, 4.0, 0.3, ...STONE, 0, true);
  box(archBox, 4.6, 0.25, -6.5, 4.06, 0.03, 4.06, 0.6, ...GOLD, 0.04);
  // cloud: pedestal
  archCyl.push(0, 0.3, -16, 0, 3.2, 0.6, 3.2, 0.3, ...STONE, 0);
  archCyl.push(0, 0.62, -16, 0, 3.26, 0.05, 3.26, 0.6, ...GOLD, 0.05);
  colliders.push(-1.5, -17.5, 1.5, -14.5);
  // text: stage
  box(archBox, 0, 0.2, -25.6, 15, 0.4, 3.4, 0.3, ...STONE, 0, true);
  box(archBox, 0, 0.41, -24.0, 15, 0.03, 0.08, 0.6, ...GOLD, 0.04);
  for (let i = 0; i < 6; i++) archCyl.push((i - 2.5) * 2.3, 0.62, -25.6, 0, 1.3, 0.4, 1.3, 0.6, ...GOLD, 0.05);

  archBox.upload(); archCyl.upload(); windows.upload();

  // floor quad
  const floorVao = gl.createVertexArray();
  gl.bindVertexArray(floorVao);
  const fb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, fb);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-HX, HZ0, HX, HZ0, HX, HZ1, -HX, HZ0, HX, HZ1, -HX, HZ1]), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

  // light shafts: one slanted slab per window (two crossing quads)
  const beamV = [];
  for (let z = 8.5; z > HZ0; z -= 5) {
    const t = (HX * 2 - 1.5) / SUN[0]; // run until near the far wall
    const tFloor = (8.6) / -SUN[1];
    const L = Math.min(t, tFloor + 1.5);
    for (const [ya, yb, za, zb] of [[6.6, 10.6, z, z], [8.6, 8.6, z - 1.1, z + 1.1]]) {
      const p0 = [-HX + 0.05, ya, za], p1 = [-HX + 0.05, yb, zb];
      if (ya === yb) { p0[2] = za; p1[2] = zb; }
      const q0 = [p0[0] + SUN[0] * L, p0[1] + SUN[1] * L, p0[2] + SUN[2] * L];
      const q1 = [p1[0] + SUN[0] * L, p1[1] + SUN[1] * L, p1[2] + SUN[2] * L];
      beamV.push(...p0, 0, 0, ...p1, 1, 0, ...q1, 1, 1, ...p0, 0, 0, ...q1, 1, 1, ...q0, 0, 1);
    }
  }
  const beamVao = gl.createVertexArray();
  gl.bindVertexArray(beamVao);
  const bb = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, bb);
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(beamV), gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 20, 0);
  gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 20, 12);
  const beamCount = beamV.length / 5;

  // dust motes along the shafts
  const DUST = 900;
  const dustData = new Float32Array(DUST * 4);
  for (let i = 0; i < DUST; i++) {
    const bay = 8.5 - 5 * Math.floor(Math.random() * 9);
    const t = Math.random() * 13;
    dustData[i * 4] = -HX + SUN[0] * t + (Math.random() - 0.5) * 0.6;
    dustData[i * 4 + 1] = 8.6 + SUN[1] * t + (Math.random() - 0.5) * 3.5;
    dustData[i * 4 + 2] = bay + SUN[2] * t + (Math.random() - 0.5) * 2.0;
    dustData[i * 4 + 3] = Math.random();
  }
  const dustVao = gl.createVertexArray();
  gl.bindVertexArray(dustVao);
  const db = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, db);
  gl.bufferData(gl.ARRAY_BUFFER, dustData, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 4, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);

  // ---------- stations ----------
  const balls = group(gl, sphLo, 320);
  const orbs = group(gl, sph, 16);
  const blocks = group(gl, cube, 256);
  const letters = group(gl, cube, 220);
  const bars = group(gl, cube, 8);

  const st = { ready: false, active: false, near: -1, nearK: new Float32Array(ST.length) };
  const wake = ST.map(() => ({ v: 0 }));  // proximity glow per station, animated

  // ---- particles: GPU points, morph mix driven by anikit animate() ----
  const NP = 100_000;
  const shapes = makeShapes(NP);
  const ptsVao = gl.createVertexArray();
  gl.bindVertexArray(ptsVao);
  const pA = gl.createBuffer(), pB = gl.createBuffer(), pS = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, pA); gl.bufferData(gl.ARRAY_BUFFER, shapes[0], gl.DYNAMIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0);
  gl.bindBuffer(gl.ARRAY_BUFFER, pB); gl.bufferData(gl.ARRAY_BUFFER, shapes[1], gl.DYNAMIC_DRAW);
  gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0);
  const seeds = new Float32Array(NP * 4);
  for (let i = 0; i < NP; i++) {
    const y = shapes[0][i * 3 + 1];
    seeds[i * 4] = Math.min(1, Math.max(0, (y + 1.6) / 3.2 * 0.75 + Math.random() * 0.25));
    seeds[i * 4 + 1] = 0.016 + Math.random() * 0.016;
    seeds[i * 4 + 2] = Math.random() * 0.35 + 0.05;
    seeds[i * 4 + 3] = Math.random();
  }
  gl.bindBuffer(gl.ARRAY_BUFFER, pS); gl.bufferData(gl.ARRAY_BUFFER, seeds, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(2); gl.vertexAttribPointer(2, 4, gl.FLOAT, false, 0, 0);
  gl.bindVertexArray(null);
  const cloud = { mix: 0, cur: 0, next: 1, spin: 0, spinV: { v: 0.12 }, busy: false, idle: 0 };
  const CLOUD_C = [0, 2.75, -16];

  function morph() {
    // fold the current blend into A, then morph to the next shape
    cloud.cur = cloud.mix > 0.5 ? cloud.next : cloud.cur;
    cloud.next = (cloud.cur + 1) % shapes.length;
    gl.bindBuffer(gl.ARRAY_BUFFER, pA); gl.bufferSubData(gl.ARRAY_BUFFER, 0, shapes[cloud.cur]);
    gl.bindBuffer(gl.ARRAY_BUFFER, pB); gl.bufferSubData(gl.ARRAY_BUFFER, 0, shapes[cloud.next]);
    cloud.mix = 0;
    cloud.idle = 0;
    animate(cloud, { mix: [0, 1] }, { duration: 2.6, ease: 'inOutCubic' });
    // a spin kick that settles on a spring
    spring(cloud.spinV, { v: 0.12 }, { stiffness: 30, damping: 6, velocity: 2.4 });
  }

  // ---- benchmark panel (canvas texture) ----
  const BENCH = [
    // label, ratio (lower bound shown on the bar), text — all from README.md
    ['1k objects vs GSAP 3 (1.3–1.5× at 10k–50k)', 3.5, '3.5–3.8×'],
    ['batch(), 100k objects vs GSAP 3', 16, '~16×'],
    ['2D physics, 250 bodies vs matter.js', 43, '43–60×'],
    ['3D project + sort, 100k pts vs JS', 20, '20–22×'],
    ['2,000 DOM elements, fps vs GSAP', 2.2, '~2.2×'],
  ];
  const panel = makePanel(gl, BENCH);
  const benchState = BENCH.map(() => ({ h: 0 }));

  // ---- physics (world in px, mapped 1 px = 1 cm into the glass case) ----
  let wld = null;
  const PW = Math.round((PC.z1 - PC.z0) * 100), PH = Math.round((PC.y1 - PC.y0) * 100);
  const NB = 260;
  const ballCol = new Float32Array(NB * 3);
  const ballR = new Float32Array(NB);
  const BALL_PAL = [[0.95, 0.42, 0.28], [0.98, 0.72, 0.25], [0.32, 0.62, 0.95], [0.95, 0.94, 0.9], [0.5, 0.82, 0.7]];
  for (let i = 0; i < NB; i++) {
    const c = BALL_PAL[i % BALL_PAL.length];
    ballCol[i * 3] = c[0]; ballCol[i * 3 + 1] = c[1]; ballCol[i * 3 + 2] = c[2];
    ballR[i] = 6 + ((i * 7919) % 6);
  }

  // ---- springs: orbs that follow you ----
  const NO = 9;
  const ORB_PAL = [[0.98, 0.72, 0.3], [0.97, 0.45, 0.38], [0.93, 0.4, 0.62]];
  const orbS = [];
  for (let i = 0; i < NO; i++) {
    const a = (i / NO) * PI2;
    const hx = -4.6 + Math.cos(a) * 1.0, hy = 1.6 + 0.35 * Math.sin(a * 2), hz = 4.2 + Math.sin(a) * 1.0;
    orbS.push({ x: hx, y: hy, z: hz, hx, hy, hz, k: 26 + i * 9, d: 7 + i * 0.6, s: 0.15 + (i % 3) * 0.04 });
  }
  let orbFollow = false, orbRetarget = 0;

  // ---- timeline + stagger: 16×16 blocks, two superposed batch() waves ----
  const GB = 16, NBk = GB * GB;
  const waves = [null, null];
  let waveN = 0;
  const WAVE_FROM = ['center', 'edges', 0, 'random', 'last'];
  function wave() {
    const from = WAVE_FROM[waveN % WAVE_FROM.length];
    const b = batch(NBk, { y: [0, 1, -0.08, 0], s: [0, 0.3, 0] }, {
      duration: 1.6, ease: 'inOutSine', delay: stagger(0.045, { grid: [GB, GB], from }),
    });
    const old = waves[waveN & 1];
    waves[waveN & 1] = b;
    if (old) old.kill();
    waveN++;
  }

  // ---- text: voxel letters "anikit" animated with keyframes and stagger ----
  const FONT = {
    a: ['.....', '.....', '.###.', '....#', '.####', '#...#', '.####'],
    n: ['.....', '.....', '#.##.', '##..#', '#...#', '#...#', '#...#'],
    i: ['..#..', '.....', '.##..', '..#..', '..#..', '..#..', '.###.'],
    k: ['#....', '#....', '#..#.', '#.#..', '##...', '#.#..', '#..#.'],
    t: ['.#...', '.#...', '####.', '.#...', '.#...', '.#..#', '..##.'],
  };
  const WORD = 'anikit', VOX = 0.38;
  const letterS = [];
  const vox = []; // [letter, lx, ly]
  for (let li = 0; li < WORD.length; li++) {
    const rows = FONT[WORD[li]];
    letterS.push({ y: 0, ry: 0, s: 1, bob: 0, cx: (li - (WORD.length - 1) / 2) * 2.3 });
    for (let r = 0; r < 7; r++) for (let c = 0; c < 5; c++) if (rows[r][c] === '#') vox.push(li, (c - 2) * VOX, (6 - r) * VOX);
  }
  const NV = vox.length / 3;
  let textTl = null;
  function playText() {
    if (textTl) textTl.kill();
    textTl = timeline({ defaults: { ease: 'inOutCubic' } })
      .add(letterS, { y: [0, 1.3, -0.1, 0], s: [1, 1.18, 0.95, 1] }, { duration: 1.5, delay: stagger(0.09), times: [0, 0.45, 0.8, 1] })
      .add(letterS, { ry: [0, PI2] }, { duration: 1.2, delay: stagger(0.09, { from: 'center' }), ease: 'outBack' }, '<0.25');
  }

  // ---------- async: anikit core, then start the stations ----------
  ready.then(() => {
    wld = world({ gravity: [0, 900], bounds: [0, 0, PW, PH], restitution: 0.45, wallRestitution: 0.5, friction: 0.04, substeps: 3 });
    const bodies = [];
    for (let i = 0; i < NB; i++) bodies.push({ x: 20 + (i * 37) % (PW - 40), y: 10 + ((i * 13) % 120), r: ballR[i] });
    wld.addMany(bodies);
    // idle loops (all anikit)
    animate(letterS, { bob: [0, 0.14, 0] }, { duration: 3.2, repeat: -1, ease: 'inOutSine', delay: stagger(0.3) });
    wave();
    st.ready = true;
  });

  // ---------- controls & HUD ----------
  const ctl = createControls(canvas, overlayEl, {
    spawn: SPAWN, colliders, bounds: [-HX + 0.45, HZ0 + 0.45, HX - 0.45, HZ1 - 0.45],
    onUse: () => use(),
  });

  function use() {
    if (!st.ready || st.near < 0) return;
    const id = ST[st.near].id;
    if (id === 'physics') {
      // throw: an upward blast under where you stand, plus a sideways scatter
      const pz = Math.min(Math.max((ctl.pos[2] - PC.z0) * 100, 0), PW);
      wld.impulse(pz, PH, 160, 1400);
      wld.impulse(PW - pz, PH * 0.6, 120, 700);
    } else if (id === 'cloud') morph();
    else if (id === 'blocks') wave();
    else if (id === 'text') playText();
    else if (id === 'bench') benchIn(true);
    else if (id === 'springs') {
      for (let i = 0; i < NO; i++) {
        const o = orbS[i], a = (i / NO) * PI2;
        spring(o, { x: o.hx, y: o.hy, z: o.hz }, { stiffness: 40, damping: 5, velocity: { x: Math.cos(a) * 9, y: 6 + i, z: Math.sin(a) * 9 } });
      }
      orbFollow = false; orbRetarget = 0.9;
    }
  }

  function benchIn(replay) {
    for (let i = 0; i < BENCH.length; i++) {
      const target = Math.log2(1 + BENCH[i][1]) / Math.log2(61);
      animate(benchState[i], { h: replay ? [0, target] : target }, { duration: 1.1, delay: i * 0.12, ease: 'outBack' });
    }
  }

  // ---------- per-frame updates (no allocation) ----------
  function wakeStation(i, on) {
    animate(wake[i], { v: on ? 1 : 0 }, { duration: on ? 0.6 : 0.9, ease: 'outCubic' });
    if (on && ST[i].id === 'bench') benchIn(false);
    if (on && st.ready) {
      const id = ST[i].id;
      if (id === 'blocks') wave();
      else if (id === 'text') playText();
      else if (id === 'physics') wld.impulse(PW / 2, PH, 140, 700);
      else if (id === 'cloud' && cloud.mix >= 1 && cloud.idle > 2) morph();
    }
  }

  function updateNear(px, pz) {
    let best = -1, bd = 1e9;
    for (let i = 0; i < ST.length; i++) {
      const dx = px - ST[i].x, dz = pz - ST[i].z, d = Math.hypot(dx, dz);
      if (d < ST[i].r && d < bd) { bd = d; best = i; }
    }
    if (best !== st.near) {
      if (st.near >= 0) wakeStation(st.near, false);
      if (best >= 0) wakeStation(best, true);
      st.near = best;
      ctl.setStation(best >= 0 ? ST[best].name : '', best >= 0 ? ST[best].hint : '');
    }
  }

  function updateStations(t, dt, px, pz) {
    // physics balls
    if (wld && sees(cam, PC.x, (PC.y0 + PC.y1) / 2, (PC.z0 + PC.z1) / 2, 2.5)) {
      const d = wld.data;
      const zc = (PC.z0 + PC.z1) / 2;
      balls.count = NB;
      for (let i = 0; i < NB; i++) {
        const bx = d[i * 3], by = d[i * 3 + 1], r = ballR[i] * 0.02;
        const depth = ((i * 0.618) % 1 - 0.5) * 0.24;
        balls.put(i, PC.x + depth, PC.y1 - by * 0.01, PC.z0 + bx * 0.01, 0, r, r, r, 0.9,
          ballCol[i * 3], ballCol[i * 3 + 1], ballCol[i * 3 + 2], 0.1);
      }
      void zc;
      balls.upload();
      balls.visible = true;
    } else balls.visible = false;

    // springs: retarget at ~8 Hz while you are near
    const sd = Math.hypot(px + 4.6, pz - 4.2);
    orbRetarget -= dt;
    if (sd < 5.5 && st.active && orbRetarget <= 0) {
      orbRetarget = 0.12;
      orbFollow = true;
      const yaw = ctl.yaw;
      for (let i = 0; i < NO; i++) {
        const o = orbS[i];
        const a = yaw + (i - (NO - 1) / 2) * 0.32;
        const tx = px - Math.sin(a) * 1.5, tz = pz - Math.cos(a) * 1.5;
        const ty = 1.35 + 0.28 * Math.sin(t * 1.6 + i);
        spring(o, { x: clampN(tx, -7.3, 7.3), y: ty, z: tz }, { stiffness: o.k, damping: o.d });
      }
    } else if (orbFollow && (sd >= 5.5 || !st.active)) {
      orbFollow = false;
      for (let i = 0; i < NO; i++) {
        const o = orbS[i];
        spring(o, { x: o.hx, y: o.hy, z: o.hz }, { stiffness: 18 + i * 3, damping: 5 + i * 0.4 });
      }
    }
    const wk = wake[0].v;
    orbs.count = NO;
    for (let i = 0; i < NO; i++) {
      const o = orbS[i];
      const fl = orbFollow ? 0 : 0.08 * Math.sin(t * 1.2 + i * 1.7);
      const sc = o.s * (1 + 0.15 * wk);
      const oc = ORB_PAL[i % 3];
      orbs.put(i, o.x, o.y + fl, o.z, 0, sc, sc, sc, 1.0, oc[0], oc[1], oc[2], 0.06 + 0.3 * wk);
    }
    orbs.upload();

    // blocks: sum of two waves
    if (sees(cam, 4.6, 1, -6.5, 3.2)) {
      blocks.visible = true;
      blocks.count = NBk;
      const a = waves[0], b = waves[1];
      const sa = a ? a.stride : 0, sb = b ? b.stride : 0;
      const da = a ? a.data : null, dbb = b ? b.data : null;
      const ya = a ? a.channels.y : 0, ssa = a ? a.channels.s : 0;
      const yb = b ? b.channels.y : 0, ssb = b ? b.channels.s : 0;
      const wk2 = wake[3].v;
      for (let i = 0; i < NBk; i++) {
        const gx = i % GB, gz = (i / GB) | 0;
        let y = 0, s = 0;
        if (da) { y += da[i * sa + ya]; s += da[i * sa + ssa]; }
        if (dbb) { y += dbb[i * sb + yb]; s += dbb[i * sb + ssb]; }
        const h = 0.14 + Math.max(0, y) * 0.3 + s * 0.3;
        const hue = Math.min(1, Math.max(0, y));
        blocks.put(i, 4.6 + (gx - 7.5) * 0.23, 0.27 + h / 2 + y * 0.32, -6.5 + (gz - 7.5) * 0.23, 0,
          0.19, h, 0.19, 0.5,
          0.22 + 0.78 * hue, 0.45 + 0.35 * hue, 0.72 - 0.45 * hue, 0.08 + 0.7 * hue * (0.4 + wk2));
      }
      blocks.upload();
    } else blocks.visible = false;

    // letters
    if (sees(cam, 0, 2, -25, 7)) {
      letters.visible = true;
      letters.count = NV;
      const wk3 = wake[5].v;
      for (let v = 0; v < NV; v++) {
        const li = vox[v * 3], L = letterS[li];
        const lx = vox[v * 3 + 1] * L.s, ly = vox[v * 3 + 2] * L.s;
        const c = Math.cos(L.ry), s = Math.sin(L.ry);
        const k = li / (WORD.length - 1);
        letters.put(v, L.cx + c * lx, 1.1 + VOX / 2 + ly + L.y + L.bob, -25.6 - s * lx, L.ry,
          VOX * 0.94 * L.s, VOX * 0.94 * L.s, VOX * 0.94 * L.s, 0.8,
          1.0 - 0.15 * k, 0.42 + 0.3 * k, 0.28 - 0.1 * k, 0.18 + 0.35 * wk3 + 0.4 * Math.min(1, L.y));
      }
      letters.upload();
    } else letters.visible = false;

    // bench bars
    bars.count = BENCH.length;
    for (let i = 0; i < BENCH.length; i++) {
      const h = Math.max(0.02, benchState[i].h * 1.6);
      bars.put(i, 5.15, 0.95 + h / 2, 2.75 + i * 0.72, 0, 0.3, h, 0.3, 0.7, 0.98, 0.6 - i * 0.05, 0.28 + i * 0.06, 0.15 + 0.3 * wake[1].v);
    }
    bars.upload();

    // cloud: auto-morph while idle
    cloud.idle += dt;
    if (st.ready && cloud.idle > 7 && cloud.mix >= 1) morph();
    cloud.spin += cloud.spinV.v * dt;
  }

  // ---------- rendering ----------
  function setCommon(p, mirror) {
    gl.useProgram(p.p);
    const u = p.u;
    if (u.uVP) gl.uniformMatrix4fv(u.uVP, false, cam.vp);
    if (u.uMirror) gl.uniform1f(u.uMirror, mirror);
    if (u.uEye) gl.uniform3f(u.uEye, cam.eye[0], cam.eye[1], cam.eye[2]);
    if (u.uSun) gl.uniform3f(u.uSun, SUN[0], SUN[1], SUN[2]);
    if (u.uFog) gl.uniform3f(u.uFog, FOG[0], FOG[1], FOG[2]);
    if (u.uTime) gl.uniform1f(u.uTime, time);
    if (u.uPx) gl.uniform1f(u.uPx, pxScale);
  }

  let time = 0;
  function drawScene(mirror) {
    setCommon(progMesh, mirror);
    // near, small things first so the big architecture fails the depth test early
    orbs.draw(); bars.draw(); balls.draw(); blocks.draw(); letters.draw();
    archCyl.draw(); archBox.draw(); windows.draw();
    // particles
    if (sees(cam, CLOUD_C[0], CLOUD_C[1], CLOUD_C[2], 2.4)) {
      setCommon(progPts, mirror);
      gl.uniform1f(progPts.u.uMix, cloud.mix);
      gl.uniform3f(progPts.u.uCenter, CLOUD_C[0], CLOUD_C[1], CLOUD_C[2]);
      gl.uniform1f(progPts.u.uSpin, cloud.spin);
      gl.bindVertexArray(ptsVao);
      gl.drawArrays(gl.POINTS, 0, mirror < 0 ? NP >> 2 : NP);
    }
    // benchmark panel
    if (sees(cam, 6.2, 2.2, 4.2, 2.2)) {
      setCommon(progTex, mirror);
      gl.uniform1f(progTex.u.uGlow, wake[1].v);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, panel.tex);
      gl.uniform1i(progTex.u.uTex, 0);
      gl.bindVertexArray(panel.vao);
      gl.disable(gl.CULL_FACE);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
      gl.enable(gl.CULL_FACE);
    }
  }

  function render() {
    updateCamera(cam);
    gl.viewport(0, 0, W, H);
    gl.disable(gl.SCISSOR_TEST);
    gl.clearColor(0.8, 0.76, 0.7, 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.enable(gl.CULL_FACE);

    // 1) mirrored scene, only below the horizon line
    const hy = -Math.tan(cam.pitch) / cam.tanH; // horizon in NDC
    const rows = Math.ceil(Math.min(1, Math.max(0, hy * 0.5 + 0.5 + 0.02)) * H);
    if (rows > 0) {
      gl.enable(gl.SCISSOR_TEST);
      gl.scissor(0, 0, W, rows);
      gl.frontFace(gl.CW);
      drawScene(-1);
      gl.frontFace(gl.CCW);
      gl.clear(gl.DEPTH_BUFFER_BIT);
      // 2) the floor, blended over its reflection
      setCommon(progFloor, 1);
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
      gl.bindVertexArray(floorVao);
      gl.disable(gl.CULL_FACE);
      gl.drawArrays(gl.TRIANGLES, 0, 6);
      gl.enable(gl.CULL_FACE);
      gl.disable(gl.BLEND);
      gl.disable(gl.SCISSOR_TEST);
    }
    // 3) the scene
    drawScene(1);
    // 4) light: shafts and dust, additive
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE);
    gl.depthMask(false);
    gl.disable(gl.CULL_FACE);
    setCommon(progBeam, 1);
    gl.uniform1f(progBeam.u.uBeam, 0.07);
    gl.bindVertexArray(beamVao);
    gl.drawArrays(gl.TRIANGLES, 0, beamCount);
    setCommon(progDust, 1);
    gl.bindVertexArray(dustVao);
    gl.drawArrays(gl.POINTS, 0, DUST);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.bindVertexArray(null);
  }

  // ---------- the contract ----------
  const api = {
    _ctl: ctl,
    resize(cssW, cssH, dpr) {
      const d = Math.min(dpr || 1, 1.5);
      W = Math.max(1, Math.round(cssW * d));
      H = Math.max(1, Math.round(cssH * d));
      if (canvas.width !== W || canvas.height !== H) { canvas.width = W; canvas.height = H; }
      cam.aspect = W / H;
      cam.fov = cssW < cssH ? 1.35 : 1.12;
      pxScale = (H * 0.5) / Math.tan(cam.fov / 2);
    },
    preview(t, dt, k) {
      time = t;
      const e = k * k * (3 - 2 * k);
      cam.eye[0] = Math.sin(t * 0.21) * 0.25 * (1 - e);
      cam.eye[1] = 2.3 - 0.6 * e + Math.sin(t * 0.5) * 0.04;
      cam.eye[2] = 15.4 - 3.4 * e;
      cam.yaw = Math.sin(t * 0.17) * 0.05 * (1 - e);
      cam.pitch = -0.02 + 0.03 * (1 - e);
      ctl.place(cam.eye, cam.yaw, cam.pitch);
      updateCamera(cam);
      if (st.ready) updateStations(t, Math.min(dt, 0.05), cam.eye[0], cam.eye[2]);
      render();
    },
    enter() {
      if (st.active) return;
      st.active = true;
      ctl.enable();
    },
    exit() {
      if (!st.active) return;
      st.active = false;
      ctl.disable();
      if (st.near >= 0) { wakeStation(st.near, false); st.near = -1; }
    },
    frame(t, dt) {
      time = t;
      dt = Math.min(dt, 0.05);
      ctl.update(t, dt);
      cam.eye[0] = ctl.pos[0]; cam.eye[1] = ctl.pos[1]; cam.eye[2] = ctl.pos[2];
      cam.yaw = ctl.yaw; cam.pitch = ctl.pitch;
      updateCamera(cam);
      if (st.ready) {
        updateNear(ctl.pos[0], ctl.pos[2]);
        updateStations(t, dt, ctl.pos[0], ctl.pos[2]);
      }
      render();
    },
  };
  return api;
}

function clampN(v, a, b) { return v < a ? a : v > b ? b : v; }

// ---------- particle shapes ----------
function makeShapes(n) {
  const out = [];
  const rnd = mulberry(7);
  // 1. sphere shell with a soft inner glow
  const a = new Float32Array(n * 3);
  const ga = Math.PI * (3 - Math.sqrt(5));
  for (let i = 0; i < n; i++) {
    const y = 1 - (i / (n - 1)) * 2, r = Math.sqrt(1 - y * y), th = ga * i;
    const R = 1.75 * (0.96 + 0.04 * rnd());
    a[i * 3] = Math.cos(th) * r * R; a[i * 3 + 1] = y * R; a[i * 3 + 2] = Math.sin(th) * r * R;
  }
  out.push(a);
  // 2. torus knot (2,3)
  const b = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const t = (i / n) * Math.PI * 2;
    const r = 0.95 + 0.42 * Math.cos(3 * t);
    const cx = r * Math.cos(2 * t), cy = 0.42 * Math.sin(3 * t) * 1.6, cz = r * Math.sin(2 * t);
    const u = rnd() * Math.PI * 2, rr = 0.16 * Math.sqrt(rnd());
    b[i * 3] = cx + Math.cos(u) * rr; b[i * 3 + 1] = cy + Math.sin(u) * rr; b[i * 3 + 2] = cz + Math.sin(u + 1.3) * rr;
  }
  out.push(b);
  // 3. spiral galaxy, tilted
  const c = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const arm = i % 3, f = rnd();
    const rad = 0.08 + 1.75 * Math.pow(f, 0.7);
    const ang = arm * (Math.PI * 2 / 3) + rad * 2.6 + (rnd() - 0.5) * 0.45 / (0.4 + rad);
    const x = Math.cos(ang) * rad, z = Math.sin(ang) * rad, y = (rnd() - 0.5) * 0.18 * (1.8 - rad);
    c[i * 3] = x; c[i * 3 + 1] = y * 0.9 + z * 0.35; c[i * 3 + 2] = z * 0.93 - y * 0.35;
  }
  out.push(c);
  // 4. cube lattice shell
  const d = new Float32Array(n * 3);
  for (let i = 0; i < n; i++) {
    const face = i % 6, u = rnd() * 2 - 1, v = rnd() * 2 - 1, s = 1.15;
    const g = (q) => Math.round(q * 6) / 6;
    const onLine = rnd() < 0.7;
    const uu = onLine ? (rnd() < 0.5 ? g(u) : u) : u, vv = onLine ? (Math.abs(uu - g(u)) < 1e-6 ? v : g(v)) : v;
    let x, y, z;
    const w = face < 3 ? 1 : -1;
    if (face % 3 === 0) { x = w; y = uu; z = vv; } else if (face % 3 === 1) { x = uu; y = w; z = vv; } else { x = uu; y = vv; z = w; }
    d[i * 3] = x * s; d[i * 3 + 1] = y * s; d[i * 3 + 2] = z * s;
  }
  out.push(d);
  return out;
}

function mulberry(s) {
  return () => {
    s |= 0; s = (s + 0x6d2b79f5) | 0;
    let t = Math.imul(s ^ (s >>> 15), 1 | s);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// ---------- benchmark panel ----------
function makePanel(gl, rows) {
  const cv = document.createElement('canvas');
  cv.width = 1024; cv.height = 1024;
  const c = cv.getContext('2d');
  const g = c.createLinearGradient(0, 0, 0, 1024);
  g.addColorStop(0, '#fbf6ee'); g.addColorStop(1, '#efe4d2');
  c.fillStyle = g; c.fillRect(0, 0, 1024, 1024);
  c.strokeStyle = '#c9a063'; c.lineWidth = 10; c.strokeRect(14, 14, 996, 996);
  c.fillStyle = '#2a2420';
  c.font = '700 64px system-ui, -apple-system, Segoe UI, sans-serif';
  c.fillText('Benchmarks', 64, 128);
  c.font = '400 30px system-ui, -apple-system, Segoe UI, sans-serif';
  c.fillStyle = '#7a6a58';
  c.fillText('Measured with packages/anikit/scripts/*.mjs · README.md', 64, 178);
  for (let i = 0; i < rows.length; i++) {
    const y = 290 + i * 150;
    c.fillStyle = '#c0622f';
    c.font = '800 76px system-ui, -apple-system, Segoe UI, sans-serif';
    c.fillText(rows[i][2], 64, y);
    c.fillStyle = '#3a322b';
    c.font = '400 34px system-ui, -apple-system, Segoe UI, sans-serif';
    c.fillText(rows[i][0], 64, y + 52);
  }
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, cv);
  gl.generateMipmap(gl.TEXTURE_2D);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  // a standing panel behind the plinth, facing -x (toward the hall's centre)
  const x = 6.25, z0 = 2.2, z1 = 6.2, y0 = 1.05, y1 = 5.05;
  const v = new Float32Array([
    x, y0, z0, 0, 1, x, y0, z1, 1, 1, x, y1, z1, 1, 0,
    x, y0, z0, 0, 1, x, y1, z1, 1, 0, x, y1, z0, 0, 0,
  ]);
  const vao = gl.createVertexArray();
  gl.bindVertexArray(vao);
  const b = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, b);
  gl.bufferData(gl.ARRAY_BUFFER, v, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0); gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 20, 0);
  gl.enableVertexAttribArray(1); gl.vertexAttribPointer(1, 2, gl.FLOAT, false, 20, 12);
  gl.bindVertexArray(null);
  return { tex, vao };
}

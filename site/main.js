// Anikit landing page — every animation here runs on anikit.
import {
  ready, engine, animate, set, timeline, stagger, spring, scrollTrigger, scrollTriggerBatch, splitText, effects,
  world, scene3d, batch, tilt, flip, draggable, inView, hover, observer, record, physics, easing, pathThrough,
  noise2, circle, star, rect, polygon, effectNames,
} from '../packages/anikit/dist/anikit.js';
import { cosmos, bulletTime } from './cosmos.js';
import { gravity } from './gravity.js';

await ready;
engine.reducedMotion = 'user';
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const dpr = Math.min(2, devicePixelRatio || 1);
const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;

/** Run `fn` each frame only while `el` is on screen. */
function whileVisible(el, fn) {
  let off = null;
  inView(el, () => {
    off = engine.onFrame(fn);
    return () => off?.();
  }, { margin: '100px' });
}

// ---------------------------------------------------------------------------
// Chrome: progress bar, nav, brand mark
// ---------------------------------------------------------------------------
scrollTrigger({ animation: animate('#progress', { scaleX: [0, 1] }, { ease: 'linear', autoplay: false }), scrub: true, start: 'top', end: 'bottom' });
animate('#nav', { y: [-80, 0], opacity: [0, 1] }, { duration: 0.9, delay: 0.3, ease: 'outCubic' });
let navHidden = false;
observer({
  type: 'scroll', tolerance: 24,
  onDown: () => !navHidden && scrollY > 400 && ((navHidden = true), animate('#nav', { y: -90 }, { duration: 0.4, ease: 'inCubic' })),
  onUp: () => navHidden && ((navHidden = false), animate('#nav', { y: 0 }, { duration: 0.5, ease: 'outCubic' })),
});
const markShapes = [polygon([[16, 3], [29, 29], [3, 29]]), circle(16, 16, 13), star(16, 16, 5, 14, 6), rect(4, 4, 24, 24, 6)];
let markI = 0;
hover('.brand', () => {
  markI = (markI + 1) % markShapes.length;
  animate('#mark', { d: markShapes[markI] }, { duration: 0.6, ease: 'spring(bounce: 0.35, duration: 0.5)' });
});

// ---------------------------------------------------------------------------
// The cosmos (fixed behind the page), bullet time, gravity and the HUD
// ---------------------------------------------------------------------------
const cos = cosmos({ reduced });
const bt = bulletTime({ glow: cos.glow, buttons: [$('#hud-slow'), $('#slow-cta')], vignette: $('#vignette') });
const grav = gravity({
  onChange: (on, n) => {
    $('#hud-gravity').classList.toggle('on', on);
    $('#gravity-cta').textContent = on ? 'Restore the page' : 'Turn on gravity';
    hudNote = on ? `${n} bodies · drag to throw · double-click for a shockwave` : '';
  },
});
let hudNote = '';
$('#hud-gravity').addEventListener('click', () => grav.toggle());
$('#gravity-cta').addEventListener('click', () => grav.toggle());
addEventListener('keydown', (e) => e.key === 'Escape' && grav.active && grav.off());
{
  const out = $('#hud-stats');
  const ms = $('#m1-ms');
  $('#m1-count').textContent = cos.count.toLocaleString();
  setInterval(() => {
    const st = cos.stats;
    const sp = bt.speed;
    out.textContent = hudNote || `${cos.count.toLocaleString()} particles · ${st.ms.toFixed(1)} ms · ${st.fps} fps${sp < 0.99 ? ` · ${sp.toFixed(2)}× time` : ''}`;
    ms.textContent = st.ms ? st.ms.toFixed(1) : 'a few';
  }, 250);
}
cos.playIntro();

// Moments: staggered reveals
for (const m of $$('.moment-inner')) {
  const kids = Array.from(m.children);
  set(kids, { opacity: 0, y: 30 });
  inView(m, () => {
    animate(kids, { opacity: 1, y: 0 }, { duration: 1, ease: 'outCubic', delay: stagger(0.12) });
  }, { amount: 0.3 });
}

// Headline, tagline, buttons
{
  const { chars } = splitText('#headline', { type: 'words,chars', charClass: 'char', wordClass: 'gw' });
  $('#headline').dataset.gsplit = '1';
  animate(chars, { y: [90, 0], rotate: (_e, i) => [(i % 2 ? 1 : -1) * 12, 0], opacity: [0, 1] },
    { duration: 1.1, ease: 'spring(bounce: 0.25, duration: 0.9)', delay: stagger(0.035, { start: 0.25 }) });
  animate('.eyebrow, .lede, .hero-actions', { y: [16, 0], opacity: [0, 1] }, { duration: 0.9, ease: 'outCubic', delay: stagger(0.1, { start: 0.7 }) });
  const lines = [
    'An animation engine written in Rust and running in WebAssembly.',
    'Tweens, timelines, springs, physics and 3D. One engine.',
    'Frame-exact recording. Inspectable state. Agent-ready.',
    'Seven to twenty-seven times faster physics than matter.js.',
  ];
  let li = 0;
  setInterval(() => {
    if (document.hidden) return;
    li = (li + 1) % lines.length;
    animate('#tagline', { text: { value: lines[li], type: 'scramble', chars: 'lower' } }, { duration: 1.1, ease: 'linear' });
  }, 4200);
  // Magnetic buttons.
  for (const b of $$('.magnetic')) {
    b.addEventListener('pointermove', (e) => {
      const r = b.getBoundingClientRect();
      spring(b, { x: (e.clientX - r.left - r.width / 2) * 0.25, y: (e.clientY - r.top - r.height / 2) * 0.35 }, { stiffness: 260, damping: 18 });
    });
    b.addEventListener('pointerleave', () => spring(b, { x: 0, y: 0 }, { stiffness: 200, damping: 12 }));
  }
  animate('.scroll-hint span', { y: [0, 12], opacity: [1, 0] }, { duration: 1.4, ease: 'inOutSine', repeat: Infinity });
}

// Section headings reveal as they enter.
for (const head of $$('.section-head')) set(head.children, { opacity: 0 });
scrollTriggerBatch('.section-head', {
  start: 'top 88%', once: true,
  onEnter: (st) => animate(st.trigger.children, { y: [28, 0], opacity: [0, 1] }, { duration: 0.9, ease: 'outCubic', delay: stagger(0.08) }),
});

// ---------------------------------------------------------------------------
// Performance numbers
// ---------------------------------------------------------------------------
set('.stat-card', { opacity: 0 });
scrollTrigger({
  trigger: '.stats', start: 'top 85%', once: true,
  onEnter: () => {
    animate('.stat-card', { y: [40, 0], opacity: [0, 1] }, { duration: 0.8, ease: 'outCubic', delay: stagger(0.09) });
    $$('.stat-card').forEach((card, i) => {
      const c = $('.count', card);
      const to = Number(c.dataset.to);
      const d = Number(c.dataset.decimals || 0);
      effects.countUp(c, { to, duration: 1.8, delay: 0.2 + i * 0.09, locale: 'en-US', format: { minimumFractionDigits: d, maximumFractionDigits: d } });
      animate($('.bar i', card), { scaleX: [0, Number($('.bar i', card).dataset.w)] }, { duration: 1.6, delay: 0.3 + i * 0.09, ease: 'outExpo' });
    });
  },
});

// ---------------------------------------------------------------------------
// Easing lab: curve morphs between easings
// ---------------------------------------------------------------------------
{
  const list = [
    'outBack', 'spring.wobbly', 'bounce.out', 'elastic.out(1, 0.35)', 'inOutExpo', 'steps(6)',
    'custom(M0,0 C0.2,1.4 0.5,-0.4 1,1)', 'rough(16, 1.4, 7, outQuad)', 'mirror(inOutSine)', 'anticipate',
    'spring(bounce: 0.6, duration: 0.8)', 'slowmo(0.6, 0.8)', 'ease-in-out', 'blend(inQuad, outBounce, 0.5)',
  ];
  const chips = $('#ease-chips');
  const curvePath = (name) => {
    const f = easing(name);
    let d = '';
    for (let k = 0; k <= 96; k++) {
      const t = k / 96;
      d += (k ? 'L' : 'M') + (t * 300).toFixed(2) + ' ' + (200 - f(t) * 200).toFixed(2);
    }
    return d;
  };
  const ball = $('#ease-ball');
  const pick = (name, btn) => {
    $$('.chip', chips).forEach((c) => c.classList.toggle('active', c === btn));
    animate('#ease-curve', { d: curvePath(name) }, { duration: 0.7, ease: 'inOutCubic' });
    const w = $('.ease-track').clientWidth - 34;
    animate(ball, { x: [0, w] }, { duration: 1.3, ease: name, repeat: Infinity, repeatDelay: 0.6 });
    animate('#ease-code', { text: { value: `ease: '${name}'`, type: 'scramble', chars: 'symbols' } }, { duration: 0.5, ease: 'linear' });
  };
  list.forEach((name, i) => {
    const b = document.createElement('button');
    b.className = 'chip';
    b.textContent = name;
    b.addEventListener('click', () => pick(name, b));
    chips.appendChild(b);
    if (i === 0) {
      $('#ease-curve').setAttribute('d', curvePath('linear'));
      pick(name, b);
    }
  });
  set('.chip', { opacity: 0 });
  scrollTrigger({ trigger: '#ease', start: 'top 75%', once: true, onEnter: () => animate('.chip', { y: [12, 0], opacity: [0, 1] }, { duration: 0.5, delay: stagger(0.025), ease: 'outCubic' }) });
}

// ---------------------------------------------------------------------------
// Pinned story: a timeline scrubbed by scroll, snapping to labels
// ---------------------------------------------------------------------------
{
  const shape = $('#story-shape');
  shape.setAttribute('d', circle(200, 200, 30));
  const dots = $('#story-dots');
  for (let i = 0; i < 24; i++) dots.appendChild(document.createElement('i'));
  const dotEls = $$('i', dots);
  const W = () => $('#story-stage').clientWidth;
  const ringX = (i, r) => Math.cos((i / 24) * Math.PI * 2) * r;
  const ringY = (i, r) => Math.sin((i / 24) * Math.PI * 2) * r;
  const tl = timeline({ autoplay: false, defaults: { ease: 'inOutCubic', duration: 1 } })
    .label('build', 0)
    .add(shape, { d: circle(200, 200, 120) }, { ease: 'outBack' }, 0)
    .add(dotEls, { x: (_e, i) => [(i % 2 ? 1 : -1) * 600, ringX(i, W() * 0.46)], y: (_e, i) => [ringY(i, 400), ringY(i, W() * 0.46)], opacity: [0, 1] }, { delay: stagger(0.02) }, 0)
    .label('morph')
    .add(shape, { d: [null, star(200, 200, 6, 150, 70), star(200, 200, 12, 160, 120)] })
    .label('paint')
    .add('#g1 stop:first-child', { stopColor: ['#7c8cff', '#ffc46b'] })
    .add('#g1 stop:last-child', { stopColor: ['#ff7ab6', '#4de0c2'] }, {}, '<')
    .add(dotEls, { scale: [1, 2.2], backgroundColor: ['#eceef4', '#ffc46b'] }, { delay: stagger(0.01, { from: 'center' }) }, '<')
    .label('orbit')
    .add('#story-stage', { transform: ['rotateX(0deg) rotateY(0deg)', 'rotateX(58deg) rotateZ(-28deg) scale(0.92)'] })
    .add(dotEls, { y: (_e, i) => ringY(i, W() * 0.46) - 60 }, { delay: stagger(0.015) }, '<');
  const steps = $$('#story-steps li');
  scrollTrigger({
    trigger: '#story', start: 'top top', end: '+=2600', pin: '#story-pin', scrub: 0.6, animation: tl,
    snap: { to: 'labels', duration: 0.6, ease: 'inOutCubic' },
    onUpdate: () => {
      const cur = tl.currentLabel ?? 'build';
      steps.forEach((li) => li.classList.toggle('on', li.dataset.label === cur));
    },
  });
}

// ---------------------------------------------------------------------------
// SVG cards
// ---------------------------------------------------------------------------
{
  const blobShape = (seed) => pathThrough(Array.from({ length: 7 }, (_, k) => {
    const a = (k / 7) * Math.PI * 2, r = 62 + noise2(k * 0.7, seed) * 22;
    return [100 + Math.cos(a) * r, 100 + Math.sin(a) * r];
  }), { closed: true });
  const blob = $('#blob');
  blob.setAttribute('d', star(100, 100, 5, 78, 34));
  animate(blob, {
    d: [null, circle(100, 100, 70), rect(32, 32, 136, 136, 28), blobShape(3), star(100, 100, 9, 80, 56), star(100, 100, 5, 78, 34)],
    fill: ['#7c8cff', '#ff7ab6', '#ffc46b', '#4de0c2', '#c9cfff', '#7c8cff'],
  }, { duration: 9, ease: 'inOutCubic', repeat: Infinity });
  animate('#draw-path', { draw: [0, 1] }, { duration: 2.2, ease: 'inOutCubic', repeat: Infinity, yoyo: true, repeatDelay: 0.5 });
  animate('#rider', { path: { path: '#draw-path' } }, { duration: 2.2, ease: 'inOutCubic', repeat: Infinity, yoyo: true, repeatDelay: 0.5 });
  const spline = $('#spline');
  const sdots = $('#spline-dots');
  for (let k = 0; k < 7; k++) sdots.appendChild(document.createElementNS('http://www.w3.org/2000/svg', 'circle')).setAttribute('r', '3.5');
  const cs = $$('circle', sdots);
  whileVisible(spline, (t) => {
    const pts = cs.map((c, k) => {
      const a = (k / 7) * Math.PI * 2;
      const r = 58 + noise2(k * 3.1, t / 1400) * 30;
      const p = [100 + Math.cos(a) * r, 100 + Math.sin(a) * r];
      c.setAttribute('cx', p[0].toFixed(1));
      c.setAttribute('cy', p[1].toFixed(1));
      return p;
    });
    spline.setAttribute('d', pathThrough(pts, { closed: true }));
  });
}

// ---------------------------------------------------------------------------
// Physics
// ---------------------------------------------------------------------------
{
  // DOM letters as bodies.
  const stage = $('#letters-stage');
  const word = 'ANIKIT×WASM';
  [...word].forEach((ch, i) => {
    const d = document.createElement('div');
    d.className = 'letter' + (i === 6 ? ' accent' : '');
    d.textContent = ch;
    d.style.left = 30 + ((i * 76) % (stage.clientWidth - 90)) + 'px';
    d.style.top = 20 + Math.floor((i * 76) / (stage.clientWidth - 90)) * 72 + 'px';
    stage.appendChild(d);
  });
  const letters = world({ container: stage, gravity: [0, 1800], restitution: 0.45, friction: 0.08, substeps: 6 });
  letters.bind('.letter', { rotate: true });
  letters.dragging();
  letters.pause();
  inView(stage, () => {
    letters.play();
  }, { amount: 0.4 });
  stage.addEventListener('dblclick', (e) => {
    const r = stage.getBoundingClientRect();
    letters.impulse(e.clientX - r.left, e.clientY - r.top, 260, 1400);
  });

  // Canvas field.
  const fieldStage = $('#field-stage');
  const canvas = $('#field');
  const ctx = canvas.getContext('2d');
  const size = () => {
    canvas.width = fieldStage.clientWidth * dpr;
    canvas.height = fieldStage.clientHeight * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  };
  size();
  addEventListener('resize', size);
  const field = world({ container: fieldStage, gravity: [0, 600], restitution: 0.25, friction: 0.04, autoStep: false, substeps: 3 });
  const n = innerWidth < 700 ? 500 : 950;
  const fw = fieldStage.clientWidth;
  field.addMany(Array.from({ length: n }, (_, i) => ({ x: 20 + Math.random() * (fw - 40), y: Math.random() * 300, r: 2.5 + (i % 7) * 0.9 })));
  field.rope([[fw * 0.7, 0], [fw * 0.7, 170]], { segments: 18, pinStart: true, radius: 3.5 });
  field.pointer({ mode: 'repel', radius: 110, strength: 9000 });
  field.dragging({ hitSlop: 6 });
  const palette = ['#7c8cff', '#ff7ab6', '#ffc46b', '#c9cfff', '#4de0c2', '#5b6274', '#9aa3ff'];
  const colorOf = (i) => palette[i % palette.length];
  let acc = 0, k = 0, lastT = performance.now();
  whileVisible(fieldStage, (_t, dt) => {
    const t0 = performance.now();
    field.step(Math.min(dt, 33) / 1000);
    acc += performance.now() - t0;
    k++;
    field.draw(ctx, { color: colorOf, links: 'rgba(255,255,255,0.35)' });
    if (performance.now() - lastT > 800) {
      $('#field-stat').textContent = `${field.count.toLocaleString()} bodies · ${(acc / k).toFixed(2)} ms/step · ${field.pairTests.toLocaleString()} pair tests`;
      acc = k = 0;
      lastT = performance.now();
    }
  });
}

// ---------------------------------------------------------------------------
// 3D: tilt + transform interpolation, FLIP layout
// ---------------------------------------------------------------------------
{
  const host = $('#flip-cards');
  const data = [
    ['Transforms', 'Click to flip', 'rotateY(180deg)', 'decompose → slerp → recompose'],
    ['Springs', 'Tilt follows your pointer', 'spring(bounce: 0.35)', 'velocity-preserving retargets'],
    ['Matrices', 'Any transform list', 'translateZ · rotate3d · skew', 'interpolated in WebAssembly'],
  ];
  for (const [t, s, c, b] of data) {
    const card = document.createElement('div');
    card.className = 'flip';
    card.innerHTML = `<div class="flip-inner" style="position:absolute;inset:0;transform-style:preserve-3d">
      <div class="face front"><div class="cube-wrap"><div class="cube">${'<i></i>'.repeat(6)}</div></div><h4>${t}</h4><p>${s}</p></div>
      <div class="face back"><h4><code>${c}</code></h4><p>${b}</p></div></div>`;
    host.appendChild(card);
    let flipped = false;
    card.addEventListener('click', () => {
      flipped = !flipped;
      animate($('.flip-inner', card), { transform: flipped ? 'rotateY(180deg)' : 'rotateY(0deg)' }, { duration: 0.9, ease: 'spring(bounce: 0.3, duration: 0.8)' });
    });
  }
  // Each card's cube shows a different way of driving 3D transforms.
  const cubes = $$('.cube', host);
  set(cubes, { rotateX: -24, rotateY: 30 });
  animate(cubes[0], { rotateY: 390 }, { duration: 9, ease: 'linear', repeat: -1 });
  animate(cubes[1], { rotateX: [-24, 24], rotateY: [30, -30] }, { duration: 2.6, ease: 'inOutSine', repeat: -1, yoyo: true });
  animate(cubes[2], {
    transform: [
      'rotateX(-24deg) rotateY(30deg)',
      'rotateX(40deg) rotateY(-50deg) scale(0.85)',
      'rotate3d(1, 1, 0, 70deg) translateZ(10px)',
      'rotateX(-24deg) rotateY(30deg)',
    ],
  }, { duration: 6, ease: 'inOutCubic', repeat: -1 });
  tilt('.flip', { max: 10, scale: 1.02 });
  set('.flip', { opacity: 0 });
  scrollTrigger({ trigger: '#flip-cards', start: 'top 85%', once: true, onEnter: () => effects.flipIn('.flip', { stagger: 0.1, axis: 'x' }) });

  const tiles = $('#tiles');
  for (let i = 0; i < 16; i++) {
    const t = document.createElement('div');
    t.className = 'tile' + (i % 5 === 0 ? ' hot' : '');
    t.textContent = String(i + 1).padStart(2, '0');
    tiles.appendChild(t);
  }
  $('#shuffle').addEventListener('click', () => flip('.tile', () => {
    const kids = $$('.tile');
    for (let i = kids.length - 1; i > 0; i--) tiles.appendChild(kids.splice(Math.floor(Math.random() * (i + 1)), 1)[0]);
  }, { duration: 0.7, ease: 'spring(bounce: 0.2, duration: 0.6)', delay: stagger(0.015) }));
  $('#relayout').addEventListener('click', () => flip('.tile', () => tiles.classList.toggle('wide'), { duration: 0.8, ease: 'spring(bounce: 0.15, duration: 0.7)' }));
}

// ---------------------------------------------------------------------------
// Effects gallery
// ---------------------------------------------------------------------------
{
  const grid = $('#effects-grid');
  $('#fx-count').textContent = String(effectNames().length);
  const pick = ['fadeIn', 'slideIn', 'zoomIn', 'pop', 'bounceIn', 'dropIn', 'flipIn', 'rotateIn', 'blurIn', 'revealClip', 'shake',
    'headShake', 'wobble', 'jello', 'rubberBand', 'swing', 'tada', 'pulse', 'heartbeat', 'flash', 'float', 'spin', 'breathe', 'wiggleLoop']
    .filter((n) => effectNames().includes(n));
  for (const name of pick) {
    const tile = document.createElement('div');
    tile.className = 'fx';
    tile.innerHTML = `<div class="fx-box"></div><span class="fx-name">${name}</span>`;
    grid.appendChild(tile);
    const box = $('.fx-box', tile);
    let running = null;
    const run = () => {
      running?.kill();
      set(box, { x: 0, y: 0, rotate: 0, scale: 1, opacity: 1, skewX: 0, skewY: 0, scaleX: 1, scaleY: 1, rotateX: 0, rotateY: 0 });
      box.style.filter = '';
      box.style.clipPath = '';
      const loop = ['float', 'spin', 'breathe', 'wiggleLoop'].includes(name);
      running = effects[name](box, loop ? { repeat: 1 } : {});
    };
    tile.addEventListener('pointerenter', run);
    tile.addEventListener('click', run);
  }
  set('.fx', { opacity: 0 });
  scrollTrigger({
    trigger: '#effects-grid', start: 'top 85%', once: true,
    onEnter: () => animate('.fx', { scale: [0.9, 1], opacity: [0, 1] }, { duration: 0.6, ease: 'outBack', delay: stagger(0.03, { grid: [6, 4], from: 'center' }) }),
  });
}

// ---------------------------------------------------------------------------
// Feel: draggable, spring follow, rotary knob
// ---------------------------------------------------------------------------
{
  set('#puck', { x: 53 * 2, y: 53 * 2 });
  draggable('#puck', { bounds: '#drag-zone', inertia: true, snap: { x: 53, y: 53 }, edgeResistance: 0.25 });
  const zone = $('#follow-zone');
  zone.addEventListener('pointermove', (e) => {
    const r = zone.getBoundingClientRect();
    const p = { x: e.clientX - r.left, y: e.clientY - r.top };
    spring('#follower', p, { stiffness: 90, damping: 9 });
    spring('#follower2', p, { stiffness: 600, damping: 30 });
  });
  set(['#follower', '#follower2'], { x: 160, y: 150 });
  const out = $('#knob-value');
  const show = (d) => (out.textContent = String(Math.round(((d.rotation + 140) / 280) * 100)));
  draggable('#knob', { type: 'rotate', inertia: true, bounds: { minRotation: -140, maxRotation: 140 }, onDrag: show, onThrowUpdate: show });
}

// ---------------------------------------------------------------------------
// Agents: live inspect(), typed code, record to GIF
// ---------------------------------------------------------------------------
{
  const pre = $('#inspect-out');
  let lastT = 0;
  whileVisible(pre, (t) => {
    if (t - lastT < 250) return;
    lastT = t;
    const all = engine.inspect().filter((s) => s.state === 'running');
    const live = all.filter((s) => s.progress > 0 && s.progress < 1);
    const snap = (live.length ? live : all).slice(0, 7);
    const q = (v) => (typeof v === 'string' ? `'${v}'` : Array.isArray(v) ? `[${v.map(q).join(', ')}]` : typeof v === 'number' ? +v.toFixed(2) : v);
    const line = (s) => '  { ' + ['id', 'type', 'target', 'props', 'progress'].filter((k) => s[k] !== undefined).map((k) => `${k}: ${q(s[k])}`).join(', ') + ' }';
    pre.textContent = `// ${all.length} running\n[\n${snap.map(line).join(',\n')}${all.length > snap.length ? ',\n  …' : ''}\n]`;
  });
  const code = `import { testing, engine, record } from 'anikit';

testing.useFakeTime();
animate(card, { x: 100 }, { duration: 1, ease: 'linear' });
testing.advance(0.5);
// card.style.transform === 'translate(50px,0px)'

engine.inspect();
// [{ type: 'tween', target: 'div#card',
//    props: ['x'], progress: 0.5, … }]

const gif = await record('#hero', { duration: 2 });
// $ npx anikit-record page.html --out clip.mp4`;
  scrollTrigger({ trigger: '#code-type', start: 'top 80%', once: true, onEnter: () => animate('#code-type', { text: code }, { duration: 4, ease: 'linear' }) });

  // Orbiting record demo.
  const orbs = $$('.rd-orb');
  orbs.forEach((o, i) => {
    animate(o, { path: { path: circle(0, 0, 46 + i * 18) } }, { duration: 3 + i, ease: 'linear', repeat: Infinity });
  });
  const btn = $('#record-btn');
  btn.addEventListener('click', async () => {
    btn.disabled = true;
    btn.textContent = 'Recording…';
    const t0 = performance.now();
    const rec = await record('#record-demo', { duration: 2, fps: 24, scale: 1, onProgress: (p) => (btn.textContent = `Recording… ${Math.round(p * 100)}%`) });
    const img = document.createElement('img');
    img.src = rec.url;
    img.alt = 'Recorded GIF';
    $('#record-demo').replaceWith(img);
    $('#record-out').innerHTML = `${rec.frames} frames · ${(rec.blob.size / 1024).toFixed(0)} KB GIF · ${((performance.now() - t0) / 1000).toFixed(1)} s · <a href="${rec.url}" download="anikit.gif"><u>download</u></a>`;
    btn.textContent = 'Recorded ✓';
    effects.pop(img);
  });
}

// ---------------------------------------------------------------------------
// Install: title, copy, confetti physics
// ---------------------------------------------------------------------------
{
  const { chars } = splitText('#install-title', { type: 'words,chars', charClass: 'char', wordClass: 'gw' });
  $('#install-title').dataset.gsplit = '1';
  set(chars, { opacity: 0 });
  scrollTrigger({
    trigger: '#install', start: 'top 75%', once: true,
    onEnter: () => animate(chars, { y: [60, 0], opacity: [0, 1], rotateX: [-80, 0] }, { duration: 0.9, ease: 'outBack', delay: stagger(0.03) }),
  });
  $('#copy').addEventListener('click', async (e) => {
    try { await navigator.clipboard.writeText('npm install anikit'); } catch {}
    e.currentTarget.textContent = 'Copied';
    effects.pop(e.currentTarget, { duration: 0.35 });
  });
  const section = $('#install');
  const canvas = $('#confetti');
  const ctx = canvas.getContext('2d');
  let bits = [];
  const fit = () => {
    canvas.width = section.clientWidth * dpr;
    canvas.height = section.clientHeight * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  };
  fit();
  addEventListener('resize', fit);
  const colors = ['#7c8cff', '#ff7ab6', '#ffc46b', '#4de0c2', '#eceef4'];
  section.addEventListener('click', (e) => {
    if (reduced || e.target.closest('button')) return;
    const r = section.getBoundingClientRect();
    const fresh = Array.from({ length: 70 }, (_, i) => ({ x: e.clientX - r.left, y: e.clientY - r.top, rot: 0, c: colors[i % colors.length], s: 4 + Math.random() * 5 }));
    for (const b of fresh) {
      const a = Math.random() * Math.PI * 2;
      const v = 300 + Math.random() * 700;
      physics(b, { x: { velocity: Math.cos(a) * v }, y: { velocity: Math.sin(a) * v - 400, acceleration: 1400, max: section.clientHeight - 6 }, rot: { velocity: (Math.random() - 0.5) * 900 } }, { friction: 0.9, restitution: 0.4, life: 3 });
    }
    bits = bits.concat(fresh).slice(-600);
  });
  engine.onFrame(() => {
    if (!bits.length) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (const b of bits) {
      ctx.save();
      ctx.translate(b.x, b.y);
      ctx.rotate((b.rot * Math.PI) / 180);
      ctx.fillStyle = b.c;
      ctx.fillRect(-b.s / 2, -b.s / 4, b.s, b.s / 2);
      ctx.restore();
    }
  });
}

// Expose for tinkering in devtools.
window.anikit = await import('../packages/anikit/dist/anikit.js');

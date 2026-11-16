// A starter anikit film. Render it:
//   npx anikit-render film.mjs --still 1.5 --out still.png     (one frame, a few seconds)
//   npx anikit-render film.mjs --preview 15 --out preview/     (contact frames, seconds)
//   npx anikit-render film.mjs --out film.mp4 --workers 4      (1080p60, frame-exact)
//
// Rules that keep films deterministic and fast:
// - Start tweens/simulations in enter(); advance simulations in step() with f.dt.
// - draw() must only read state (it can run several times per frame for motion blur).
// - Math.random is seeded by anikit-render; avoid Date.now()/performance.now() in choreography.
export default async function film({ anikit: A, node }) {
  const { composition, scene3d, renderer3d, mesh, instances, pointCloud, glow, typeFx, ease } = A;
  const W = 1920, H = 1080;
  // node.registerFont('/path/to/Font.otf', 'Display');   // fonts for ctx.font / textPoints / typeFx

  const film = composition({
    width: W, height: H, fps: 60, duration: 6,
    motionBlur: { samples: 2, shutter: 0.5 },
    post: { aa: 1, bloom: 0.8, bloomThreshold: 0.55, streaks: 0.35, vignette: 0.4, grain: 4, contrast: 1.06 },
  });
  film.impact(1.0, { shake: 24, flash: '#ffffff', flashOpacity: 0.5, aberration: 6 });

  // Solid 3D: 2,000 cubes on a camera that orbits. Dark chrome bodies; the colour
  // lives in light (glowing edges, a travelling pulse, a few emissive accents).
  const cam = scene3d({ fov: 45, width: W, height: H, near: 5, far: 20000 });
  const r3 = renderer3d({ width: W, height: H });
  const cube = mesh.cube();
  const N = 2000;
  const cubes = instances(N);
  const target = (i) => { const a = i * 2.39996, r = 120 + Math.sqrt(i) * 14; return [Math.cos(a) * r, (i % 40) * 6 - 120, Math.sin(a) * r]; };

  // Particles as light: 50,000 points flowing between two shapes.
  const cloud = pointCloud(50_000, { autoStep: false, shapes: [(i) => [0, 0, 0], (i) => { const y = 1 - (2 * i) / 50_000, r = Math.sqrt(1 - y * y), t = i * 2.4; return [Math.cos(t) * r * 520, y * 520, Math.sin(t) * r * 520]; }] });
  const gt = node.glowTarget(W, H);
  const g = glow(gt, { background: '#000000', budget: 0 });
  g.resize(W, H);

  film.scene('intro', 0, 6, {
    enter() { cloud.morph(1, { duration: 2, ease: 'outExpo' }); },
    step({ dt }) { cloud.step(dt); },
    draw({ ctx, t }) {
      const a = t * 0.6;
      Object.assign(cam.camera, { x: Math.sin(a) * 1400, y: 300, z: Math.cos(a) * 1400 });
      for (let i = 0; i < N; i++) {
        const p = ease('outExpo', Math.min(1, Math.max(0, (t - 0.2 - (i / N) * 0.6) / 0.8)));
        const [x, y, z] = target(i);
        cubes.set(i, { x: x * p, y: y * p - (1 - p) * 900, z: z * p, rx: (1 - p) * 360, ry: i, scale: 18, color: i % 9 ? [0.05, 0.055, 0.07] : [1.6, 0.25, 0.08] });
      }
      const material = { metal: 0.8, specular: 1.3, shininess: 100, emissive: 0.6, lines: 1.4, grid: [1, 1], lineWidth: 0.06, lineColor: [1.5, 0.55, 0.3], pulse: 2.2, pulsePhase: t * 1.5, pulseDir: [0, 0.006, 0] };
      const px = r3.render(cam, [{ mesh: cube, instances: cubes, material }], {
        background: '#07080b', fog: [900, 4000], fogColor: [0.03, 0.03, 0.05],
        sky: [0.02, 0.025, 0.05], horizon: [0.35, 0.18, 0.2], ground: [0.01, 0.01, 0.015], strips: [1.6, 1.6, 1.8],
        key: [0.3, 0.6, -0.7], keyColor: [1, 0.85, 0.75],
      });
      ctx.putImageData(new globalThis.ImageData(px, W, H), 0, 0);
      g.render(cam, cloud, { size: 1.3, intensity: 0.4 });
      ctx.save(); ctx.globalCompositeOperation = 'lighter'; ctx.drawImage(gt.canvas, 0, 0, W, H); ctx.restore();
      // A scrim keeps the type readable over bright cubes (paper on dark).
      const scrim = ctx.createLinearGradient(0, H - 360, 0, H);
      scrim.addColorStop(0, 'rgba(7,8,11,0)');
      scrim.addColorStop(1, 'rgba(7,8,11,0.85)');
      ctx.fillStyle = scrim;
      ctx.fillRect(0, H - 360, W, 360);
      typeFx(ctx, 'Hello, anikit.', W / 2, H - 120, { t, start: 1.0, effect: 'slam', size: 120, color: '#ece6da', align: 'center' });
    },
  });
  return film;
}

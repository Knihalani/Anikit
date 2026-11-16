// hall-controls.js — first-person controls and the minimal HUD for the hall.
// Desktop: WASD/arrows, mouse look (pointer lock on click), Shift to run, E or click to use.
// Phones: left-thumb joystick, drag to look, tap (or the Use button) to use.
// Walking speed is spring-smoothed with anikit; the HUD fades with anikit.

import { spring, animate } from '../../../packages/anikit/dist/anikit.lean.js';

const WALK = 3.0, RUN = 5.6, EYE = 1.7, RAD = 0.35;

export function createControls(canvas, overlayEl, opts) {
  const pos = new Float32Array([opts.spawn[0], opts.spawn[1], opts.spawn[2]]);
  const ctl = {
    pos, yaw: 0, pitch: 0,
    place(eye, yaw, pitch) { /* preview: keep spawn for enter() */ void eye; void yaw; void pitch; },
    enable, disable, update, setStation,
  };
  const keys = { f: 0, b: 0, l: 0, r: 0, run: 0 };
  const mv = { f: 0, s: 0 };           // local-frame velocity, spring-driven
  let tf = 0, ts = 0;                   // current targets
  let active = false, locked = false, dragging = false, lastX = 0, lastY = 0;
  let bob = 0;
  const joy = { id: -1, x0: 0, y0: 0, x: 0, y: 0, lastSend: 0 };
  const look = { id: -1, x: 0, y: 0, moved: 0, t0: 0 };
  const touch = matchMedia('(pointer: coarse)').matches;

  // ---- HUD ----
  const hud = document.createElement('div');
  hud.style.cssText = 'position:absolute;inset:0;pointer-events:none;font:500 15px/1.3 system-ui,-apple-system,Segoe UI,sans-serif;color:#2a2018;opacity:0';
  const cross = document.createElement('div');
  cross.style.cssText = 'position:absolute;left:50%;top:50%;width:6px;height:6px;margin:-3px 0 0 -3px;border-radius:50%;background:#fff;box-shadow:0 0 0 1.5px rgba(40,30,20,.55);transition:transform .25s';
  const card = document.createElement('div');
  card.style.cssText = 'position:absolute;left:50%;bottom:12%;transform:translateX(-50%);text-align:center;opacity:0;white-space:nowrap;text-shadow:0 1px 8px rgba(255,248,236,.9)';
  const name = document.createElement('div');
  name.style.cssText = 'font-weight:700;font-size:22px;letter-spacing:.01em';
  const hint = document.createElement('div');
  hint.style.cssText = 'opacity:.75;margin-top:4px;font-size:14px';
  card.append(name, hint);
  const tip = document.createElement('div');
  tip.style.cssText = 'position:absolute;left:50%;top:58%;transform:translateX(-50%);font-size:14px;opacity:.8;white-space:nowrap;text-shadow:0 1px 8px rgba(255,248,236,.9)';
  tip.textContent = touch ? 'Left thumb to walk · drag to look' : 'Click to look around · WASD to walk · Shift to run';
  const jBase = document.createElement('div');
  jBase.style.cssText = 'position:absolute;width:110px;height:110px;margin:-55px 0 0 -55px;border-radius:50%;border:2px solid rgba(255,255,255,.75);background:rgba(255,250,240,.18);opacity:0;left:0;top:0';
  const jKnob = document.createElement('div');
  jKnob.style.cssText = 'position:absolute;width:46px;height:46px;margin:-23px 0 0 -23px;border-radius:50%;background:rgba(255,255,255,.85);box-shadow:0 2px 10px rgba(0,0,0,.15);opacity:0;left:0;top:0';
  const useBtn = document.createElement('button');
  useBtn.textContent = 'Use';
  useBtn.style.cssText = 'position:absolute;right:22px;bottom:22px;width:76px;height:76px;border-radius:50%;border:0;background:rgba(255,252,246,.9);color:#2a2018;font:700 16px system-ui,sans-serif;box-shadow:0 4px 18px rgba(80,50,20,.18);pointer-events:auto;opacity:0;display:' + (touch ? 'block' : 'none');
  hud.append(cross, card, tip, jBase, jKnob, useBtn);
  const cardS = { o: 0 }, hudS = { o: 0 }, tipS = { o: 1 };

  function setStation(n, h) {
    if (n) { name.textContent = n; hint.textContent = touch ? h.replace(/Click or E|E to/g, (m) => (m === 'E to' ? 'Use to' : 'Tap')) : h; }
    animate(card, { opacity: n ? 1 : 0, y: n ? [8, 0] : 0 }, { duration: n ? 0.45 : 0.3, ease: 'outCubic' });
    animate(useBtn, { opacity: n ? 1 : 0.35, scale: n ? 1 : 0.9 }, { duration: 0.35, ease: 'outBack' });
    animate(cross, { scale: n ? 1.8 : 1 }, { duration: 0.35, ease: 'outBack' });
    void cardS;
  }

  function retarget() {
    const sp = keys.run ? RUN : WALK;
    let f = keys.f - keys.b, s = keys.r - keys.l;
    if (joy.id >= 0) {
      const dx = (joy.x - joy.x0) / 50, dy = (joy.y - joy.y0) / 50;
      const m = Math.min(1, Math.hypot(dx, dy));
      const a = Math.atan2(dx, -dy);
      f = Math.cos(a) * m; s = Math.sin(a) * m;
      const nf = f * (m > 0.92 ? RUN : WALK), ns = s * (m > 0.92 ? RUN : WALK);
      if (Math.abs(nf - tf) + Math.abs(ns - ts) < 0.15) return;
      tf = nf; ts = ns;
    } else {
      const l = Math.hypot(f, s) || 1;
      const nf = (f / l) * sp, ns = (s / l) * sp;
      if (nf === tf && ns === ts) return;
      tf = nf; ts = ns;
    }
    spring(mv, { f: tf, s: ts }, { stiffness: 70, damping: 15 });
  }

  const KEYMAP = { KeyW: 'f', ArrowUp: 'f', KeyS: 'b', ArrowDown: 'b', KeyA: 'l', ArrowLeft: 'l', KeyD: 'r', ArrowRight: 'r', ShiftLeft: 'run', ShiftRight: 'run' };
  function onKey(e) {
    if (!active) return;
    const down = e.type === 'keydown';
    const k = KEYMAP[e.code];
    if (k) {
      keys[k] = down ? 1 : 0;
      if (e.code.startsWith('Arrow')) e.preventDefault();
      retarget();
      if (down) hideTip();
    } else if (down && (e.code === 'KeyE' || e.code === 'Enter' || e.code === 'Space')) {
      e.preventDefault();
      opts.onUse();
    }
  }
  function onBlur() { keys.f = keys.b = keys.l = keys.r = keys.run = 0; retarget(); }

  function isUI(t) { return t && t.closest && t.closest('button,a,input,select,textarea,[data-ui]'); }
  function onDown(e) {
    if (!active || e.pointerType === 'touch' || isUI(e.target)) return;
    if (locked) { opts.onUse(); return; }
    dragging = true; lastX = e.clientX; lastY = e.clientY;
    look.moved = 0;
    if (canvas.requestPointerLock) {
      try {
        const p = canvas.requestPointerLock();
        if (p && p.catch) p.catch(() => {});
      } catch { /* no pointer lock: drag to look */ }
    }
  }
  function onUp(e) {
    if (e.pointerType === 'touch') return;
    if (dragging && !locked && look.moved < 4 && !isUI(e.target)) opts.onUse();
    dragging = false;
  }
  function onMove(e) {
    if (!active || e.pointerType === 'touch') return;
    if (locked) turn(e.movementX, e.movementY, 0.0022);
    else if (dragging) {
      const dx = e.clientX - lastX, dy = e.clientY - lastY;
      look.moved += Math.abs(dx) + Math.abs(dy);
      lastX = e.clientX; lastY = e.clientY;
      turn(dx, dy, 0.005);
    }
  }
  function turn(dx, dy, k) {
    ctl.yaw -= dx * k;
    ctl.pitch = Math.max(-1.2, Math.min(1.2, ctl.pitch - dy * k));
  }
  function onLock() {
    locked = document.pointerLockElement === canvas;
    if (locked) hideTip();
  }

  // touch
  function onTouchStart(e) {
    if (!active) return;
    for (const t of e.changedTouches) {
      if (isUI(t.target)) continue;
      if (t.clientX < innerWidth * 0.45 && joy.id < 0) {
        joy.id = t.identifier; joy.x0 = joy.x = t.clientX; joy.y0 = joy.y = t.clientY;
        jBase.style.left = jKnob.style.left = t.clientX + 'px';
        jBase.style.top = jKnob.style.top = t.clientY + 'px';
        jBase.style.opacity = '1'; jKnob.style.opacity = '1';
        hideTip();
      } else if (look.id < 0) {
        look.id = t.identifier; look.x = t.clientX; look.y = t.clientY; look.moved = 0; look.t0 = performance.now();
      }
    }
    if (e.cancelable) e.preventDefault();
  }
  function onTouchMove(e) {
    if (!active) return;
    for (const t of e.changedTouches) {
      if (t.identifier === joy.id) {
        let dx = t.clientX - joy.x0, dy = t.clientY - joy.y0;
        const m = Math.hypot(dx, dy);
        if (m > 50) { dx *= 50 / m; dy *= 50 / m; }
        joy.x = joy.x0 + dx; joy.y = joy.y0 + dy;
        jKnob.style.transform = `translate(${dx}px,${dy}px)`;
        retarget();
      } else if (t.identifier === look.id) {
        const dx = t.clientX - look.x, dy = t.clientY - look.y;
        look.moved += Math.abs(dx) + Math.abs(dy);
        look.x = t.clientX; look.y = t.clientY;
        turn(dx, dy, 0.006);
        hideTip();
      }
    }
    if (e.cancelable) e.preventDefault();
  }
  function onTouchEnd(e) {
    for (const t of e.changedTouches) {
      if (t.identifier === joy.id) {
        joy.id = -1;
        jKnob.style.transform = '';
        jBase.style.opacity = '0'; jKnob.style.opacity = '0';
        tf = ts = 0;
        spring(mv, { f: 0, s: 0 }, { stiffness: 70, damping: 15 });
      } else if (t.identifier === look.id) {
        if (look.moved < 10 && performance.now() - look.t0 < 300) opts.onUse();
        look.id = -1;
      }
    }
  }
  function onUseBtn(e) { e.preventDefault(); e.stopPropagation(); opts.onUse(); }

  let tipShown = true;
  function hideTip() {
    if (!tipShown) return;
    tipShown = false;
    animate(tip, { opacity: 0 }, { duration: 0.6, ease: 'outCubic' });
    void tipS;
  }

  function enable() {
    if (active) return;
    active = true;
    // start at the spawn point, looking down the hall
    pos[0] = opts.spawn[0]; pos[1] = opts.spawn[1]; pos[2] = opts.spawn[2];
    ctl.yaw = 0; ctl.pitch = 0; mv.f = mv.s = 0; tf = ts = 0;
    if (!hud.parentNode && overlayEl) overlayEl.appendChild(hud);
    animate(hud, { opacity: 1 }, { duration: 0.8, ease: 'outCubic' });
    tipShown = true; tip.style.opacity = '0.8';
    addEventListener('keydown', onKey);
    addEventListener('keyup', onKey);
    addEventListener('blur', onBlur);
    addEventListener('pointerdown', onDown);
    addEventListener('pointerup', onUp);
    addEventListener('pointermove', onMove);
    document.addEventListener('pointerlockchange', onLock);
    addEventListener('touchstart', onTouchStart, { passive: false });
    addEventListener('touchmove', onTouchMove, { passive: false });
    addEventListener('touchend', onTouchEnd);
    addEventListener('touchcancel', onTouchEnd);
    useBtn.addEventListener('pointerdown', onUseBtn);
    void hudS;
  }
  function disable() {
    if (!active) return;
    active = false;
    removeEventListener('keydown', onKey);
    removeEventListener('keyup', onKey);
    removeEventListener('blur', onBlur);
    removeEventListener('pointerdown', onDown);
    removeEventListener('pointerup', onUp);
    removeEventListener('pointermove', onMove);
    document.removeEventListener('pointerlockchange', onLock);
    removeEventListener('touchstart', onTouchStart);
    removeEventListener('touchmove', onTouchMove);
    removeEventListener('touchend', onTouchEnd);
    removeEventListener('touchcancel', onTouchEnd);
    useBtn.removeEventListener('pointerdown', onUseBtn);
    if (document.pointerLockElement === canvas) document.exitPointerLock();
    onBlur();
    joy.id = look.id = -1;
    animate(hud, { opacity: 0 }, { duration: 0.4, ease: 'outCubic', onComplete: () => { if (!active && hud.parentNode) hud.remove(); } });
  }

  const C = opts.colliders, B = opts.bounds;
  function update(t, dt) {
    const sy = Math.sin(ctl.yaw), cy = Math.cos(ctl.yaw);
    // forward = (-sin, -cos), right = (cos, -sin)
    let x = pos[0] + (-sy * mv.f + cy * mv.s) * dt;
    let z = pos[2] + (-cy * mv.f - sy * mv.s) * dt;
    for (let i = 0; i < C.length; i += 4) {
      const cx = x < C[i] ? C[i] : x > C[i + 2] ? C[i + 2] : x;
      const cz = z < C[i + 1] ? C[i + 1] : z > C[i + 3] ? C[i + 3] : z;
      const dx = x - cx, dz = z - cz, d2 = dx * dx + dz * dz;
      if (d2 < RAD * RAD) {
        if (d2 > 1e-8) { const d = Math.sqrt(d2), k = (RAD - d) / d; x += dx * k; z += dz * k; }
        else { // inside: push out along the shortest axis
          const l = x - C[i], r = C[i + 2] - x, f = z - C[i + 1], b = C[i + 3] - z, m = Math.min(l, r, f, b);
          if (m === l) x = C[i] - RAD; else if (m === r) x = C[i + 2] + RAD; else if (m === f) z = C[i + 1] - RAD; else z = C[i + 3] + RAD;
        }
      }
    }
    pos[0] = x < B[0] ? B[0] : x > B[2] ? B[2] : x;
    pos[2] = z < B[1] ? B[1] : z > B[3] ? B[3] : z;
    const speed = Math.hypot(mv.f, mv.s);
    bob += speed * dt * 2.2;
    const amp = Math.min(1, speed / WALK);
    pos[1] = EYE + Math.sin(bob * 2) * 0.035 * amp;
    void t;
  }

  return ctl;
}

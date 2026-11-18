// Generates index.html: the tile grid is written out as static markup so every tile has a stable id.
import { writeFileSync } from "node:fs";

const COLS = 19, ROWS = 11, CELL = 120, X0 = -180, Y0 = -120, CI = 9, CJ = 5;
function mulberry32(a) {
  return () => {
    a |= 0; a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const rnd = mulberry32(1405);

const tiles = [];
for (let j = 0; j < ROWS; j++)
  for (let i = 0; i < COLS; i++) {
    let rot = Math.floor(rnd() * 4) * 90;
    if (i === CI && j === CJ) rot = 0; // the hero tile lands here unrotated
    const cls = ["t", j % 2 === 0 ? "re" : "ro", i % 2 === 1 ? "co" : "ce"].join(" ");
    tiles.push(
      `<div class="${cls}" id="t-${i}-${j}" style="left:${X0 + i * CELL}px;top:${Y0 + j * CELL}px" data-i="${i}" data-j="${j}" data-r="${rot}"><div class="ti"></div></div>`,
    );
  }

// a strip of small tiles for the end card
const BAND = 36, BN = 40;
const band = [];
const rb = mulberry32(27);
for (let k = 0; k < BN; k++) {
  const rot = Math.floor(rb() * 4) * 90;
  band.push(`<div class="b" id="b-${k}" data-r="${rot}"><div class="bi"></div></div>`);
}

const html = /* html */ `<!doctype html>
<html lang="en" data-composition-variables='[
  {"id":"name","type":"string","label":"Conference name","default":"offset"},
  {"id":"what","type":"string","label":"What","default":"Design Systems Conference"},
  {"id":"where","type":"string","label":"Where","default":"Lisbon, Portugal"},
  {"id":"when","type":"string","label":"When","default":"14–15 May 2027"},
  {"id":"cobalt","type":"color","label":"Cobalt","default":"#1C3FAA"},
  {"id":"vermilion","type":"color","label":"Vermilion","default":"#E8492E"},
  {"id":"cream","type":"color","label":"Cream","default":"#F3ECDF"}
]'>
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=1920, height=1080" />
    <script src="https://cdn.jsdelivr.net/npm/gsap@3.15.0/dist/gsap.min.js"></script>
    <style>
      :root {
        --cobalt: #1c3faa;
        --vermilion: #e8492e;
        --cream: #f3ecdf;
        --grout: #e3d8c4;
        --ink: #161a24;
        --mute: #7d7668;
      }
      * { margin: 0; padding: 0; box-sizing: border-box; }
      html, body { width: 1920px; height: 1080px; overflow: hidden; background: var(--cream); }
      #root { position: relative; width: 1920px; height: 1080px; overflow: hidden; background: var(--cream);
        font-family: "Inter", sans-serif; color: var(--ink); }
      .layer { position: absolute; inset: 0; }
      .mono { font-family: "IBM Plex Mono", monospace; font-weight: 400; font-size: 28px; letter-spacing: 0.02em; }

      /* faint construction grid */
      #grid line { stroke: var(--cobalt); stroke-width: 1; opacity: 0.14; }

      /* tiles */
      .t { position: absolute; width: ${CELL}px; height: ${CELL}px; }
      .ti { position: absolute; inset: 0; background-image: url("assets/tile.svg"); background-size: 100% 100%; }
      #tiles { background: transparent; }

      /* hero tile */
      #hero { position: absolute; left: ${960 - 60}px; top: ${540 - 60}px; width: 120px; height: 120px; overflow: visible; }
      #hero-ann { position: absolute; inset: 0; }
      .dim { position: absolute; color: var(--cobalt); white-space: nowrap; }
      .reg { position: absolute; width: 56px; height: 56px; }

      /* wordmark */
      #mark { position: absolute; left: 0; right: 0; top: 168px; height: 440px; }
      .plate { position: absolute; left: 0; right: 0; top: 0; text-align: center; font-family: "Inter", sans-serif;
        font-weight: 900; font-size: 420px; line-height: 440px; letter-spacing: -0.058em; padding-right: 0.058em; }
      #plate-v { color: var(--vermilion); }
      #plate-c { color: var(--cobalt); mix-blend-mode: multiply; }
      #plate-k { color: var(--cobalt); }
      #token { position: absolute; left: 1338px; top: 594px; color: var(--vermilion); white-space: nowrap; }

      /* spec row */
      #rule { position: absolute; left: 240px; top: 690px; width: 1440px; height: 2px; background: var(--ink); transform-origin: 0 50%; }
      .col { position: absolute; top: 716px; }
      .col .k { color: var(--mute); display: block; margin-bottom: 10px; }
      .col .v { font-weight: 700; font-size: 42px; letter-spacing: -0.02em; display: block; white-space: nowrap; }
      .col .v span { display: inline-block; }
      #c1 { left: 240px; }
      #c2 { left: 900px; }
      #c3 { right: 240px; text-align: right; }

      /* chrome */
      #chrome .mono { position: absolute; color: var(--ink); opacity: 0.72; }
      #band { position: absolute; left: 240px; top: 892px; width: 1440px; height: ${BAND}px; display: flex; }
      .b { width: ${BAND}px; height: ${BAND}px; position: relative; flex: none; }
      .bi { position: absolute; inset: 0; background-image: url("assets/tile.svg"); background-size: 100% 100%; }

      #black { position: absolute; inset: 0; background: #07080b; opacity: 0; }
    </style>
  </head>
  <body>
    <div id="root" data-composition-id="main" data-start="0" data-duration="15" data-width="1920" data-height="1080" data-motion-blur="on">

      <!-- construction grid -->
      <svg id="grid" class="clip layer" data-start="0" data-duration="15" data-track-index="0" data-layout-ignore viewBox="0 0 1920 1080" width="1920" height="1080">
        ${Array.from({ length: 17 }, (_, k) => `<line class="gv" x1="${60 + k * 120}" y1="0" x2="${60 + k * 120}" y2="1080"/>`).join("")}
        ${Array.from({ length: 9 }, (_, k) => `<line class="gh" x1="0" y1="${60 + k * 120}" x2="1920" y2="${60 + k * 120}"/>`).join("")}
      </svg>

      <!-- the pattern -->
      <div id="tiles" class="clip layer" data-start="0" data-duration="8" data-track-index="1" data-layout-allow-overflow data-layout-ignore>
        ${tiles.join("\n        ")}
      </div>

      <!-- one component -->
      <div id="hero-wrap" class="clip layer" data-start="0" data-duration="1.9" data-track-index="2" data-layout-allow-overflow data-layout-allow-overlap>
        <svg id="hero" viewBox="0 0 120 120" width="120" height="120">
          <rect id="h-fill" x="2" y="2" width="116" height="116" rx="3" fill="#F4EEE2" />
          <g clip-path="url(#hclip)">
            <path id="h-disc" d="M0 0 H60 A60 60 0 0 1 0 60 Z" fill="#1C3FAA" />
            <path id="h-ring" d="M120 60 A60 60 0 0 0 60 120 H76 A44 44 0 0 1 120 76 Z" fill="#1C3FAA" />
            <path id="h-dot" d="M120 98 A22 22 0 0 0 98 120 H120 Z" fill="#1C3FAA" />
          </g>
          <defs><clipPath id="hclip"><rect x="2" y="2" width="116" height="116" rx="3" /></clipPath></defs>
          <rect id="h-line" x="2" y="2" width="116" height="116" rx="3" fill="none" stroke="#1C3FAA" stroke-width="0.6" pathLength="100" stroke-dasharray="100 120" stroke-dashoffset="100" />
        </svg>
        <div id="hero-ann">
          <svg class="layer" viewBox="0 0 1920 1080" width="1920" height="1080">
            <g id="dim-w" stroke="#1C3FAA" stroke-width="1.5">
              <line x1="780" y1="330" x2="1140" y2="330" /><line x1="780" y1="318" x2="780" y2="342" /><line x1="1140" y1="318" x2="1140" y2="342" />
            </g>
            <g id="dim-h" stroke="#1C3FAA" stroke-width="1.5">
              <line x1="1190" y1="360" x2="1190" y2="720" /><line x1="1178" y1="360" x2="1202" y2="360" /><line x1="1178" y1="720" x2="1202" y2="720" />
            </g>
          </svg>
          <div class="dim mono" id="lab-w" style="left:960px;top:278px;transform:translateX(-50%)">120</div>
          <div class="dim mono" id="lab-h" style="left:1214px;top:522px">120</div>
          <div class="dim mono" id="lab-n" style="left:780px;top:742px">tile / 01</div>
        </div>
      </div>

      <!-- the mark -->
      <div id="mark" class="clip" data-start="6.8" data-duration="8.2" data-track-index="3" data-layout-allow-overlap data-layout-allow-overflow>
        <div class="plate" id="plate-v" data-var-text="name">offset</div>
        <div class="plate" id="plate-c" data-var-text="name">offset</div>
        <div class="plate" id="plate-k" data-var-text="name">offset</div>
      </div>
      <div id="token-wrap" class="clip layer" data-start="8" data-duration="7" data-track-index="4">
        <div id="token" class="mono">↘ offset: 14px</div>
      </div>

      <div id="spec" class="clip layer" data-start="8.4" data-duration="6.6" data-track-index="5">
        <div id="rule"></div>
        <div class="col" id="c1"><span class="k mono">01 / what</span><span class="v"><span>Design</span> <span>Systems</span> <span>Conference</span></span></div>
        <div class="col" id="c2"><span class="k mono">02 / where</span><span class="v"><span>Lisbon,</span> <span>Portugal</span></span></div>
        <div class="col" id="c3"><span class="k mono">03 / when</span><span class="v"><span>14–15</span> <span>May</span> <span>2027</span></span></div>
      </div>

      <div id="band-wrap" class="clip layer" data-start="9.8" data-duration="5.2" data-track-index="6">
        <div id="band">${band.join("")}</div>
      </div>

      <!-- registration chrome -->
      <div id="chrome" class="clip layer" data-start="0" data-duration="15" data-track-index="7">
        <div class="mono" id="ch-tl" style="left:120px;top:84px">offset / 27</div>
        <div class="mono" id="ch-tr" style="right:120px;top:84px;text-align:right">38.72° N  9.14° W</div>
        <div class="mono" id="ch-bl" style="left:120px;bottom:84px">opening titles</div>
        <div class="mono" id="ch-br" style="right:120px;bottom:84px;text-align:right">keynote 01 follows</div>
      </div>

      <div id="black" class="clip" data-start="13.8" data-duration="1.2" data-track-index="8"></div>

      <!-- sound -->
      <audio id="score" class="clip" src="assets/score.wav" data-start="0" data-duration="15" data-track-index="10" data-volume="0.8"></audio>
      <audio id="sfx-swap" class="clip" src="assets/sfx/click-soft.mp3" data-start="1.69" data-duration="0.36" data-track-index="11" data-volume="0.7"></audio>
      <audio id="sfx-row" class="clip" src="assets/sfx/whoosh-short.mp3" data-start="3.56" data-duration="0.57" data-track-index="11" data-volume="0.55"></audio>
      <audio id="sfx-impact" class="clip" src="assets/sfx/impact-bass-1.mp3" data-start="7.96" data-duration="2.1" data-track-index="11" data-volume="0.8"></audio>
    </div>

    <script>
      const tl = gsap.timeline({ paused: true });
      const CI = ${CI}, CJ = ${CJ};
      const tiles = gsap.utils.toArray(".t");
      const info = tiles.map((el) => {
        const i = +el.dataset.i, j = +el.dataset.j;
        return { el, inner: el.firstElementChild, i, j, r: +el.dataset.r, d: Math.hypot(i - CI, j - CJ) };
      });

      // ── 0 · one component ───────────────────────────────────────────────
      gsap.set("#hero", { scale: 3, transformOrigin: "50% 50%" });
      gsap.set(tiles, { scale: 0, opacity: 0 });
      gsap.set("#t-${CI}-${CJ}", { scale: 1, opacity: 0 });
      tl.fromTo("#h-fill", { opacity: 0 }, { opacity: 1, duration: 0.5, ease: "power2.out" }, 0.45);
      tl.fromTo("#h-line", { strokeDashoffset: 100 }, { strokeDashoffset: 0, duration: 0.6, ease: "power2.inOut" }, 0.15);
      tl.fromTo("#h-disc", { scale: 0, svgOrigin: "0 0" }, { scale: 1, svgOrigin: "0 0", duration: 0.55, ease: "expo.out" }, 0.55);
      tl.fromTo("#h-ring", { scale: 0, svgOrigin: "120 120" }, { scale: 1, svgOrigin: "120 120", duration: 0.5, ease: "expo.out" }, 0.75);
      tl.fromTo("#h-dot", { scale: 0, svgOrigin: "120 120" }, { scale: 1, svgOrigin: "120 120", duration: 0.4, ease: "back.out(1.6)" }, 0.9);
      tl.fromTo(["#dim-w", "#dim-h"], { opacity: 0 }, { opacity: 1, duration: 0.3, stagger: 0.08, ease: "power2.out" }, 0.35);
      tl.fromTo(["#lab-w", "#lab-h", "#lab-n"], { opacity: 0, filter: "blur(6px)" }, { opacity: 1, filter: "blur(0px)", duration: 0.18, stagger: 0.07, ease: "power4.out" }, 0.45);

      // ── 1 · it becomes a system ─────────────────────────────────────────
      tl.to("#hero-ann", { opacity: 0, filter: "blur(6px)", duration: 0.2, ease: "power2.in" }, 1.2);
      tl.to(["#ch-tl", "#ch-tr", "#ch-bl", "#ch-br"], { opacity: 0, duration: 0.2, ease: "power2.in" }, 1.25);
      tl.to("#h-line", { opacity: 0, duration: 0.25 }, 1.3);
      tl.to("#hero", { scale: 1, duration: 0.5, ease: "power3.inOut" }, 1.25);
      tl.fromTo("#grid .gv", { opacity: 0 }, { opacity: 0.14, duration: 0.4, stagger: { each: 0.02, from: "center" }, ease: "power1.out" }, 1.35);
      tl.fromTo("#grid .gh", { opacity: 0 }, { opacity: 0.14, duration: 0.4, stagger: { each: 0.03, from: "center" }, ease: "power1.out" }, 1.4);
      const centre = info.find((t) => t.i === CI && t.j === CJ);
      tl.set(centre.el, { opacity: 1 }, 1.75);
      tl.set("#hero", { opacity: 0 }, 1.76);

      // wave outwards from the centre
      info.forEach((t) => {
        if (t === centre) return;
        const at = 1.72 + t.d * 0.16;
        tl.fromTo(t.el, { scale: 0.2, opacity: 0 }, { scale: 1, opacity: 1, duration: 0.42, ease: "expo.out" }, at);
        tl.fromTo(t.inner, { rotation: t.r - 90 }, { rotation: t.r, duration: 0.5, ease: "back.out(1.2)" }, at);
      });
      tl.set(centre.inner, { rotation: 0 }, 0);

      // ── 2 · offset ──────────────────────────────────────────────────────
      tl.to(".t.re", { x: 60, duration: 0.45, ease: "power3.inOut" }, 3.5);
      tl.to(".t.co", { y: 60, duration: 0.45, ease: "power3.inOut" }, 4.5);
      info.forEach((t) => {
        tl.to(t.inner, { rotation: t.r + 90, duration: 0.42, ease: "power3.inOut" }, 5.5 + (t.i + t.j) * 0.018);
      });
      tl.to(".t.re", { x: 0, duration: 0.4, ease: "power3.inOut" }, 6.25);
      tl.to(".t.co", { y: 0, duration: 0.4, ease: "power3.inOut" }, 6.25);

      // the pattern opens from the centre
      info.forEach((t) => {
        tl.to(t.el, { scale: 0, duration: 0.42, ease: "power3.in" }, 6.85 + t.d * 0.045);
      });
      tl.to("#grid line", { opacity: 0.07, duration: 0.6, ease: "power2.inOut" }, 7.2);

      // ── 3 · the mark comes into register ────────────────────────────────
      tl.fromTo("#plate-v", { x: -230, y: 60, opacity: 0, filter: "blur(14px)" },
        { x: 14, y: 14, opacity: 1, filter: "blur(0px)", duration: 1.1, ease: "expo.out" }, 6.92);
      tl.fromTo("#plate-k", { x: 230, y: -60, filter: "blur(14px)" },
        { x: 0, y: 0, filter: "blur(0px)", duration: 1.1, ease: "expo.out" }, 6.92);
      tl.fromTo("#plate-c", { x: 230, y: -60, opacity: 0, filter: "blur(14px)" },
        { x: 0, y: 0, opacity: 1, filter: "blur(0px)", duration: 1.1, ease: "expo.out" }, 6.92);
      // a second, tighter pull lands exactly on the downbeat
      tl.fromTo("#mark", { scale: 1.035 }, { scale: 1, duration: 0.5, ease: "power3.out" }, 7.98);
      tl.fromTo("#plate-k", { opacity: 0 }, { opacity: 1, duration: 0.55, ease: "power2.inOut" }, 7.42);
      tl.fromTo("#token", { opacity: 0, x: -12, filter: "blur(6px)" }, { opacity: 1, x: 0, filter: "blur(0px)", duration: 0.25, ease: "power4.out" }, 8.25);

      tl.to(["#ch-tl", "#ch-tr", "#ch-bl", "#ch-br"], { opacity: 0.72, duration: 0.4, stagger: 0.06, ease: "power2.out" }, 8.3);

      // ── 4 · the spec ────────────────────────────────────────────────────
      tl.fromTo("#rule", { scaleX: 0 }, { scaleX: 1, duration: 0.7, ease: "power3.inOut" }, 8.4);
      ["#c1", "#c2", "#c3"].forEach((c, k) => {
        const at = 8.7 + k * 0.4;
        tl.fromTo(c + " .k", { opacity: 0 }, { opacity: 1, duration: 0.25, ease: "power2.out" }, at);
        tl.fromTo(c + " .v span", { opacity: 0, filter: "blur(6px)" },
          { opacity: 1, filter: "blur(0px)", duration: 0.14, stagger: 0.08, ease: "power4.out" }, at + 0.06);
      });

      // ── 5 · a strip of the pattern, still working ───────────────────────
      const bands = gsap.utils.toArray(".b");
      bands.forEach((b, k) => {
        const inner = b.firstElementChild, r = +b.dataset.r;
        tl.fromTo(b, { scale: 0 }, { scale: 1, duration: 0.3, ease: "expo.out" }, 9.9 + k * 0.022);
        tl.fromTo(inner, { rotation: r - 90 }, { rotation: r, duration: 0.4, ease: "back.out(1.2)" }, 9.9 + k * 0.022);
        tl.to(inner, { rotation: r + 90, duration: 0.36, ease: "power3.inOut" }, 11.9 + k * 0.02);
        tl.to(inner, { rotation: r + 180, duration: 0.36, ease: "power3.inOut" }, 12.9 + (bands.length - k) * 0.02);
      });

      // ── out ─────────────────────────────────────────────────────────────
      tl.fromTo("#black", { opacity: 0 }, { opacity: 1, duration: 1.0, ease: "power2.in" }, 13.85);

      window.__timelines["main"] = tl;
    </script>
  </body>
</html>
`;
writeFileSync(new URL("./index.html", import.meta.url), html);
console.log("tiles", tiles.length);

// The landing page, built the same way for every library (bench-page.mjs).
export function buildPage() {
  const css = `body{margin:0;font:18px/1.5 system-ui,sans-serif;color:#123}
  .hero{height:90vh;display:flex;align-items:center;justify-content:center;flex-direction:column}
  .hero h1{font-size:56px;margin:0 0 20px;max-width:900px;text-align:center}
  .btn{display:inline-block;padding:14px 28px;margin:0 10px;border-radius:30px;background:#6ea8ff;color:#fff}
  section.s{min-height:80vh;padding:60px 40px}
  section.s h2{font-size:36px}
  .cards{display:grid;grid-template-columns:repeat(4,1fr);gap:16px}
  .card{height:140px;border-radius:12px;background:#e8f0ff;padding:12px}
  .pin{height:100vh;background:#123;color:#fff;display:flex;align-items:center;justify-content:center;font-size:40px}
  .pinwrap{position:relative}
  .marquee{overflow:hidden;white-space:nowrap;padding:30px 0;background:#f4f4f4}
  .track{display:inline-flex;gap:40px;font-size:32px}`;
  const st = document.createElement('style');
  st.textContent = css;
  document.head.appendChild(st);
  const words = 'Build interfaces that move with intent, at sixty frames, on every device'.split(' ');
  let h = `<header class="hero"><h1 class="title">${words.join(' ')}</h1><div><a class="btn">Get started</a><a class="btn">Read the docs</a><a class="btn">Examples</a></div></header>`;
  h += `<div class="marquee"><div class="track">${Array.from({ length: 12 }, (_, i) => `<span>Brand ${i + 1}</span>`).join('')}</div></div>`;
  for (let i = 0; i < 6; i++) {
    h += `<section class="s"><h2>Section ${i + 1}</h2><div class="cards">${Array.from({ length: 8 }, (_, k) => `<div class="card">Card ${k + 1}</div>`).join('')}</div></section>`;
    if (i === 2) h += `<div class="pinwrap"><div class="pin">Pinned story</div></div>`;
  }
  h += `<footer style="height:60vh"></footer>`;
  document.body.innerHTML = h;
}

// Plays an ak264 MP4 in headless Chromium (Playwright) and checks decode:
//   node crates/ak264/scripts/browser-check.js out.mp4
// Prints canPlayType, readyState, currentTime after 1.5 s of playback and the
// mean pixel of the drawn frame; exits non-zero if playback fails.
const { chromium } = require('playwright');
const http = require('http');
const fs = require('fs');
const path = require('path');
const file = path.resolve(process.argv[2] || 'out.mp4');
const server = http.createServer((req, res) => {
  if (req.url === '/') {
    res.end('<video src="/v.mp4" playsinline muted></video>');
  } else {
    res.writeHead(200, { 'Content-Type': 'video/mp4' });
    fs.createReadStream(file).pipe(res);
  }
});
server.listen(0, async () => {
  const b = await chromium.launch();
  const p = await b.newPage();
  await p.goto(`http://127.0.0.1:${server.address().port}/`);
  const r = await p.evaluate(async () => {
    const v = document.querySelector('video');
    const can = v.canPlayType('video/mp4; codecs="avc1.4d4028"');
    const ok = await new Promise((res) => {
      v.oncanplay = () => res('canplay');
      v.onerror = () => res('error ' + (v.error && v.error.code));
      setTimeout(() => res('timeout'), 8000);
      v.load();
    });
    if (ok !== 'canplay') return { can, ok };
    await v.play().catch(() => 0);
    await new Promise((r) => setTimeout(r, 1500));
    const c = document.createElement('canvas');
    c.width = 64;
    c.height = 36;
    const g = c.getContext('2d');
    g.drawImage(v, 0, 0, 64, 36);
    const d = g.getImageData(0, 0, 64, 36).data;
    let s = 0;
    for (let i = 0; i < d.length; i += 4) s += d[i] + d[i + 1] + d[i + 2];
    return { can, ok, readyState: v.readyState, currentTime: v.currentTime, width: v.videoWidth, meanPixel: s / (d.length / 4) / 3 };
  });
  console.log(JSON.stringify(r));
  await b.close();
  server.close();
  process.exit(r.ok === 'canplay' && r.currentTime > 0.5 ? 0 : 1);
});

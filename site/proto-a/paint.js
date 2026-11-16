// A small DOM-to-canvas painter: backgrounds, borders and text runs, placed
// exactly where the browser laid them out. It lets WebGL take over a piece of
// the page with pixels that match the HTML underneath.

const transparent = (c) => !c || c === 'transparent' || /rgba\([^)]*,\s*0\)$/.test(c);

function roundRect(ctx, x, y, w, h, r) {
  r = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

/**
 * Paint `root` into a canvas of its size × scale.
 * `boxes: false` paints the text only.
 */
export function paint(root, scale, { boxes = true, canvas = document.createElement('canvas') } = {}) {
  const R = root.getBoundingClientRect();
  canvas.width = Math.max(1, Math.round(R.width * scale));
  canvas.height = Math.max(1, Math.round(R.height * scale));
  const ctx = canvas.getContext('2d');
  ctx.setTransform(scale, 0, 0, scale, -R.left * scale, -R.top * scale);
  ctx.clearRect(R.left, R.top, R.width, R.height);
  if (boxes) {
    for (const el of [root, ...root.querySelectorAll('*')]) {
      const cs = getComputedStyle(el);
      if (cs.display === 'none') continue;
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) continue;
      const rad = parseFloat(cs.borderTopLeftRadius) || 0;
      if (!transparent(cs.backgroundColor)) {
        ctx.fillStyle = cs.backgroundColor;
        roundRect(ctx, r.left, r.top, r.width, r.height, rad);
        ctx.fill();
      }
      const bw = parseFloat(cs.borderTopWidth);
      if (bw > 0 && cs.borderTopStyle !== 'none' && !transparent(cs.borderTopColor)) {
        ctx.strokeStyle = cs.borderTopColor; ctx.lineWidth = bw;
        roundRect(ctx, r.left + bw / 2, r.top + bw / 2, r.width - bw, r.height - bw, Math.max(0, rad - bw / 2));
        ctx.stroke();
      }
    }
  }
  const range = document.createRange();
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let node;
  while ((node = walker.nextNode())) {
    const text = node.textContent;
    if (!text.trim()) continue;
    const cs = getComputedStyle(node.parentElement);
    ctx.font = cs.font || `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
    ctx.fillStyle = node.parentElement.dataset.paintColor || cs.color;
    if ('letterSpacing' in ctx) ctx.letterSpacing = cs.letterSpacing === 'normal' ? '0px' : cs.letterSpacing;
    ctx.textBaseline = 'alphabetic';
    const underline = cs.textDecorationLine.includes('underline');
    const re = /\S+/g;
    let m;
    while ((m = re.exec(text))) {
      range.setStart(node, m.index);
      range.setEnd(node, m.index + m[0].length);
      const rects = range.getClientRects();
      if (!rects.length) continue;
      const r = rects[0];
      const fm = ctx.measureText(m[0]);
      const base = r.top + (r.height - (fm.fontBoundingBoxAscent + fm.fontBoundingBoxDescent)) / 2 + fm.fontBoundingBoxAscent;
      ctx.fillText(m[0], r.left, base);
      if (underline) ctx.fillRect(r.left, base + 2, r.width, Math.max(1, parseFloat(cs.fontSize) / 16));
    }
  }
  return canvas;
}

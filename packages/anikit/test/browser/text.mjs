import { ok, near } from './assert.mjs';

// Visual left-to-right order of the words of `sel` (by their rendered x),
// measured through Range rects on the text nodes (works before and after a split).
const wordOrder = (page, sel) => page.$eval(sel, (el) => {
  const out = [];
  const walk = (n) => {
    for (const c of n.childNodes) {
      if (c.nodeType === 3) {
        const re = /\S+/g;
        let m;
        while ((m = re.exec(c.textContent))) {
          const r = document.createRange();
          r.setStart(c, m.index);
          r.setEnd(c, m.index + m[0].length);
          const b = r.getBoundingClientRect();
          out.push({ w: m[0], x: b.left, y: Math.round(b.top) });
        }
      } else walk(c);
    }
  };
  walk(el);
  // Words split into chars come back as single chars: join adjacent pieces of one word span.
  return out;
});

export const tests = [
  {
    name: 'emoji, flags, skin tones and combining marks stay one char each',
    html: '<h1 id="t">Hi 👩‍👩‍👧‍👦 🇯🇵🇫🇷 👍🏽 été नमस्ते</h1>',
    async run(page) {
      const r = await page.evaluate(() => {
        const s = window.ak.splitText('#t', { type: 'words,chars' });
        return { chars: s.chars.map((c) => c.textContent), words: s.words.map((w) => w.textContent) };
      });
      ok(r.words.length === 6, `words ${JSON.stringify(r.words)}`);
      for (const g of ['👩‍👩‍👧‍👦', '🇯🇵', '🇫🇷', '👍🏽', 'é']) ok(r.chars.includes(g), `missing grapheme ${g} in ${JSON.stringify(r.chars)}`);
      ok(!r.chars.some((c) => c === '́' || c === '‍'), 'a lone combining mark or ZWJ');
    },
  },
  {
    name: 'revert() restores the exact markup and aria-label',
    html: '<p id="t" class="x">Hello <b>bold <i>it</i></b> world<br>next 🇯🇵 line &amp; more</p>',
    async run(page) {
      const r = await page.evaluate(() => {
        const el = document.getElementById('t');
        const before = el.outerHTML;
        const s = window.ak.splitText(el, { type: 'lines,words,chars', mask: 'lines' });
        const mid = el.outerHTML;
        s.revert();
        return { before, mid, after: el.outerHTML, label: el.getAttribute('aria-label') };
      });
      ok(r.mid !== r.before, 'nothing was split');
      ok(r.after === r.before, `markup differs:\n${r.before}\n${r.after}`);
      ok(r.label === null, 'aria-label left behind');
    },
  },
  {
    name: 'context().revert() undoes a split and its animation',
    html: '<p id="t" style="width:200px">The quick brown fox jumps over the lazy dog</p>',
    async run(page) {
      const r = await page.evaluate(() => {
        const el = document.getElementById('t');
        const before = el.outerHTML;
        const ctx = window.ak.context(() => {
          window.ak.splitText(el, { type: 'lines', autoSplit: true, onSplit: ({ lines }) => window.ak.animate(lines, { y: [20, 0] }) });
        });
        ctx.revert();
        return { before, after: el.outerHTML };
      });
      ok(r.after === r.before, `markup differs:\n${r.before}\n${r.after}`);
    },
  },
  {
    name: 'lines: words grouped by rendered line, mask wraps each line',
    html: '<p id="t" style="width:220px">The quick brown fox jumps over the lazy dog and keeps running far away</p>',
    async run(page) {
      const r = await page.evaluate(() => {
        const s = window.ak.splitText('#t', { type: 'lines,words', mask: 'lines' });
        return {
          lines: s.lines.map((l) => l.textContent.trim()),
          tops: s.lines.map((l) => l.getBoundingClientRect().top),
          masks: s.masks.length,
          maskedOk: s.masks.every((m, i) => m.firstChild === s.lines[i] && getComputedStyle(m).overflow.includes('clip')),
          words: s.words.length,
        };
      });
      ok(r.lines.length >= 3, `lines ${JSON.stringify(r.lines)}`);
      ok(r.masks === r.lines.length && r.maskedOk, 'one clipping mask per line');
      for (let i = 1; i < r.tops.length; i++) ok(r.tops[i] > r.tops[i - 1], 'lines go down');
      ok(r.lines.join(' ').split(/\s+/).length === r.words, 'every word in a line');
    },
  },
  {
    name: 'RTL paragraph keeps its visual word order',
    html: '<p id="t" dir="rtl" style="width:600px">שלום עולם יפה מאוד</p><p id="a" dir="rtl" style="width:600px">مرحبا بالعالم الجميل</p>',
    async run(page) {
      for (const sel of ['#t', '#a']) {
        const before = (await wordOrder(page, sel)).sort((a, b) => a.x - b.x).map((w) => w.w);
        await page.evaluate((sel) => window.ak.splitText(sel, { type: 'words' }), sel);
        const after = (await wordOrder(page, sel)).sort((a, b) => a.x - b.x).map((w) => w.w);
        ok(JSON.stringify(before) === JSON.stringify(after), `${sel} order ${before} -> ${after}`);
      }
    },
  },
  {
    name: 'mixed bidi: a Hebrew phrase inside an LTR line keeps its visual order',
    html: '<p id="t" style="width:700px">Say שלום עולם יפה to everyone</p>',
    async run(page) {
      const before = (await wordOrder(page, '#t')).sort((a, b) => a.x - b.x).map((w) => w.w);
      await page.evaluate(() => window.ak.splitText('#t', { type: 'words' }));
      const after = (await wordOrder(page, '#t')).sort((a, b) => a.x - b.x).map((w) => w.w);
      ok(JSON.stringify(before) === JSON.stringify(after), `order ${before} -> ${after}`);
    },
  },
  {
    name: 'Arabic split into chars keeps the joined letter shapes (same width)',
    html: '<p id="t" style="font-size:32px"><span id="w">مرحبا</span></p>',
    async run(page) {
      const w0 = await page.$eval('#w', (el) => el.getBoundingClientRect().width);
      await page.evaluate(() => window.ak.splitText('#w', { type: 'chars' }));
      const w1 = await page.$eval('#w', (el) => el.getBoundingClientRect().width);
      near(w1, w0, Math.max(2, w0 * 0.06), 'width after a char split (isolated forms are wider)');
    },
  },
  {
    name: 'autoSplit re-splits lines on a width change and hands over the animation',
    html: '<p id="t" style="width:600px">The quick brown fox jumps over the lazy dog and keeps running far away</p>',
    async run(page, h) {
      await page.evaluate(() => {
        window.calls = 0;
        window.s = window.ak.splitText('#t', {
          type: 'lines', autoSplit: true,
          onSplit: ({ lines }) => { window.calls++; return window.ak.animate(lines, { opacity: [0, 1] }, { duration: 5 }); },
        });
        window.n0 = window.s.lines.length;
      });
      await h.frames(3);
      await page.$eval('#t', (el) => (el.style.width = '200px'));
      await h.frames(4);
      const r = await page.evaluate(() => ({ n0: window.n0, n1: window.s.lines.length, calls: window.calls, text: document.getElementById('t').textContent }));
      ok(r.calls === 2, `onSplit calls ${r.calls}`);
      ok(r.n1 > r.n0, `lines ${r.n0} -> ${r.n1}`);
      ok(r.text.replace(/\s+/g, ' ').trim() === 'The quick brown fox jumps over the lazy dog and keeps running far away', 'text intact');
      // Old line wrappers gone: no nested line spans.
      const nested = await page.$eval('#t', (el) => [...el.querySelectorAll('span')].filter((s) => s.style.display === 'block' && s.parentElement.closest('span[style]') && s.parentElement.closest('span[style]').style.display === 'block').length);
      ok(nested === 0, 'nested line wrappers after re-split');
      await page.evaluate(() => window.s.revert());
    },
  },
  {
    name: 'splitting an already split element and reverting both restores it',
    html: '<p id="t">Hello big world</p>',
    async run(page) {
      const r = await page.evaluate(() => {
        const el = document.getElementById('t');
        const before = el.outerHTML;
        const a = window.ak.splitText(el, { type: 'words' });
        const b = window.ak.splitText(el, { type: 'chars' });
        b.revert();
        a.revert();
        return { before, after: el.outerHTML };
      });
      ok(r.after === r.before, `markup differs:\n${r.before}\n${r.after}`);
    },
  },
  {
    name: 'lines keep nested markup: each word keeps its own formatting and link',
    html: '<p id="t" style="width:180px">Plain words then <b>bold words that wrap across <a href="#x">a link here</a> lines</b> and plain again</p>',
    async run(page) {
      const style = () => page.$eval('#t', (el) => {
        const out = [];
        const walk = (n) => {
          for (const c of n.childNodes) {
            if (c.nodeType === 3) {
              for (const w of c.textContent.split(/\s+/).filter(Boolean)) {
                const p = c.parentElement;
                out.push(`${w}:${getComputedStyle(p).fontWeight}:${p.closest('a') ? 'a' : '-'}`);
              }
            } else walk(c);
          }
        };
        walk(el);
        return out.join(' ');
      });
      const before = await style();
      const r = await page.evaluate(() => {
        const s = window.ak.splitText('#t', { type: 'lines,words' });
        return { n: s.lines.length, parents: s.lines.every((l) => l.parentElement.id === 't') };
      });
      ok(r.n >= 3, `lines ${r.n}`);
      ok(r.parents, 'line wrappers are direct children of the element');
      const after = await style();
      ok(after === before, `formatting changed:\n${before}\n${after}`);
    },
  },
  {
    name: 'mixed bidi with chars: every letter keeps its place (LTR line with Hebrew, RTL line with English)',
    html: '<p id="a" style="width:700px">Say שלום עולם to all</p><p id="b" dir="rtl" style="width:700px">אמר hello world לכולם</p>',
    async run(page) {
      const xs = (sel) => page.$eval(sel, (el) => {
        const out = [];
        const walk = (n) => {
          for (const c of n.childNodes) {
            if (c.nodeType === 3) {
              const t = c.textContent;
              for (let i = 0; i < t.length; i++) {
                if (/[\s\u200d]/.test(t[i])) continue;
                const r = document.createRange();
                r.setStart(c, i);
                r.setEnd(c, i + 1);
                out.push([t[i], Math.round(r.getBoundingClientRect().left)]);
              }
            } else walk(c);
          }
        };
        walk(el);
        return out.sort((a, b) => a[1] - b[1]).map((p) => p[0]).join('');
      });
      for (const sel of ['#a', '#b']) {
        const before = await xs(sel);
        await page.evaluate((sel) => window.ak.splitText(sel, { type: 'words,chars' }), sel);
        const after = await xs(sel);
        ok(before === after, `${sel} visual order ${before} -> ${after}`);
      }
      for (const sel of ['#a', '#b']) {
        const before = await page.$eval(sel, (el) => el.innerHTML);
        await page.evaluate((sel) => window.ak.splitText(sel, { type: 'chars' }).revert(), sel);
        ok(before === (await page.$eval(sel, (el) => el.innerHTML)), 'revert of a nested split');
      }
    },
  },
  {
    name: 'chars-only split of an LTR line with a Hebrew phrase keeps visual order',
    html: '<p id="a" style="width:700px">Say שלום עולם to all</p>',
    async run(page) {
      const order = () => page.$eval('#a', (el) => {
        const out = [];
        const walk = (n) => {
          for (const c of n.childNodes) {
            if (c.nodeType === 3) {
              const t = c.textContent;
              for (let i = 0; i < t.length; i++) {
                if (/[\s]/.test(t[i])) continue;
                const r = document.createRange();
                r.setStart(c, i);
                r.setEnd(c, i + 1);
                out.push([t[i], Math.round(r.getBoundingClientRect().left)]);
              }
            } else walk(c);
          }
        };
        walk(el);
        return out.sort((a, b) => a[1] - b[1]).map((p) => p[0]).join('');
      });
      const before = await order();
      await page.evaluate(() => window.ak.splitText('#a', { type: 'chars' }));
      const after = await order();
      ok(before === after, `visual order ${before} -> ${after}`);
    },
  },
];

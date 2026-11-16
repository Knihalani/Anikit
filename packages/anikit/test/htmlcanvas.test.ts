import { describe, it, expect, beforeAll } from 'vitest';
import { htmlCanvas, htmlEffect, htmlCanvasSupported, htmlToSvg, context, testing } from '../src/index';
import { setup } from './setup';

beforeAll(setup);

describe('htmlCanvas', () => {
  it('reports no native HTML-in-Canvas support in happy-dom', () => {
    expect(htmlCanvasSupported()).toBe(false);
  });

  it('serialises an element into an SVG foreignObject with inlined styles and form state', async () => {
    const el = document.createElement('div');
    el.id = 'src';
    el.innerHTML = '<b class="x">Hi &amp; bye</b><input value="typed"><script>bad()</script>';
    el.style.color = 'rgb(255, 0, 0)';
    document.body.append(el);
    (el.querySelector('input') as HTMLInputElement).value = 'edited';
    const { url, width, height } = await htmlToSvg(el);
    const svg = decodeURIComponent(url.slice(url.indexOf(',') + 1));
    expect(url.startsWith('data:image/svg+xml')).toBe(true);
    expect(width).toBeGreaterThan(0);
    expect(height).toBeGreaterThan(0);
    expect(svg).toContain('<foreignObject');
    expect(svg).toContain('http://www.w3.org/1999/xhtml');
    expect(svg).toContain('Hi &amp; bye');
    expect(svg).toContain('value="edited"');
    expect(svg).not.toContain('bad()');
    expect(svg).not.toContain('id="src"');
    el.remove();
  });

  it('warns on unknown options and a bad target, throws on a missing element, and is tracked by context()', () => {
    const el = document.createElement('div');
    document.body.append(el);
    const warnings = testing.captureWarnings();
    let hc!: ReturnType<typeof htmlCanvas>;
    const ctx = context(() => {
      hc = htmlCanvas(el, { live: false, sacle: 2, target: {} } as never);
    });
    expect(warnings.some((w) => w.includes('unknown option "sacle"') && w.includes('scale'))).toBe(true);
    expect(warnings.some((w) => w.includes('`target`'))).toBe(true);
    expect(hc.canvas).toBeInstanceOf(HTMLCanvasElement);
    expect(hc.texture).toBeNull();
    expect(() => htmlCanvas('#nope')).toThrow(/matched nothing/);
    ctx.revert();
    el.remove();
  });
});

describe('htmlEffect', () => {
  it('validates the effect name and options', () => {
    const el = document.createElement('div');
    document.body.append(el);
    const warnings = testing.captureWarnings();
    const fx = htmlEffect(el, 'explode' as never, { duration: 800, colour: 'red' } as never);
    expect(warnings.some((w) => w.includes('unknown effect "explode"'))).toBe(true);
    expect(warnings.some((w) => w.includes('duration is 800 seconds'))).toBe(true);
    expect(warnings.some((w) => w.includes('unknown option "colour"'))).toBe(true);
    fx.revert();
    expect(el.style.opacity).toBe('');
    el.remove();
  });
});

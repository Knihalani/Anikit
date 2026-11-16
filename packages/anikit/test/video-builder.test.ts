import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { absolutize, video } from '../src/video/index';

const fixture = JSON.parse(readFileSync(join(process.cwd(), '../../crates/anikit-video/tests/fixtures/builder.json'), 'utf8'));

/** The same video as crates/anikit-py/tests/test_video_builder.py builds. */
function build() {
  return video({ size: '9:16', fps: 30 })
    .theme({ colors: { primary: '#7c5cff' } })
    .transition({ type: 'crossfade', duration: 0.4 })
    .marker('drop', 2)
    .scene((s) => s
      .text('Ship faster', { style: 'title', split: 'words', enter: { preset: 'fadeUp', stagger: 0.08 } })
      .shape('circle', { width: 200, height: 200, fill: '$primary', loop: 'float', animate: 'pop' }), { duration: 2.5 })
    .scene({ duration: 3, transition: { type: 'push', direction: 'left' } }, (s) => s
      .camera({ animate: [{ to: { zoom: 1.05 }, duration: 3, ease: 'inOutSine' }] })
      .box({ style: { direction: 'row', gap: 24 }, stagger: 0.1 }, (b) => b
        .text('Fast', { style: 'heading', animate: 'rise' })
        .text('Native', { style: 'heading', enter: 'fadeUp' }))
      .image('shot.png', { fit: 'cover', width: '80%', y: '70%' })
      .component('barChart', { values: [3, 5, 8] }, { enter: 'fade' }))
    .audio('music.mp3', { volume: 0.8, fadeOut: 1 });
}

describe('anikit/video builder', () => {
  it('produces the shared fixture spec', () => {
    expect(build().toJSON()).toEqual(fixture);
  });

  it('accepts a bare box style and preset aliases', () => {
    const spec = video().scene((s) => s.box({ direction: 'column', gap: 8 }, (b) => b.text('x', { animate: 'type' }))).spec();
    expect(spec.scenes[0].layers[0]).toEqual({ type: 'box', style: { direction: 'column', gap: 8 }, children: [{ text: 'x', enter: 'typewriter' }] });
  });

  it('makes relative asset paths absolute for rendering', () => {
    const spec = absolutize(build().toJSON(), '/proj', (a, b) => `${a}/${b}`);
    const img = spec.scenes[1].layers[1] as { src: string };
    expect(img.src).toBe('/proj/shot.png');
    expect(spec.audio?.[0].src).toBe('/proj/music.mp3');
  });

  it('explains a missing binary', async () => {
    await expect(build().render('out.mp4', { bin: '/nonexistent/anikit-video' })).rejects.toThrow(/not found.*ANIKIT_VIDEO_BIN/s);
  });
});

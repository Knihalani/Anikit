// TypeScript/JavaScript builder: node examples/video/build.mjs (after `node packages/anikit/scripts/build.mjs`
// and `cargo build --release -p anikit-video`; set ANIKIT_VIDEO_BIN=target/release/anikit-video).
import { video, template, render } from '../../packages/anikit/dist/video.js';

// 1. Hand-built with the fluent API.
const teaser = video({ size: '9:16', fps: 30 })
  .theme({ colors: { primary: '#7C5CFF', accent: '#FF5CA8' } })
  .scene((s) => s
    .text('Ship the video', { style: 'title', split: 'words', enter: { preset: 'fadeUp', stagger: 0.08, ease: 'outExpo' } })
    .text('before the meeting.', { style: 'subtitle', y: '60%', animate: 'rise' }), { duration: 2.6 })
  .scene((s) => s
    .text('{value:,} fps', { value: 935, style: 'title', enter: 'countUp' })
    .text('on a laptop', { style: 'subtitle', y: '62%', enter: { preset: 'fadeUp', delay: 0.6 } }), { duration: 3, transition: { type: 'push', direction: 'up' } });
console.log(JSON.stringify(teaser.toJSON()).length, 'bytes of spec');
await teaser.render('teaser.mp4');

// 2. From a template plus a brand kit, in all three formats.
const params = JSON.parse(await (await import('node:fs/promises')).readFile(new URL('./launch/params.json', import.meta.url), 'utf8'));
for (const format of ['16:9', '9:16', '1:1']) {
  const spec = await template('launch', params, { brand: new URL('./brand.json', import.meta.url).pathname, format });
  await render(spec, `launch-${format.replace(':', 'x')}.mp4`);
}

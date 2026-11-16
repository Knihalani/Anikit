import { buildPage } from './common.js';
import { init, ready, splitText, animate, stagger, scrollTrigger, magnetic, marquee } from '../../dist/anikit.lean.js';
init('anikit.wasm');
buildPage();
ready.then(() => {
  const { words } = splitText('.title', { type: 'words' });
  animate(words, { opacity: [0, 1], y: [30, 0] }, { duration: 0.8, delay: stagger(0.05), ease: 'outCubic' });
  for (const s of document.querySelectorAll('section.s')) {
    const cards = s.querySelectorAll('.card');
    const a = animate(cards, { opacity: [0, 1], y: [40, 0] }, { duration: 0.6, delay: stagger(0.06), autoplay: false });
    scrollTrigger({ trigger: s, start: 'top 80%', animation: a });
  }
  const pin = animate('.pin', { scale: [1, 0.8], opacity: [1, 0.6] }, { ease: 'linear', autoplay: false });
  scrollTrigger({ trigger: '.pin', start: 'top top', end: '+=800', pin: true, scrub: true, animation: pin });
  magnetic('.btn', { strength: 0.3 });
  marquee('.marquee', { speed: 80 });
  window.__ready = performance.now();
});

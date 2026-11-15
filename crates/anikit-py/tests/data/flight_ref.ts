// Regenerate flight_ref.json: node crates/anikit-py/tests/data/flight_ref.ts > crates/anikit-py/tests/data/flight_ref.json
import { flightPath } from '../../../../packages/anikit/src/flight.ts';
const out: any = {};
const a = flightPath([[0, 0, 2000], [300, 80, 0], [-200, -60, -3000], [0, 0, -6000]], { lookAhead: 500, bank: 1 });
out.open = {
  length: a.length,
  points: [0, 100, 1234.5, 4000, a.length, a.length + 50, -20].map((s) => a.point(s)),
  at: [0, 800, 2500, 5000, a.length - 100].map((s) => a.at(s)),
  tangent: [10, 3000].map((s) => a.tangent(s)),
};
const tm = a.timing([[0, 0], [2, 0.3], [5, 1]]);
out.open.timing = [-1, 0, 0.5, 1, 2, 3.3, 4.9, 5, 6].map(tm);
const tm2 = a.timing([[0, 0], [1, 0.5], [2, 0.5], [3, 1]]);
out.open.timing2 = [0.25, 1.5, 2.5].map(tm2);
const b = flightPath([[0, 300, 2000], [0, 120, -2000], [180, 60, -8000], [-500, 400, -12000]], { lookAhead: 900, bank: 1.4, maxBank: 30, closed: true });
out.closed = {
  length: b.length,
  points: [0, 5000, b.length + 1000, -700].map((s) => b.point(s)),
  at: [0, 3000, 9000, 20000].map((s) => b.at(s)),
  atOpts: b.at(3000, { lookAhead: 100, bank: 3 }),
};
const c = flightPath([{ x: 0, y: 0, z: 0 }, { x: 100, y: 0, z: 0 }]);
out.line = { length: c.length, at: c.at(50), one: c.timing([[1, 0.5]])(3) };
console.log(JSON.stringify(out));

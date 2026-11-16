// Tiny assertions for the browser tests (scripts/test-browser.mjs).
export function ok(cond, msg) {
  if (!cond) throw new Error(msg);
}
export function near(a, b, tol, msg) {
  if (!(Math.abs(a - b) <= tol)) throw new Error(`${msg}: ${a} not within ${tol} of ${b}`);
}

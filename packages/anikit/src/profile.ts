/**
 * Milliseconds per `renderer3d` pass, summed over every render since the
 * composition last cleared it (once per delivered frame). Internal: read it
 * through `RenderedFrame.timing.passes` or `anikit-render --profile`.
 */
export const passTimes: Record<string, number> = {};

/** Add `ms` to pass `name`. */
export function addPass(name: string, ms: number): void {
  passTimes[name] = (passTimes[name] ?? 0) + ms;
}

/** Meshes and instance buffers created so far (compositions warn when `draw()` keeps creating them). */
export const created = { meshes: 0, instances: 0 };

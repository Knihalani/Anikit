// Stub of the wasm module for running shatter.ts under Node: a fake linear
// memory holding one source mesh (id 0): positions at byte 0, indices after.
import { readFileSync } from 'node:fs';
const src = JSON.parse(readFileSync(new URL('./mesh.json', import.meta.url), 'utf8')) as { pos: number[]; idx: number[] };
const buffer = new ArrayBuffer(src.pos.length * 4 + src.idx.length * 4);
new Float32Array(buffer, 0, src.pos.length).set(src.pos);
new Uint32Array(buffer, src.pos.length * 4, src.idx.length).set(src.idx);
export const W = { memory: { buffer } };
export function core() {
  return {
    ak_mesh_ptr(_id: number, which: number): number {
      if (which === 0) return 0;
      if (which === 2) return src.pos.length * 4;
      if (which === 3) return src.pos.length / 3;
      if (which === 4) return src.idx.length / 3;
      return 0;
    },
  };
}

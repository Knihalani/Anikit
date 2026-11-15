// List members of exported option/result interfaces that have no JSDoc, so
// editor hover always explains them. Usage: node scripts/check-jsdoc.mjs
// (exits 1 when something is missing; files listed in SKIP are maintained
// elsewhere).
import ts from 'typescript';
import { join } from 'node:path';
import { pkg } from './snippets.mjs';

const SKIP = new Set(['mesh.ts', 'video.ts', 'scene.ts', 'cloud.ts', 'emitter.ts']);
const program = ts.createProgram([join(pkg, 'src/index.ts')], { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ESNext, strict: true });
const checker = program.getTypeChecker();
const index = program.getSourceFile(join(pkg, 'src/index.ts'));
const missing = [];
for (const sym of checker.getExportsOfModule(checker.getSymbolAtLocation(index))) {
  const target = sym.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(sym) : sym;
  for (const decl of target.declarations ?? []) {
    if (!ts.isInterfaceDeclaration(decl)) continue;
    const file = decl.getSourceFile().fileName.split('/').pop();
    if (SKIP.has(file)) continue;
    for (const m of decl.members) {
      if (!m.name || !(ts.isPropertySignature(m) || ts.isMethodSignature(m))) continue;
      const doc = ts.getJSDocCommentsAndTags(m);
      if (!doc.length) missing.push(`${file}: ${sym.name}.${m.name.getText()}`);
    }
  }
}
if (missing.length) {
  console.error(`${missing.length} public interface members without JSDoc:\n  ${missing.join('\n  ')}`);
  process.exit(1);
}
console.log('check-jsdoc: every member of the exported interfaces is documented');

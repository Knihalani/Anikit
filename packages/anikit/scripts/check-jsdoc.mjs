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
const documented = (m) => ts.getJSDocCommentsAndTags(m).length > 0 || (checker.getSymbolAtLocation(m.name)?.getDocumentationComment(checker).length ?? 0) > 0;
/** Members of an inline options object (`fn(x, o?: { a?: number })`). */
function checkParams(where, params) {
  for (const p of params) {
    let t = p.type;
    if (t && ts.isUnionTypeNode(t)) t = t.types.find((u) => ts.isTypeLiteralNode(u));
    if (!t || !ts.isTypeLiteralNode(t)) continue;
    for (const m of t.members) {
      if (m.name && (ts.isPropertySignature(m) || ts.isMethodSignature(m)) && !documented(m)) missing.push(`${where}(${p.name.getText()}.${m.name.getText()})`);
    }
  }
}
for (const sym of checker.getExportsOfModule(checker.getSymbolAtLocation(index))) {
  const target = sym.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(sym) : sym;
  for (const decl of target.declarations ?? []) {
    const file = decl.getSourceFile().fileName.split('/').pop();
    if (SKIP.has(file)) continue;
    if (ts.isInterfaceDeclaration(decl)) {
      for (const m of decl.members) {
        if (!m.name || !(ts.isPropertySignature(m) || ts.isMethodSignature(m))) continue;
        if (!documented(m)) missing.push(`${file}: ${sym.name}.${m.name.getText()}`);
      }
    } else if (ts.isFunctionDeclaration(decl)) {
      checkParams(`${file}: ${sym.name}`, decl.parameters);
    } else if (ts.isClassDeclaration(decl)) {
      for (const m of decl.members) {
        const priv = m.modifiers?.some((k) => k.kind === ts.SyntaxKind.PrivateKeyword || k.kind === ts.SyntaxKind.ProtectedKeyword);
        if (priv || !m.name || m.name.getText().startsWith('_')) continue;
        if (ts.isMethodDeclaration(m) || ts.isConstructorDeclaration(m)) checkParams(`${file}: ${sym.name}.${m.name?.getText() ?? 'constructor'}`, m.parameters);
      }
    } else if (ts.isVariableDeclaration(decl) && decl.initializer && (ts.isArrowFunction(decl.initializer) || ts.isFunctionExpression(decl.initializer))) {
      checkParams(`${file}: ${sym.name}`, decl.initializer.parameters);
    }
  }
}
if (missing.length) {
  console.error(`${missing.length} public interface members without JSDoc:\n  ${missing.join('\n  ')}`);
  process.exit(1);
}
console.log('check-jsdoc: every member of the exported interfaces is documented');

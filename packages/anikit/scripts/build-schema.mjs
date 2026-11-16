// Generate docs/api.schema.json: a JSON Schema (draft 2020-12) of every public
// options interface, read from the TypeScript types with the compiler API.
// Each property carries its JSDoc as `description` and its TS type as
// `tsType`; primitive, literal-union and array types map to JSON Schema.
//   node scripts/build-schema.mjs            write the file
//   node scripts/build-schema.mjs --check    fail if the file is stale
import ts from 'typescript';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = resolve(root, '../../docs/api.schema.json');
const PKG_OUT = resolve(root, 'api.schema.json'); // shipped in the npm package
const entries = { anikit: resolve(root, 'src/index.ts'), 'anikit/react': resolve(root, 'src/react.ts') };
const program = ts.createProgram(Object.values(entries), { target: ts.ScriptTarget.ES2020, moduleResolution: ts.ModuleResolutionKind.Bundler, strict: true, skipLibCheck: true });
const checker = program.getTypeChecker();

/** Options-like interfaces: their names end in Options or Props (plus a few known ones). */
const isOptionsName = (n) => /(Options|Props)$/.test(n) && n !== 'Props' && n !== 'TransformProps';

function schemaOf(type, depth = 0) {
  const tsType = checker.typeToString(type);
  const f = type.flags;
  if (f & ts.TypeFlags.Number || f & ts.TypeFlags.NumberLiteral) return type.isNumberLiteral() ? { const: type.value } : { type: 'number' };
  if (f & ts.TypeFlags.String) return { type: 'string' };
  if (f & ts.TypeFlags.StringLiteral) return { const: type.value };
  if (f & ts.TypeFlags.Boolean || f & ts.TypeFlags.BooleanLiteral) return { type: 'boolean' };
  if (type.isUnion()) {
    const parts = type.types.filter((t) => !(t.flags & (ts.TypeFlags.Undefined | ts.TypeFlags.Null)));
    if (parts.length && parts.every((t) => t.flags & ts.TypeFlags.BooleanLiteral)) return { type: 'boolean' };
    if (parts.length && parts.every((t) => t.isStringLiteral())) return { enum: parts.map((t) => t.value) };
    // Open string unions (names plus any string, e.g. easings): a string with examples.
    if (parts.some((t) => t.flags & ts.TypeFlags.String) || parts.filter((t) => t.isStringLiteral()).length > 12) {
      const lits = parts.filter((t) => t.isStringLiteral()).map((t) => t.value);
      const rest = parts.filter((t) => !t.isStringLiteral() && !(t.flags & ts.TypeFlags.String));
      const str = { type: 'string', ...(lits.length ? { examples: lits.slice(0, 12) } : {}) };
      return rest.length ? { anyOf: [str, ...rest.map((t) => schemaOf(t, depth + 1))] } : str;
    }
    if (parts.length === 1) return schemaOf(parts[0], depth);
    const any = parts.map((t) => schemaOf(t, depth + 1)).filter((s) => Object.keys(s).length);
    return any.length === parts.length ? { anyOf: any } : { tsType };
  }
  if (checker.isArrayType(type)) {
    const [el] = checker.getTypeArguments(type);
    return { type: 'array', items: el ? schemaOf(el, depth + 1) : {} };
  }
  if (type.getCallSignatures().length) return { tsType, 'x-function': true };
  const sname = type.getSymbol()?.getName() ?? '';
  if (f & ts.TypeFlags.Object && depth < 1 && (sname === '__type' || /Options$/.test(sname))) {
    const props = type.getProperties();
    if (props.length && props.length < 40) return { type: 'object', properties: propsOf(type, depth + 1), additionalProperties: false };
  }
  return { tsType };
}

function propsOf(type, depth) {
  const out = {};
  for (const p of type.getProperties()) {
    const decl = p.valueDeclaration ?? p.declarations?.[0];
    if (!decl) continue;
    const t = checker.getTypeOfSymbolAtLocation(p, decl);
    const s = { ...schemaOf(t, depth) };
    const doc = ts.displayPartsToString(p.getDocumentationComment(checker)).trim();
    if (doc) s.description = doc;
    s.tsType = checker.typeToString(t);
    out[p.getName()] = s;
  }
  return out;
}

const defs = {};
const where = {};
for (const [mod, file] of Object.entries(entries)) {
  const sf = program.getSourceFile(file);
  const sym = checker.getSymbolAtLocation(sf);
  for (const ex of checker.getExportsOfModule(sym)) {
    const name = ex.getName();
    if (!isOptionsName(name)) continue;
    const target = ex.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(ex) : ex;
    if (!(target.flags & (ts.SymbolFlags.Interface | ts.SymbolFlags.TypeAlias))) continue;
    const type = checker.getDeclaredTypeOfSymbol(target);
    const doc = ts.displayPartsToString(target.getDocumentationComment(checker)).trim();
    const open = !!checker.getIndexInfosOfType(type).length;
    defs[name] = { type: 'object', ...(doc ? { description: doc } : {}), 'x-module': mod, properties: propsOf(type, 0), additionalProperties: open };
    where[name] = mod;
  }
}

const sorted = Object.fromEntries(Object.keys(defs).sort().map((k) => [k, defs[k]]));
const schema = {
  $schema: 'https://json-schema.org/draft/2020-12/schema',
  $id: 'anikit/api.schema.json',
  title: 'anikit public options',
  description: 'Every options object of the public anikit API, generated from the TypeScript types by packages/anikit/scripts/build-schema.mjs. Properties carry their JSDoc (`description`) and TypeScript type (`tsType`); functions are marked `x-function`. Unknown keys are reported at runtime with an "anikit:" warning.',
  $defs: sorted,
};
const text = JSON.stringify(schema, null, 2) + '\n';
if (process.argv.includes('--check')) {
  let old = '';
  try { old = readFileSync(OUT, 'utf8') + readFileSync(PKG_OUT, 'utf8'); } catch {}
  old = old.length === 2 * text.length && old.startsWith(text) ? text : old;
  if (old !== text) {
    console.error('docs/api.schema.json is stale: run node scripts/build-schema.mjs');
    process.exit(1);
  }
  console.log(`api.schema.json: up to date (${Object.keys(sorted).length} option types)`);
} else {
  writeFileSync(OUT, text);
  writeFileSync(PKG_OUT, text);
  console.log(`api.schema.json: ${Object.keys(sorted).length} option types → ${OUT}`);
}

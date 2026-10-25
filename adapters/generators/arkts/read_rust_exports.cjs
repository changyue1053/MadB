'use strict';
const fs = require('node:fs');
const crypto = require('node:crypto');
const ts = require(process.argv[2]);
const records = JSON.parse(fs.readFileSync(0, 'utf8'));
if (!Array.isArray(records) || records.length === 0 || records.some(r => r.kind !== 'fn' || typeof r.def !== 'string')) {
  throw new Error('restricted Rust backend metadata requires function records');
}
const text = records.map(r => r.def + ';').join('\n');
const source = ts.createSourceFile('napi.raw.d.ts', text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
if (source.parseDiagnostics.length !== 0) throw new Error('invalid emitted backend declarations');
if (source.statements.length !== records.length) throw new Error('metadata/declaration count mismatch');
const names = new Set();
const rawExports = source.statements.map((node, index) => {
  if (!ts.isFunctionDeclaration(node) || !node.name || !ts.isIdentifier(node.name) || node.body || node.typeParameters?.length) throw new Error('unsupported raw declaration');
  if (node.type?.kind !== ts.SyntaxKind.UnknownKeyword) throw new Error('raw return must remain unknown');
  const name = node.name.text;
  if (name !== records[index].name || names.has(name)) throw new Error('duplicate/mismatched macro export name');
  names.add(name);
  const parameters = node.parameters.map(parameter => {
    if (!ts.isIdentifier(parameter.name) || parameter.type?.kind !== ts.SyntaxKind.UnknownKeyword || parameter.questionToken || parameter.dotDotDotToken || parameter.initializer) throw new Error('unsupported raw parameter');
    return {name: parameter.name.text, rawType: 'unknown'};
  });
  return {name, arity: parameters.length, parameters, rawReturn: 'unknown', backendRecord: index, location: {path: 'napi.raw.d.ts', start: node.getStart(source), end: node.end, offsets: 'utf16'}};
});
process.stdout.write(JSON.stringify({authority: 'actual napi proc-macro JSONL parsed with TypeScript AST; raw binding surface only', typescriptVersion: ts.version, generatedDeclarationSha256: crypto.createHash('sha256').update(text).digest('hex'), exports: rawExports}) + '\n');

'use strict';
// Real authored source variants, not manufactured compiler descriptors.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const root = path.resolve(__dirname, '../../..');
const {readFacade, readValueDeclarations} = require(path.join(root, 'adapters/generators/arkts/read_facade.cjs'));
const [compiler, expectedDigest, output, operation = 'variants'] = process.argv.slice(2);
const sha = data => crypto.createHash('sha256').update(data).digest('hex');
assert.equal(process.version.slice(1), fs.readFileSync(path.join(root, '.node-version'), 'utf8').trim());
assert.ok(path.isAbsolute(compiler || '') && path.isAbsolute(output || ''));
assert.match(expectedDigest || '', /^[a-f0-9]{64}$/);
assert.equal(sha(fs.readFileSync(compiler)), expectedDigest);
const ts = require(compiler);
if (operation === 'declaration') {
  const descriptor = readValueDeclarations(ts, fs.readFileSync(output), path.basename(output), 'declaration');
  assert.equal(sha(fs.readFileSync(compiler)), expectedDigest);
  console.log(JSON.stringify(descriptor));
  process.exit(0);
}
if (operation === 'typecheck') {
  const program = ts.createProgram([path.join(output, 'Consumer.ts')], {
    strict: true, noEmit: true, target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS,
  });
  const diagnostics = ts.getPreEmitDiagnostics(program);
  if (diagnostics.length) throw new Error(ts.formatDiagnostics(diagnostics, {
    getCanonicalFileName: name => name, getCurrentDirectory: () => output, getNewLine: () => '\n',
  }));
  assert.equal(sha(fs.readFileSync(compiler)), expectedDigest);
  console.log('generated declarations consumed by locked TypeScript compiler');
  process.exit(0);
}
assert.equal(operation, 'variants');
fs.mkdirSync(output);
const original = fs.readFileSync(path.join(root, 'docs/examples/language-api/second-business.ets'), 'utf8');
const added = original + '\nexport function describe(value: Reading): string { return value.label; }\n';
const field = added.replace('label: string; }', 'label: string; revision: number; }')
  .replace('label: value.label.trim() }', 'label: value.label.trim(), revision: value.revision }');
assert.notEqual(field, added);
const renamed = field.replace(/\bReading\b/g, 'Measurement').replace(/\bnormalize\b/g, 'normalizeReading')
  .replace(/\bdescribe\b/g, 'describeReading');
const deleted = renamed.replace('export function describeReading(value: Measurement): string { return value.label; }\n', '');
assert.notEqual(deleted, renamed);
const stages = [
  ['initial', original, 'Reading', ['normalize'], ['value', 'label']],
  ['add-function', added, 'Reading', ['normalize', 'describe'], ['value', 'label']],
  ['add-field', field, 'Reading', ['normalize', 'describe'], ['value', 'label', 'revision']],
  ['rename', renamed, 'Measurement', ['normalizeReading', 'describeReading'], ['value', 'label', 'revision']],
  ['delete-function', deleted, 'Measurement', ['normalizeReading'], ['value', 'label', 'revision']],
];
const evidence = {scope: 'actual compiler AST read; no native or target execution', compiler: {path: compiler,
  sha256: expectedDigest, version: ts.version}, node: process.version, stages: [], rejections: [], c07: []};
for (const [name, source, record, functions, fields] of stages) {
  const dir = path.join(output, name); fs.mkdirSync(dir);
  const filename = path.join(dir, 'Business.ets'); fs.writeFileSync(filename, source);
  const bytes = fs.readFileSync(filename), logical = `${name}/Business.ets`;
  const descriptor = readValueDeclarations(ts, bytes, logical);
  assert.deepEqual(descriptor.records.map(r => r.name), [record]);
  assert.deepEqual(descriptor.records[0].fields.map(f => f.name), fields);
  assert.deepEqual(descriptor.functions.map(f => f.name), functions);
  assert.deepEqual(descriptor.resources, []); assert.deepEqual(descriptor.errors, []);
  assert.throws(() => readFacade(ts, bytes, logical), /public declaration counts outside restricted profile/);
  fs.writeFileSync(path.join(dir, 'descriptor.json'), JSON.stringify(descriptor, null, 2) + '\n');
  evidence.stages.push({name, source: filename, sha256: sha(bytes), record, functions, fields});
}
const negatives = [
  ['resource', original + '\nexport interface Owner { close(): void; }', 'does not support resources'],
  ['promise', original.replace('): Reading {', '): Promise<Reading> {'), 'only synchronous scalar/record values'],
  ['callback', original + '\nexport function observe(listener: (value: Reading) => void): void {}', 'only synchronous scalar/record values'],
  ['import', 'import { Foreign } from "foreign";\n' + original, 'does not support imports'],
  ['import-equals', 'import Foreign = require("foreign");\n' + original, 'does not support imports'],
  ['optional', original.replace('label: string', 'label?: string'), 'record property must be required'],
  ['unknown-type', original.replace('): Reading {', '): Missing {'), 'unknown public type reference'],
  ['duplicate', original + '\nexport function normalize(value: Reading): Reading { return value; }', 'duplicate declaration/member'],
  ['mixed', original.replace('label: string;', 'label: string; close(): void;'), 'mixed record/resource interface'],
];
for (const [name, source, message] of negatives) {
  const logical = `reject-${name}/Business.ets`;
  assert.throws(() => readValueDeclarations(ts, Buffer.from(source), logical), error => {
    assert.ok(error.message.startsWith(logical + ':')); assert.ok(error.message.includes(message));
    assert.match(error.message.slice(logical.length + 1), /^[1-9][0-9]*:[1-9][0-9]*: /);
    evidence.rejections.push({name, message: error.message}); return true;
  });
}
function concrete(bytes) { return Buffer.from(bytes.toString().replace('@MADB_NATIVE_MODULE@', '"libreader.so"')
  .replace('@MADB_EXPECTED_IDENTITY@', '"reader-only"')); }
for (const [relative, record] of [['sdk/arkts/facade/Boundary.ets.in', 'Snapshot'],
  ['tests/fixtures/c07_arkts_generation/rename-record/Boundary.ets.in', 'Parcel']]) {
  const bytes = concrete(fs.readFileSync(path.join(root, relative)));
  assert.equal(readFacade(ts, bytes, relative).records[0].name, record); evidence.c07.push(relative);
}
for (const [name, code] of [['reject-async', 'ARKTS_DECL_UNSUPPORTED_MODIFIERS'],
  ['reject-async-listener', 'ARKTS_DECL_CALLBACK_RESULT'], ['reject-callback-result', 'ARKTS_DECL_CALLBACK_RESULT']]) {
  const relative = `tests/fixtures/c07_arkts_generation/${name}/Boundary.ets.in`;
  assert.throws(() => readFacade(ts, concrete(fs.readFileSync(path.join(root, relative))), relative),
    error => error.message.startsWith(relative + ':') && error.message.includes(code));
  evidence.c07.push(relative);
}
assert.equal(sha(fs.readFileSync(compiler)), expectedDigest);
fs.writeFileSync(path.join(output, 'reader-checks.json'), JSON.stringify(evidence, null, 2) + '\n');
console.log(JSON.stringify({stages: stages.length, rejections: negatives.length, c07: evidence.c07.length}));

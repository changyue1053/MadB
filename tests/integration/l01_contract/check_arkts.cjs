'use strict';
// Execute the existing AST reader against real authored source. No target code runs.
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../../..');
assert.equal(process.version.slice(1), fs.readFileSync(path.join(root, '.node-version'), 'utf8').trim(),
  'use repository-pinned Node');
const {readFacade} = require(path.join(root, 'adapters/generators/arkts/read_facade.cjs'));
const [modulePath, expectedHash] = process.argv.slice(2);
assert.ok(path.isAbsolute(modulePath || ''), 'provide an absolute reviewed TypeScript module');
assert.match(expectedHash || '', /^[a-f0-9]{64}$/, 'provide its expected SHA-256');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const moduleBytes = fs.readFileSync(modulePath);
assert.equal(sha(moduleBytes), expectedHash, 'compiler module bytes differ');
const ts = require(modulePath);
const inputs = [];
const cases = [];
function read(relative) {
  const bytes = fs.readFileSync(path.join(root, relative));
  inputs.push({path: relative, sha256: sha(bytes)});
  // The same two literals are populated by the production driver. Their values
  // do not add declarations, a real native module, or runtime identity evidence.
  return Buffer.from(bytes.toString('utf8')
    .replace('@MADB_NATIVE_MODULE@', '"libl01-reader-only.so"')
    .replace('@MADB_EXPECTED_IDENTITY@', '"l01-reader-only"'));
}
function parse(relative) {
  return readFacade(ts, read(relative), relative);
}
const ordinary = parse('sdk/arkts/facade/Boundary.ets.in');
assert.equal(ordinary.records[0].name, 'Snapshot');
assert.deepEqual(ordinary.records[0].fields.map(f => [f.name, f.type.tag]), [['value', 'number'], ['text', 'string']]);
const state = ordinary.resources.find(r => r.name === 'Engine').methods.find(m => m.name === 'observeState');
assert.equal(state.params[0].type.returns.tag, 'void');
cases.push('ordinary authored surface retains scalar record and synchronous void listener');
const renamed = parse('tests/fixtures/c07_arkts_generation/rename-record/Boundary.ets.in');
assert.equal(renamed.records[0].name, 'Parcel');
assert.ok(renamed.resources.find(r => r.name === 'Engine').methods.some(m => m.name === 'copyRecord'));
assert.ok(!renamed.resources.find(r => r.name === 'Engine').methods.some(m => m.name === 'roundtrip'));
cases.push('real rename changes record and method source names');
for (const [fixture, classification] of [
  ['reject-async', 'ARKTS_DECL_UNSUPPORTED_MODIFIERS'],
  ['reject-async-listener', 'ARKTS_DECL_CALLBACK_RESULT'],
  ['reject-callback-result', 'ARKTS_DECL_CALLBACK_RESULT'],
]) {
  const relative = `tests/fixtures/c07_arkts_generation/${fixture}/Boundary.ets.in`;
  assert.throws(() => parse(relative), error => {
    assert.ok(error.message.startsWith(relative + ':'), 'diagnostic names actual source');
    assert.match(error.message.slice(relative.length + 1), /^[1-9][0-9]*:[1-9][0-9]*: /);
    assert.ok(error.message.includes(classification));
    return true;
  });
  cases.push(fixture + ' rejects at its actual source node with required classification');
}
const second = 'docs/examples/language-api/second-business.ets';
assert.throws(() => parse(second), error => {
  assert.ok(error.message.startsWith(second + ':'));
  assert.ok(error.message.includes('public declaration counts outside restricted profile'));
  return true;
});
cases.push('independent value-only business remains rejected by the fixed C07 profile');
assert.equal(sha(fs.readFileSync(modulePath)), expectedHash, 'compiler module changed during check');
process.stdout.write(JSON.stringify({
  scope: 'L01 host AST reader; no target compilation, native execution or SDK acceptance',
  node: process.version, compilerApiVersion: ts.version,
  compilerModule: {path: modulePath, sha256: expectedHash},
  reader: {path: 'adapters/generators/arkts/read_facade.cjs', sha256: sha(fs.readFileSync(path.join(root, 'adapters/generators/arkts/read_facade.cjs')))},
  inputs, passed: cases.length, cases,
}, null, 2) + '\n');

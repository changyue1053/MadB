// Native ECMAScript serialization is an independent RFC 8785 numeric/string oracle.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {createHash} = require('node:crypto');
const fixtures = JSON.parse(fs.readFileSync(path.join(__dirname, '../fixtures/contracts/canonical-vectors.json'), 'utf8'));
function canonical(value) {
  if (Array.isArray(value)) return '[' + value.map(canonical).join(',') + ']';
  if (value !== null && typeof value === 'object') {
    return '{' + Object.keys(value).sort().map(key => JSON.stringify(key) + ':' + canonical(value[key])).join(',') + '}';
  }
  if (typeof value === 'number') assert(Number.isSafeInteger(value));
  return JSON.stringify(value);
}
for (const vector of fixtures.vectors) {
  const bytes = canonical(vector.input);
  assert.equal(bytes, vector.canonical, vector.id);
  const hash = createHash('sha256').update(fixtures.domain + '\n' + bytes).digest('hex');
  assert.equal('sha256:' + hash, vector.sha256, vector.id);
}
console.log(`Canonical vectors: ${fixtures.vectors.length} passed in Node; no raw JSON parser claim.`);

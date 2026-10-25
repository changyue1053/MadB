'use strict';
// Private C04 reader slice. Syntax comes from the locked SDK compiler AST;
// ownership/ABI roles require a separate reviewed policy and are never inferred.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const {TextDecoder} = require('node:util');
const hash = b => 'sha256:' + crypto.createHash('sha256').update(b).digest('hex');
const MAX_SOURCE = 1024 * 1024;
function checkedFile(file, sha, size, maximum = Infinity) {
  if (!/^sha256:[0-9a-f]{64}$/.test(sha) || !/^(0|[1-9][0-9]*)$/.test(size))
    throw new Error('invalid frozen file identity');
  const n = Number(size);
  if (!Number.isSafeInteger(n) || n > maximum) throw new Error('file budget exceeded');
  const stat = fs.statSync(file);
  if (!stat.isFile() || stat.size !== n) throw new Error('frozen input size changed: ' + file);
  const bytes = fs.readFileSync(file);
  if (hash(bytes) !== sha) throw new Error('frozen input hash changed: ' + file);
  return bytes;
}
function sourcePath(root, logical) {
  if (!path.isAbsolute(root) || !logical || logical.includes('\\') || logical.includes('\0')
      || path.isAbsolute(logical) || logical.split('/').some(p => ['', '.', '..'].includes(p)))
    throw new Error('invalid frozen source path');
  if (fs.lstatSync(root).isSymbolicLink() || !fs.statSync(root).isDirectory())
    throw new Error('source root must be an ordinary directory');
  let current = fs.realpathSync(root);
  for (const part of logical.split('/')) {
    current = path.join(current, part);
    if (fs.lstatSync(current).isSymbolicLink()) throw new Error('source symlink is unsupported');
  }
  return current;
}
function readDeclarations(ts, bytes, logical) {
  const text = new TextDecoder('utf-8', {fatal: true}).decode(bytes);
  const sf = ts.createSourceFile(logical, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  function location(node) {
    const start = node.getStart(sf), end = node.getEnd();
    const a = sf.getLineAndCharacterOfPosition(start), b = sf.getLineAndCharacterOfPosition(end);
    return {path: logical, start, end, line: a.line + 1, column: a.character + 1,
      endLine: b.line + 1, endColumn: b.character + 1, offsets: 'utf16'};
  }
  function fail(node, reason) {
    const loc = location(node);
    throw new Error(`${logical}:${loc.line}:${loc.column}: ${reason}`);
  }
  if (sf.parseDiagnostics.length) {
    const d = sf.parseDiagnostics[0], loc = sf.getLineAndCharacterOfPosition(d.start || 0);
    throw new Error(`${logical}:${loc.line + 1}:${loc.character + 1}: ${ts.flattenDiagnosticMessageText(d.messageText, ' ')}`);
  }
  if (sf.referencedFiles.length || sf.typeReferenceDirectives.length || sf.libReferenceDirectives.length
      || sf.hasNoDefaultLib) fail(sf, 'external or special reference directives are unsupported');
  function identifier(node) {
    if (!node || !ts.isIdentifier(node) || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(node.text))
      fail(node || sf, 'expected a plain ASCII identifier');
    return node.text;
  }
  function modifiers(node, expected) {
    const got = Array.from(node.modifiers || [], m => m.kind);
    if (got.length !== expected.length || expected.some((k, i) => got[i] !== k))
      fail(node, 'unsupported declaration modifiers');
  }
  function bounded(values, node, what, maximum = 64) {
    if (!values.length || values.length > maximum) fail(node, `${what} count is out of bounds`);
  }
  function unique(values, node, what) {
    const seen = new Set();
    for (const value of values) {
      if (seen.has(value.name)) fail(node, 'duplicate ' + what + ': ' + value.name);
      seen.add(value.name);
    }
  }
  function type(node, depth = 0) {
    if (!node || depth > 8) fail(node || sf, 'missing or too deeply nested type');
    if (node.kind === ts.SyntaxKind.NumberKeyword) return {tag: 'number'};
    if (node.kind === ts.SyntaxKind.StringKeyword) return {tag: 'string'};
    if (node.kind === ts.SyntaxKind.VoidKeyword) return {tag: 'void'};
    if (ts.isTypeReferenceNode(node) && !node.typeArguments?.length)
      return {tag: 'reference', name: identifier(node.typeName)};
    if (ts.isArrayTypeNode(node) && node.elementType.kind === ts.SyntaxKind.NumberKeyword)
      return {tag: 'array', item: {tag: 'number'}};
    if (ts.isUnionTypeNode(node) && node.types.length === 2) {
      const nil = node.types.filter(t => ts.isLiteralTypeNode(t) && t.literal.kind === ts.SyntaxKind.NullKeyword);
      const other = node.types.filter(t => !nil.includes(t));
      if (nil.length === 1 && other.length === 1 && ts.isTypeReferenceNode(other[0]))
        return {tag: 'nullable', item: type(other[0], depth + 1)};
    }
    fail(node, 'unsupported type syntax: ' + ts.SyntaxKind[node.kind]);
  }
  function parameters(node) {
    if (node.typeParameters?.length || node.parameters.length > 8) fail(node, 'generic or oversized callable');
    const params = Array.from(node.parameters, p => {
      modifiers(p, []);
      if (p.dotDotDotToken || p.questionToken || p.initializer) fail(p, 'rest/optional/default parameter is unsupported');
      const value = {name: identifier(p.name), type: type(p.type), location: location(p)};
      if (value.name === 'this' || value.type.tag === 'void') fail(p, 'this/void parameter is unsupported');
      return value;
    });
    unique(params, node, 'parameter');
    return params;
  }
  const records = [], callbacks = [], variables = [], defaults = [];
  for (const node of sf.statements) {
    if (ts.isInterfaceDeclaration(node)) {
      modifiers(node, [ts.SyntaxKind.ExportKeyword]);
      if (node.typeParameters?.length || node.heritageClauses?.length) fail(node, 'generic/inherited record is unsupported');
      bounded(node.members, node, 'record field');
      const fields = Array.from(node.members, m => {
        if (!ts.isPropertySignature(m) || m.questionToken || m.initializer) fail(m, 'expected required record property');
        modifiers(m, []);
        const t = type(m.type);
        if (!['number', 'string'].includes(t.tag)) fail(m, 'record fields must be scalar number/string');
        return {name: identifier(m.name), type: t, location: location(m)};
      });
      unique(fields, node, 'record field');
      records.push({name: identifier(node.name), fields, location: location(node)});
    } else if (ts.isTypeAliasDeclaration(node)) {
      modifiers(node, [ts.SyntaxKind.ExportKeyword]);
      if (node.typeParameters?.length || !ts.isFunctionTypeNode(node.type)) fail(node, 'only non-generic callback aliases are supported');
      const returns = type(node.type.type);
      if (returns.tag !== 'void') fail(node, 'callback return must be void');
      callbacks.push({name: identifier(node.name), params: parameters(node.type), returns, location: location(node)});
    } else if (ts.isVariableStatement(node)) {
      modifiers(node, [ts.SyntaxKind.DeclareKeyword]);
      if ((node.declarationList.flags & ts.NodeFlags.BlockScoped) !== ts.NodeFlags.Const || node.declarationList.declarations.length !== 1)
        fail(node, 'expected one declared const');
      const d = node.declarationList.declarations[0];
      if (d.initializer || d.exclamationToken || !d.type || !ts.isTypeLiteralNode(d.type))
        fail(d, 'default binding must have a method-only type literal');
      bounded(d.type.members, d, 'method');
      const methods = Array.from(d.type.members, m => {
        if (!ts.isMethodSignature(m) || m.questionToken) fail(m, 'expected required method signature');
        modifiers(m, []);
        return {name: identifier(m.name), params: parameters(m), returns: type(m.type), location: location(m)};
      });
      unique(methods, d, 'method');
      variables.push({name: identifier(d.name), methods, location: location(d)});
    } else if (ts.isExportAssignment(node)) {
      modifiers(node, []);
      if (node.isExportEquals) fail(node, 'export= is unsupported');
      defaults.push(identifier(node.expression));
    } else fail(node, 'unsupported top-level declaration: ' + ts.SyntaxKind[node.kind]);
  }
  if (variables.length !== 1 || defaults.length !== 1 || variables[0].name !== defaults[0])
    fail(sf, 'expected exactly one default-exported declared binding');
  if (records.length + callbacks.length > 64) fail(sf, 'type declaration budget exceeded');
  unique([...records, ...callbacks, variables[0]], sf, 'declaration');
  const declaredRecords = new Set(records.map(v => v.name));
  const declaredCallbacks = new Set(callbacks.map(v => v.name));
  function validateReference(t, loc, allowCallback) {
    const bad = reason => { throw new Error(`${logical}:${loc.line}:${loc.column}: ${reason}`); };
    if (t.tag === 'reference' && !declaredRecords.has(t.name)
        && !(allowCallback && declaredCallbacks.has(t.name))) bad('unknown or unsupported type reference: ' + t.name);
    if (t.tag === 'nullable' && !declaredRecords.has(t.item.name)) bad('nullable type must reference a declared record');
  }
  for (const c of callbacks) for (const p of c.params) validateReference(p.type, p.location, false);
  for (const m of variables[0].methods) {
    for (const p of m.params) validateReference(p.type, p.location, true);
    validateReference(m.returns, m.location, false);
  }
  return {authority: 'declared-arkts-syntax', defaultExport: variables[0].name,
    records, callbacks, methods: variables[0].methods};
}
function main(args) {
  if (args.length !== 9) throw new Error('expected root logical sourceSha sourceSize tsPath tsSha tsSize nodeSha nodeSize');
  const [root, logical, sourceSha, sourceSize, tsPath, tsSha, tsSize, nodeSha, nodeSize] = args;
  if (!path.isAbsolute(tsPath)) throw new Error('compiler path must be absolute');
  const file = sourcePath(root, logical);
  const source = checkedFile(file, sourceSha, sourceSize, MAX_SOURCE);
  checkedFile(process.execPath, nodeSha, nodeSize);
  checkedFile(tsPath, tsSha, tsSize);
  const readerBytes = fs.readFileSync(__filename);
  const ts = require(tsPath);
  const result = readDeclarations(ts, source, logical);
  checkedFile(file, sourceSha, sourceSize, MAX_SOURCE);
  checkedFile(tsPath, tsSha, tsSize);
  checkedFile(process.execPath, nodeSha, nodeSize);
  if (!fs.readFileSync(__filename).equals(readerBytes)) throw new Error('reader changed during invocation');
  result.schema = 'madb.private.arkts-declarations.v1';
  result.source = {path: logical, sha256: sourceSha, size: Number(sourceSize)};
  result.tools = {
    node: {version: process.versions.node, sha256: nodeSha, size: Number(nodeSize)},
    typescript: {version: ts.version, sha256: tsSha, size: Number(tsSize)},
    reader: {sha256: hash(readerBytes), size: readerBytes.length}
  };
  // This is a syntax descriptor, not type-checking, ContractIR, ABI, or capability proof.
  return result;
}
if (require.main === module) {
  try { process.stdout.write(JSON.stringify(main(process.argv.slice(2)), null, 2) + '\n'); }
  catch (error) { process.stderr.write(String(error.message || error) + '\n'); process.exitCode = 1; }
}
module.exports = {readDeclarations};

'use strict';
// Fixed SDK source inspection, not a general ArkTS or lifecycle proof engine.
const fs = require('node:fs');
const {TextDecoder} = require('node:util');

function inspect(ts, bytes, logical, mode) {
  if (!['state', 'ui'].includes(mode) || bytes.length > 1024 * 1024) throw new Error('unsupported SDK source or budget');
  const sf = ts.createSourceFile(logical, new TextDecoder('utf-8', {fatal: true}).decode(bytes),
    ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  function fail(node, message) {
    const pos = sf.getLineAndCharacterOfPosition(node.getStart(sf));
    throw new Error(`${logical}:${pos.line + 1}:${pos.character + 1}: ${message}`);
  }
  if (sf.parseDiagnostics.length) fail(sf, 'invalid SDK syntax');
  const imports = [], exports = [], classes = new Map();
  for (const node of sf.statements) {
    if (ts.isImportDeclaration(node)) {
      if (!ts.isStringLiteral(node.moduleSpecifier)) fail(node, 'nonliteral SDK import');
      imports.push(node.moduleSpecifier.text);
    } else if (ts.isImportEqualsDeclaration(node) || ts.isExportDeclaration(node) || ts.isExportAssignment(node)) {
      fail(node, 'unsupported SDK alias or reexport');
    }
    if (!(node.modifiers || []).some(m => m.kind === ts.SyntaxKind.ExportKeyword)) continue;
    if (!node.name || !ts.isIdentifier(node.name)) fail(node, 'unnamed SDK export');
    exports.push(node.name.text);
    if (ts.isClassDeclaration(node)) classes.set(node.name.text, node);
    else if (!ts.isTypeAliasDeclaration(node)) fail(node, 'unexpected SDK export');
  }
  const expectedImports = mode === 'state' ? ['./Boundary'] : ['./Boundary', './StateObservation'];
  const expectedExports = mode === 'state' ? ['ObservationPhase', 'StateObservationError', 'StateObservation'] : ['ArkUIStateModel'];
  if (JSON.stringify(imports) !== JSON.stringify(expectedImports) || JSON.stringify(exports) !== JSON.stringify(expectedExports)) {
    fail(sf, 'SDK import/export closure differs from fixed recipe');
  }
  const name = mode === 'state' ? 'StateObservation' : 'ArkUIStateModel';
  const owner = classes.get(name);
  if (!owner) fail(sf, 'missing SDK owner');
  function method(member, parameters, result) {
    const matches = owner.members.filter(n => ts.isMethodDeclaration(n) && n.name?.text === member);
    if (matches.length !== 1) fail(owner, 'missing or duplicate SDK method ' + member);
    const node = matches[0];
    if (node.type?.getText(sf) !== result || JSON.stringify(node.parameters.map(p => p.type?.getText(sf))) !== JSON.stringify(parameters)) {
      fail(node, 'SDK method shape differs: ' + member);
    }
    return name + '.' + member;
  }
  const methods = [method('attach', mode === 'state' ? ['Engine', '(value: Snapshot) => void'] : ['Engine'], 'void'),
    method('requestStop', [], 'void'), method('close', [], 'Promise<void>')];
  function decorators(node) {
    return Array.from(node.modifiers || []).filter(m => m.kind === ts.SyntaxKind.Decorator)
      .map(m => m.expression.getText(sf));
  }
  if (mode === 'ui') {
    const snapshots = owner.members.filter(n => ts.isPropertyDeclaration(n) && n.name?.text === 'snapshot');
    if (JSON.stringify(decorators(owner)) !== '["ObservedV2"]' || snapshots.length !== 1
      || JSON.stringify(decorators(snapshots[0])) !== '["Trace"]' || snapshots[0].type?.getText(sf) !== 'Snapshot | undefined') {
      fail(owner, 'SDK observed snapshot differs from fixed recipe');
    }
  }
  return {imports, exports, methods, compiler: ts.version};
}

if (require.main === module) {
  try {
    if (process.argv.length !== 5) throw new Error('expected locked compiler, logical source, state/ui');
    const value = inspect(require(process.argv[2]), fs.readFileSync(0), process.argv[3], process.argv[4]);
    process.stdout.write(JSON.stringify(value) + '\n');
  } catch (error) { process.stderr.write(String(error.message || error) + '\n'); process.exitCode = 1; }
}
module.exports = {inspect};

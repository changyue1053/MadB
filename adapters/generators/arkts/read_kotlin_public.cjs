'use strict';
const fs = require('node:fs');
const ts = require(process.argv[2]);
const source = ts.createSourceFile(process.argv[3], fs.readFileSync(process.argv[3], 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
if (source.parseDiagnostics.length) throw new Error('Invalid public declaration source');
const interfaces = {};
for (const node of source.statements) {
  if (!ts.isInterfaceDeclaration(node) || !node.modifiers?.some(x=>x.kind===ts.SyntaxKind.ExportKeyword)) continue;
  const members = node.members.map(member => {
    if (!(ts.isMethodSignature(member)||ts.isPropertySignature(member)) || !member.name || !ts.isIdentifier(member.name)) throw new Error('Unsupported public member');
    return member.name.text;
  });
  if (new Set(members).size !== members.length) throw new Error('Overloaded public member unsupported');
  interfaces[node.name.text] = members.sort();
}
process.stdout.write(JSON.stringify({typescriptVersion:ts.version,interfaces})+'\n');

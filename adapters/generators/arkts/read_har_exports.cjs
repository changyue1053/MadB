'use strict';
// Read the actual compiler-emitted HAR entry's finite named re-export closure.
const fs=require('node:fs');const ts=require(process.argv[2]);
try {
  if(process.argv.length!==4)throw new Error('expected compiler module and logical entry path');
  const bytes=fs.readFileSync(0);if(bytes.length>1024*1024)throw new Error('entry budget exceeded');
  const text=new TextDecoder('utf-8',{fatal:true}).decode(bytes);
  const sf=ts.createSourceFile(process.argv[3],text,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
  if(sf.parseDiagnostics.length)throw new Error('HAR export entry has compiler parse errors');
  const exports=[],seen=new Set();
  for(const node of sf.statements){
    if(!ts.isExportDeclaration(node)||node.modifiers?.length||!node.exportClause||!ts.isNamedExports(node.exportClause)||!node.moduleSpecifier||!ts.isStringLiteral(node.moduleSpecifier))throw new Error('unsupported HAR entry declaration');
    const names=[];
    for(const e of node.exportClause.elements){
      if(e.propertyName||!ts.isIdentifier(e.name)||seen.has(e.name.text))throw new Error('aliased/duplicate HAR public export');
      seen.add(e.name.text);names.push(e.name.text);
    }
    if(!names.length)throw new Error('empty HAR public export group');
    exports.push({module:node.moduleSpecifier.text,names});
  }
  if(!exports.length||exports.length>16)throw new Error('HAR export group count outside budget');
  process.stdout.write(JSON.stringify({exports})+'\n');
}catch(e){process.stderr.write(String(e.message||e)+'\n');process.exitCode=1;}

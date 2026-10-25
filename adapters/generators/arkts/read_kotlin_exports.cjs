'use strict';
const fs=require('node:fs');
const ts=require(process.argv[2]);
const input=JSON.parse(fs.readFileSync(0,'utf8'));
const source=ts.createSourceFile('record-export.d.ts',input.declaration,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
if(source.parseDiagnostics.length)throw new Error('KSP declaration syntax error');
const facts=input.facts;
if(facts.schema!=='madb.kotlin-ksp-facts/1')throw new Error('Unsupported KSP facts');
const short=n=>n.slice(n.lastIndexOf('.')+1);
const translate=n=>({'kotlin.Int':'number','kotlin.String':'string','kotlin.Boolean':'boolean','kotlin.Unit':'void'}[n]??short(n));
const type=node=>{
  if(node?.kind===ts.SyntaxKind.NumberKeyword)return 'number';
  if(node?.kind===ts.SyntaxKind.StringKeyword)return 'string';
  if(node?.kind===ts.SyntaxKind.BooleanKeyword)return 'boolean';
  if(node?.kind===ts.SyntaxKind.VoidKeyword)return 'void';
  if(node&&ts.isTypeReferenceNode(node)&&ts.isIdentifier(node.typeName)&&!node.typeArguments?.length)return node.typeName.text;
  throw new Error('Unsupported KSP TypeScript type');
};
const loc=node=>({path:'record-export.d.ts',start:node.getStart(source),end:node.end,offsets:'utf16'});
const method=node=>{
 if(!node.name||!ts.isIdentifier(node.name)||node.typeParameters?.length||node.questionToken)throw new Error('Unsupported KSP function');
 return {name:node.name.text,parameters:node.parameters.map(p=>{
  if(!ts.isIdentifier(p.name)||p.questionToken||p.dotDotDotToken||p.initializer)throw new Error('Unsupported KSP parameter');
  return {name:p.name.text,type:type(p.type)};
 }),returnType:type(node.type),location:loc(node)};
};
const functions=[],interfaces=[];
for(const node of source.statements){
 if(!node.modifiers?.some(x=>x.kind===ts.SyntaxKind.ExportKeyword))throw new Error('Unexported KSP declaration');
 if(ts.isFunctionDeclaration(node)&&!node.body){functions.push(method(node));continue;}
 if(!ts.isInterfaceDeclaration(node)||node.typeParameters?.length||node.heritageClauses?.length)throw new Error('Unsupported KSP declaration');
 interfaces.push({name:node.name.text,members:node.members.map(m=>{
  if(ts.isMethodSignature(m))return {kind:'method',...method(m)};
  if(!ts.isPropertySignature(m)||!m.name||!ts.isIdentifier(m.name)||m.questionToken||!m.modifiers?.some(x=>x.kind===ts.SyntaxKind.ReadonlyKeyword))throw new Error('Unsupported KSP record field');
  return {kind:'property',name:m.name.text,type:type(m.type),readonly:true,location:loc(m)};
 }),location:loc(node)});
}
const equal=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
const sig=x=>({name:x.name,parameters:x.parameters.map(p=>({name:p.name,type:p.type})),returnType:x.returnType});
const expected=f=>({name:f.name,parameters:f.parameters.map(p=>({name:p.name,type:translate(p.kotlinType)})),returnType:translate(f.returnType)});
if(!equal(functions.map(sig),facts.functions.map(expected)))throw new Error('KSP facts and actual TypeScript exports disagree');
if(new Set(functions.map(f=>f.name)).size!==functions.length)throw new Error('Duplicate KSP function');
if(interfaces.length!==facts.interfaces.length+1)throw new Error('KSP interface count differs');
for(const item of facts.interfaces){const actual=interfaces.find(i=>i.name===short(item.qualifiedName));if(!actual||!equal(actual.members.map(sig).sort((a,b)=>a.name.localeCompare(b.name)),item.methods.map(expected).sort((a,b)=>a.name.localeCompare(b.name))))throw new Error('KSP callback facts and declaration disagree');}
const record=interfaces.find(i=>i.name===short(facts.record.qualifiedName));
if(!record||!equal(record.members.map(m=>({name:m.name,type:m.type,readonly:m.readonly})),facts.record.fields.map(f=>({name:f.name,type:translate(f.kotlinType),readonly:true}))))throw new Error('KSP record fields differ');
process.stdout.write(JSON.stringify({authority:'actual KSP symbols plus emitted declaration parsed by pinned TypeScript AST',typescriptVersion:ts.version,functions,interfaces})+'\n');

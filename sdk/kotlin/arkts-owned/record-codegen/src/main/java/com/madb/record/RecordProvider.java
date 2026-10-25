package com.madb.record;

import com.google.devtools.ksp.processing.*;
import com.google.devtools.ksp.symbol.*;
import java.util.*;
import java.io.*;
import java.nio.charset.StandardCharsets;

/** MADB r5 owned-runtime processor. Parses one actual annotated data declaration.
 * No Snapshot name/field spelling is embedded in the generator. */
public final class RecordProvider implements SymbolProcessorProvider {
  public SymbolProcessor create(SymbolProcessorEnvironment env) { return new Processor(env); }
  private static final class Processor implements SymbolProcessor {
    final SymbolProcessorEnvironment env; boolean done;
    Processor(SymbolProcessorEnvironment e) { env=e; }
    void fail(String message, KSNode at) { env.getLogger().error("MADB_RECORD: "+message,at); throw new IllegalArgumentException(message); }
    String name(KSDeclaration d) { return d.getQualifiedName()==null ? "" : d.getQualifiedName().asString(); }
    String identifier(String n, KSNode at) { if (!n.matches("[A-Za-z_][A-Za-z0-9_]*")) fail("simple identifier required",at); return n; }
    List<KSAnnotated> symbols(Resolver r,String annotation) { var out=new ArrayList<KSAnnotated>();r.getSymbolsWithAnnotation(annotation,false).iterator().forEachRemaining(out::add); return out; }
    String type(KSTypeReference r) { KSType t=r.resolve(); if(t.getNullability()!=Nullability.NOT_NULL) fail("nullable types are outside this record probe",r); return name(t.getDeclaration()); }
    boolean annotated(KSAnnotated node,String n) { var it=node.getAnnotations().iterator(); while(it.hasNext()) if(name(it.next().getAnnotationType().resolve().getDeclaration()).equals(n)) return true; return false; }
    void noAlias(KSAnnotated node,String n) { var it=node.getAnnotations().iterator(); while(it.hasNext()){var a=it.next();if(name(a.getAnnotationType().resolve().getDeclaration()).equals(n)) for(var arg:a.getArguments()) if(arg.getName()!=null&&arg.getName().asString().equals("name")&&!Objects.equals(arg.getValue(),"")) fail("export aliases are unsupported",node); } }
    String q(String s) { return "\""+s.replace("\\","\\\\").replace("\"","\\\"").replace("\n","\\n")+"\""; }
    String source(KSDeclaration d) {
      if(d.getContainingFile()==null || !(d.getLocation() instanceof FileLocation)) fail("export must have a real source location",d);
      return "{\"path\":"+q(d.getContainingFile().getFilePath())+",\"line\":"+((FileLocation)d.getLocation()).getLineNumber()+"}";
    }
    String functionFact(KSFunctionDeclaration f) {
      StringBuilder out=new StringBuilder("{\"name\":"+q(f.getSimpleName().asString())+",\"qualifiedName\":"+q(name(f))+",\"source\":"+source(f)+",\"parameters\":[");
      for(int i=0;i<f.getParameters().size();i++) { var p=f.getParameters().get(i);if(i>0)out.append(',');out.append("{\"name\":").append(q(p.getName().asString())).append(",\"kotlinType\":").append(q(type(p.getType()))).append('}'); }
      return out.append("],\"returnType\":").append(q(type(f.getReturnType()))).append('}').toString();
    }
    String frontendFacts(KSClassDeclaration record, List<String> names, List<String> types, List<KSClassDeclaration> interfaces, List<KSFunctionDeclaration> functions) {
      StringBuilder out=new StringBuilder("{\"schema\":\"madb.kotlin-ksp-facts/1\",\"record\":{\"qualifiedName\":"+q(name(record))+",\"source\":"+source(record)+",\"fields\":[");
      for(int i=0;i<names.size();i++){if(i>0)out.append(',');out.append("{\"name\":").append(q(names.get(i))).append(",\"kotlinType\":").append(q(types.get(i))).append('}');}
      out.append("]},\"interfaces\":[");
      for(int i=0;i<interfaces.size();i++){var node=interfaces.get(i);if(i>0)out.append(',');out.append("{\"qualifiedName\":").append(q(name(node))).append(",\"source\":").append(source(node)).append(",\"methods\":[");var methods=new ArrayList<KSFunctionDeclaration>();node.getDeclarations().iterator().forEachRemaining(d->{if(d instanceof KSFunctionDeclaration f)methods.add(f);});methods.sort(Comparator.comparing(f->f.getSimpleName().asString()));for(int j=0;j<methods.size();j++){if(j>0)out.append(',');out.append(functionFact(methods.get(j)));}out.append("]}");}
      out.append("],\"functions\":[");for(int i=0;i<functions.size();i++){if(i>0)out.append(',');out.append(functionFact(functions.get(i)));}return out.append("]}\n").toString();
    }
    void write(Dependencies dep,String path,String extension,String text) throws IOException {try(var out=env.getCodeGenerator().createNewFileByPath(dep,path,extension)){out.write(text.getBytes(StandardCharsets.UTF_8));}}
    String ts(String t,String record,Set<String> interfaces,KSNode at) {return switch(t){case "kotlin.Int"->"number";case "kotlin.String"->"string";case "kotlin.Boolean"->"boolean";case "kotlin.Unit"->"void";default->{if(t.equals(record)||interfaces.contains(t))yield t.substring(t.lastIndexOf('.')+1);fail("unsupported exported type "+t,at);yield "never";}};}
    String method(KSFunctionDeclaration f,String record,Set<String> interfaces) {
      if(f.getModifiers().contains(Modifier.SUSPEND)||!f.getTypeParameters().isEmpty()||f.getExtensionReceiver()!=null)fail("suspend/generic/extension export unsupported",f);
      StringBuilder s=new StringBuilder(identifier(f.getSimpleName().asString(),f)).append('(');boolean first=true;
      for(var p:f.getParameters()){if(p.isVararg()||p.getHasDefault())fail("default/vararg export unsupported",p);if(!first)s.append(", ");first=false;s.append(identifier(p.getName().asString(),p)).append(": ").append(ts(type(p.getType()),record,interfaces,p));}
      return s.append("): ").append(ts(type(f.getReturnType()),record,interfaces,f)).append(';').toString();
    }
    public List<KSAnnotated> process(Resolver resolver) {
      if(done)return List.of();
      var candidates=symbols(resolver,"com.madb.c07.MadbValueRecord"); if(candidates.isEmpty())return List.of();
      done=true;
      try {
        if(candidates.size()!=1)fail("exactly one record declaration required",candidates.get(0));
        if(!(candidates.get(0) instanceof KSClassDeclaration))fail("record must be a class",candidates.get(0));
        KSClassDeclaration c=(KSClassDeclaration)candidates.get(0);
        if(!c.getModifiers().contains(Modifier.DATA)||c.getClassKind()!=ClassKind.CLASS||!c.getTypeParameters().isEmpty()||c.getParentDeclaration()!=null||c.getModifiers().contains(Modifier.PRIVATE)||c.getModifiers().contains(Modifier.INTERNAL))fail("non-generic data class required",c);
        if(!annotated(c,"org.cpf.kotlin.akinterop.annotation.KNExportClass"))fail("stock KNExportClass metadata required",c);
        noAlias(c,"org.cpf.kotlin.akinterop.annotation.KNExportClass");
        String simple=identifier(c.getSimpleName().asString(),c), qualified=name(c), pkg=c.getPackageName().asString();
        var ctor=c.getPrimaryConstructor();if(ctor==null||ctor.getParameters().size()!=2)fail("exactly two primary fields required",c);
        var props=new LinkedHashMap<String,KSPropertyDeclaration>();c.getAllProperties().iterator().forEachRemaining(p->props.put(p.getSimpleName().asString(),p));
        if(props.size()!=2)fail("only the two primary properties are supported",c);
        var names=new ArrayList<String>();var types=new ArrayList<String>();
        for(var p:ctor.getParameters()) {
          String n=identifier(p.getName().asString(),p),t=type(p.getType());var property=props.get(n);
          if(!p.isVal()||p.isVar()||p.isVararg()||p.getHasDefault()||property==null||property.isMutable()||property.isDelegated()||property.getModifiers().contains(Modifier.PRIVATE)||property.getModifiers().contains(Modifier.PROTECTED)||property.getModifiers().contains(Modifier.INTERNAL))fail("immutable primary val without default required",p);
          if(!annotated(property,"org.cpf.kotlin.akinterop.annotation.KNExportField"))fail("each field needs stock KNExportField",property);
          noAlias(property,"org.cpf.kotlin.akinterop.annotation.KNExportField");
          names.add(n);types.add(t);
        }
        if(!new HashSet<>(types).equals(Set.of("kotlin.Int","kotlin.String")))fail("only one Int and one String field are supported",c);
        var declarations=c.getDeclarations().iterator();while(declarations.hasNext()){var d=declarations.next();if(d instanceof KSFunctionDeclaration f&&f.getOrigin()==Origin.KOTLIN&&!f.getSimpleName().asString().equals("<init>"))fail("explicit record methods unsupported",f);}
        var allFiles=new ArrayList<KSFile>();resolver.getAllFiles().iterator().forEachRemaining(allFiles::add);
        Dependencies dep=new Dependencies(true,allFiles.toArray(KSFile[]::new));
        StringBuilder kt=new StringBuilder("// Generated by MADB record KSP r5 from "+qualified+". Do not edit.\n@file:OptIn(kotlinx.cinterop.ExperimentalForeignApi::class)\npackage "+pkg+"\n\nimport org.cpf.kotlin.akinterop.generate.*\n\nfun installGeneratedRecordCodec() {\n    ManagedRecordCodec.install("+simple+"::class, "+q(qualified)+", listOf(\n");
        for(int i=0;i<2;i++)kt.append("        ").append(q(names.get(i))).append(" to ").append(types.get(i)).append("::class").append(i==0?",\n":"\n");
        kt.append("    ), { env, value ->\n        val obj = ManagedRecordValues.objectValue(env, value)\n");
        for(int i=0;i<2;i++)kt.append("        val field").append(i).append(" = ManagedRecordValues.").append(types.get(i).equals("kotlin.Int")?"int32":"string").append("(env, ManagedRecordValues.property(env, obj, ").append(q(names.get(i))).append("), ").append(q(names.get(i))).append(")\n");
        kt.append("        ").append(simple).append("(field0, field1)\n    }, { env, value ->\n        val record = value as ").append(simple).append("\n        val obj = ManagedRecordValues.newObject(env)\n");
        for(int i=0;i<2;i++)kt.append("        ManagedRecordValues.").append(types.get(i).equals("kotlin.Int")?"setInt32":"setString").append("(env, obj, ").append(q(names.get(i))).append(", record.`").append(names.get(i)).append("`)\n");
        kt.append("        obj\n    })\n}\n");
        write(dep,pkg.replace('.','/')+"/GeneratedRecordCodec","kt",kt.toString());
        var interfaces=new LinkedHashSet<String>();var ifaceNodes=new ArrayList<KSClassDeclaration>();
        for(var node:symbols(resolver,"org.cpf.kotlin.akinterop.annotation.KNExportInterface")){if(!(node instanceof KSClassDeclaration))fail("interface class required",node);var i=(KSClassDeclaration)node;if(i.getClassKind()!=ClassKind.INTERFACE||!i.getTypeParameters().isEmpty()||i.getParentDeclaration()!=null)fail("only top-level non-generic notification interfaces are supported",i);var members=i.getDeclarations().iterator();while(members.hasNext()){var member=members.next();if(!(member instanceof KSFunctionDeclaration))fail("notification interface members must be methods",member);var callback=(KSFunctionDeclaration)member;if(!type(callback.getReturnType()).equals("kotlin.Unit"))fail("synchronous return-valued callback is unsupported by the nonblocking notification profile",callback);}interfaces.add(name(i));ifaceNodes.add(i);}
        StringBuilder dts=new StringBuilder("// Generated from real KSP symbols by MADB record exporter r5.\nexport interface "+simple+" {\n");
        for(int i=0;i<2;i++)dts.append("  readonly ").append(names.get(i)).append(": ").append(types.get(i).equals("kotlin.Int")?"number":"string").append(";\n");dts.append("}\n");
        for(var i:ifaceNodes){dts.append("export interface ").append(i.getSimpleName().asString()).append(" {\n");var it=i.getDeclarations().iterator();while(it.hasNext()){var d=it.next();if(d instanceof KSFunctionDeclaration f)dts.append("  ").append(method(f,qualified,interfaces)).append('\n');}dts.append("}\n");}
        var functions=new ArrayList<KSFunctionDeclaration>();for(var node:symbols(resolver,"org.cpf.kotlin.akinterop.annotation.KNExportFunction")){if(!(node instanceof KSFunctionDeclaration))fail("function declaration required",node);functions.add((KSFunctionDeclaration)node);}functions.sort(Comparator.comparing(f->f.getSimpleName().asString()));
        for(var f:functions){noAlias(f,"org.cpf.kotlin.akinterop.annotation.KNExportFunction");
          int ownedCount=0; boolean sawOwned=false;
          for(int ix=0;ix<f.getParameters().size();ix++) {
            var parameter=f.getParameters().get(ix);String parameterType=type(parameter.getType());
            if(interfaces.contains(parameterType)) {
              ownedCount++;sawOwned=true;
              if(ownedCount>1||ix!=f.getParameters().size()-1)fail("at most one owned interface argument, in final position, is supported",parameter);
            } else if(sawOwned)fail("fallible conversion after owned argument is unsupported",parameter);
          }
          if(interfaces.contains(type(f.getReturnType())))fail("owned interface returns are unsupported",f);
          dts.append("export declare function ").append(method(f,qualified,interfaces)).append('\n');}
        write(dep,"record-export","d.ts",dts.toString());
        StringBuilder schema=new StringBuilder("{\n  \"generator\": \"madb-record-ksp-r5\",\n  \"qualifiedName\": "+q(qualified)+",\n  \"source\": "+q(c.getContainingFile().getFilePath())+",\n  \"location\": "+q(c.getLocation().toString())+",\n  \"fields\": [\n");
        for(int i=0;i<2;i++)schema.append("    {\"name\": ").append(q(names.get(i))).append(", \"kotlinType\": ").append(q(types.get(i))).append(", \"location\": ").append(q(props.get(names.get(i)).getLocation().toString())).append("}").append(i==0?",\n":"\n");schema.append("  ]\n}\n");
        write(dep,"record-schema","json",schema.toString());
        ifaceNodes.sort(Comparator.comparing(this::name));
        write(dep,"frontend-facts","json",frontendFacts(c,names,types,ifaceNodes,functions));
      } catch(IOException failure){env.getLogger().exception(failure);}
      return List.of();
    }
  }
}

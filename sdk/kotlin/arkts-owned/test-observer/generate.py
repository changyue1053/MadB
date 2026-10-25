"""Generate test-only NAPI trampolines from pinned imports and actual SDK AST."""
from pathlib import Path
import argparse,hashlib,json,subprocess,shutil

NEUTRAL={'napi_acquire_threadsafe_function','napi_call_threadsafe_function','napi_release_threadsafe_function','napi_call_threadsafe_function_with_priority'}
REGISTRATION={'napi_module_register'}
NOENV={'napi_remove_async_cleanup_hook'}
FIELDS=['Format','Role','WrappedApiCount','Capabilities','TotalAttempts','EnvArgumentAttempts','ThreadNeutralAttempts','RegistrationAttempts','CleanupHandleAttempts','NullEnvAttempts','UnboundEnvAttempts','WrongEnvAttempts','WrongThreadAttempts','UnboundCleanupHandleAttempts','BootstrapEntries','ProviderBindings','NullBindAttempts','EnvValidityUnobservedAttempts']
def ref(path):
 p=Path(path);return {'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'size':p.stat().st_size}
def generate(imports,expected,compiler,sysroot,output):
 imports=imports.resolve(strict=True);compiler=compiler.resolve(strict=True);sysroot=sysroot.resolve(strict=True)
 if ref(imports)['sha256']!=expected.removeprefix('sha256:'):raise ValueError('imports pin mismatch')
 data=json.loads(imports.read_text());libs=data['libraries']
 if len(libs)!=2 or sum(Path(lib.get('library',lib.get('path',''))).name=='libmadb_kotlin_owned_runtime.so' for lib in libs)!=1:raise ValueError('exact bootstrap/provider pair required')
 output.mkdir(parents=True,exist_ok=False)
 here=Path(__file__).resolve().parent
 for name in ['observer.h','observer_provider.h','observer_entry.h','observer_core.inc']:shutil.copyfile(here/name,output/name)
 query=output/'sdk-signatures.cpp';query.write_text('#include <napi/native_api.h>\n#include <node_api.h>\n')
 common=[str(compiler),'--target=aarch64-linux-ohos23.0.0','--sysroot='+str(sysroot),'-std=c++17','-x','c++']
 cmd=common+['-Xclang','-ast-dump=json','-fsyntax-only',str(query)]
 run=subprocess.run(cmd,text=True,capture_output=True)
 (output/'ast-stderr.txt').write_text(run.stderr)
 if run.returncode:raise RuntimeError(run.stderr)
 ast=json.loads(run.stdout);(output/'sdk-ast.json').write_text(run.stdout)
 declarations={}
 def visit(node):
  name=node.get('name','')
  if node.get('kind')=='FunctionDecl' and name.startswith('napi_'):
   args=[p['type']['qualType'] for p in node.get('inner',[]) if p.get('kind')=='ParmVarDecl']
   result=node['type']['qualType'].split(' (',1)[0]
   signature={'return':result,'arguments':args,'function_type':node['type']['qualType']}
   if name in declarations and declarations[name]!=signature:raise ValueError('ambiguous SDK declaration '+name)
   if node.get('variadic'):raise ValueError('variadic NAPI requires explicit handling '+name)
   declarations[name]=signature
  for child in node.get('inner',[]):visit(child)
 visit(ast)
 dependency=subprocess.run(common+['-M',str(query)],text=True,capture_output=True,check=True)
 dependencies=[Path(p) for p in dependency.stdout.replace('\\\n',' ').split()[1:] if p!='\\']
 records=[]
 for lib in libs:
  library=lib.get('library',lib.get('path'))
  role='provider' if Path(library).name=='libmadb_kotlin_owned_runtime.so' else 'bootstrap'
  names=sorted(lib.get('napi_undefined',[n for n in lib.get('undefined',[]) if n.startswith('napi_')]))
  if not names or len(names)!=len(set(names)):raise ValueError('invalid import set')
  source=['// Generated from actual SDK signatures; do not hand-edit.','#include <napi/native_api.h>','#include <node_api.h>','#include <type_traits>','#include "observer.h"',f'#define MADB_KN_OBSERVER_ROLE {1 if role=="provider" else 0}',f'#define MADB_KN_OBSERVER_API_COUNT {len(names)}','#define MADB_KN_OBSERVER_COUNTER_INITIALIZERS '+', '.join('{0}' for _ in names),'#include "observer_core.inc"']
  symbols=[]
  for index,name in enumerate(names):
   sig=declarations.get(name)
   if not sig:raise ValueError('import has no exact SDK declaration: '+name)
   args=sig['arguments']
   env_positions=[i for i,a in enumerate(args) if a=='napi_env']
   if name in NEUTRAL:category=1
   elif name in REGISTRATION:category=2
   elif name in NOENV:category=3
   elif env_positions==[0]:category=0
   else:raise ValueError('unclassified NAPI import '+name)
   if category and env_positions:raise ValueError('non-env category has env argument '+name)
   aliases=[f'MadbArg_{name}_{i}' for i in range(len(args))]
   source.extend(f'using {alias} = {arg};' for alias,arg in zip(aliases,args))
   parameters=', '.join(f'{alias} arg{i}' for i,alias in enumerate(aliases)) or 'void'
   forwarded=', '.join(f'arg{i}' for i in range(len(args)))
   result=sig['return'];env='reinterpret_cast<void *>(arg0)' if category==0 else 'nullptr'
   source.extend([f'extern "C" decltype({name}) __real_{name};',f'extern "C" __attribute__((visibility("hidden"))) {result} __wrap_{name}({parameters}) {{',f'    recordAttempt({index}, {category}, {env});',f'    {"return " if result!="void" else ""}__real_{name}({forwarded});','}',f'static_assert(std::is_same<decltype(&__wrap_{name}), decltype(&{name})>::value, "SDK signature mismatch: {name}");'])
   symbols.append({'index':18+index,'name':name,'category':category,**sig})
  cpp=output/(role+'_observer.cpp');cpp.write_text('\n'.join(source)+'\n')
  flags=output/(role+'-wrap.rsp');flags.write_text('\n'.join('--wrap='+n for n in names)+'\n')
  records.append({'role':role,'input_library':library,'input_sha256':lib['sha256'],'symbols':symbols,'source':ref(cpp),'linker_response':ref(flags),'clang_flags':['-Wl,--wrap='+n for n in names]})
 schema={'scope':'Test-only static ELF import call-attempt observation; no proof of env invalidity or transitive library internals. Zero attempts means unexercised, not a passed illegal-call predicate.','wire':'testNapiAudit String = bootstrap decimal CSV + semicolon + provider decimal CSV; provider unavailable until existing library and pure metric symbol are present. No extra dlopen.','format':70602,'fixed_fields':[{'index':i,'name':n} for i,n in enumerate(FIELDS)],'unknown':'UINT64_MAX (18446744073709551615) means unavailable/not applicable, never zero; bootstrap WrongThreadAttempts is NA. Preserve all CSV fields as decimal strings.','capabilities':{'1':'bootstrap legitimate callback entry env scope','2':'provider bound env comparison','4':'provider owner thread comparison'},'categories':{'0':'API with napi_env argument','1':'thread-neutral TSFN acquire/call/release including priority call','2':'module registration with no env','3':'async cleanup handle removal; no env argument, provider owner-thread still checked'},'validity':'EnvValidityUnobservedAttempts increments for every env-argument call. No invalidation time is inferred from logical close or cleanup-hook entry. Legal ref/TSFN/hook cleanup remains legal.','snapshot':'Read all pure metric fields before constructing the JS String. Each scalar is atomic; cross-field reads are not an atomic concurrent snapshot. Readback NAPI construction is itself recorded in subsequent snapshots.','lifetime':'Counters are DSO-lifetime cumulative; no reset API. Bind/entry functions never invoke NAPI or allocate. Observer RAII is required at each actual bootstrap callback entry.','roles':[{ 'role':r['role'],'count':18+len(r['symbols']),'per_api_fields':r['symbols']} for r in records]}
 (output/'observer-schema.json').write_text(json.dumps(schema,indent=2)+'\n')
 manifest={'imports':ref(imports),'compiler':ref(compiler),'headers':[ref(p) for p in sorted(set(dependencies)) if p!=query],'ast_command':cmd,'ast_exit_code':run.returncode,'ast':ref(output/'sdk-ast.json'),'query':ref(query),'generator':ref(Path(__file__)),'roles':records,'support_files':[ref(output/n) for n in ['observer.h','observer_provider.h','observer_entry.h','observer_core.inc']],'schema':ref(output/'observer-schema.json')}
 (output/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
 return ref(output/'manifest.json')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--imports',type=Path,required=True);p.add_argument('--imports-sha256',required=True);p.add_argument('--compiler',type=Path,required=True);p.add_argument('--sysroot',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();print(json.dumps(generate(a.imports,a.imports_sha256,a.compiler,a.sysroot,a.output),indent=2))

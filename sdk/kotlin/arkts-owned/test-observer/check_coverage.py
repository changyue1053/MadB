"""Fail closed when the final test ELF import set is not exactly wrapped."""
from pathlib import Path
import argparse,json,hashlib,re,subprocess

def ref(path):return {'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'size':path.stat().st_size}
def symbols(readelf,library,flag):
 p=subprocess.run([str(readelf),flag,'--wide',str(library)],capture_output=True,text=True,check=True)
 rows=[]
 for line in p.stdout.splitlines():
  match=re.match(r'\s*\d+:\s+\S+\s+\S+\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)',line)
  if match:rows.append({'type':match[1],'bind':match[2],'visibility':match[3],'index':match[4],'name':match[5].split('@',1)[0]})
 return rows,p.stdout

def check(manifest_path,readelf,libraries,output,ordinary=()):
 manifest=json.loads(manifest_path.read_text());records=[];errors=[]
 for pinned in [*manifest['support_files'],manifest['schema']]:
  if ref(Path(pinned['path']))['sha256']!=pinned['sha256']:errors.append('generated support/schema drift: '+pinned['path'])
 for role in manifest['roles']:
  label=role['role'];library=libraries[label].resolve(strict=True)
  # The observed sources/flags are the ones pinned by the generator manifest.
  for field in ['source','linker_response']:
   pinned=role[field];actual=ref(Path(pinned['path']))
   if actual['sha256']!=pinned['sha256']:errors.append(label+' '+field+' drift')
  expected={s['name'] for s in role['symbols']}
  flags=Path(role['linker_response']['path']).read_text().splitlines()
  if set(flags)!={'--wrap='+n for n in expected} or len(flags)!=len(expected):errors.append(label+' wrap response mismatch')
  dyn,_=symbols(readelf,library,'--dyn-syms');full,_=symbols(readelf,library,'--symbols')
  actual={r['name'] for r in dyn if r['index']=='UND' and r['name'].startswith('napi_')}
  defined={r['name'] for r in full if r['index']!='UND' and r['type']=='FUNC'}
  wrapped={n.removeprefix('__wrap_') for n in defined if n.startswith('__wrap_napi_')}
  unresolved={r['name'] for r in dyn if r['index']=='UND' and r['name'].startswith(('__wrap_napi_','__real_napi_'))}
  missing=sorted(actual-expected);stale=sorted(expected-actual);missing_wrappers=sorted(expected-wrapped);extra_wrappers=sorted(wrapped-expected)
  if missing or stale or missing_wrappers or extra_wrappers or unresolved:errors.append(label+' ELF coverage mismatch')
  metric='madb_kn_'+label+'_napi_metric'
  exported=any(r['name']==metric and r['index']!='UND' and r['visibility']=='DEFAULT' for r in dyn)
  if not exported:errors.append(label+' pure metric export missing')
  wrappers_hidden=all(r['visibility']=='HIDDEN' or r['bind']=='LOCAL' for r in full if r['name'].startswith('__wrap_napi_'))
  if not wrappers_hidden:errors.append(label+' wrapper symbol preemptible')
  records.append({'role':label,'library':ref(library),'actual_napi_imports':sorted(actual),'defined_wrappers':sorted(wrapped),'unexpected_imports':missing,'stale_imports':stale,'missing_wrappers':missing_wrappers,'extra_wrappers':extra_wrappers,'unresolved_trampolines':sorted(unresolved),'metric_exported':exported,'wrappers_hidden_or_local':wrappers_hidden})
 ordinary_records=[]
 for library in ordinary:
  full,_=symbols(readelf,library,'--symbols');bad=sorted({r['name'] for r in full if r['name'].startswith(('__wrap_napi_','__real_napi_')) or re.match(r'madb_kn_(provider|bootstrap)_napi_',r['name'])})
  if bad:errors.append('ordinary contains observer symbols: '+str(library))
  ordinary_records.append({'library':ref(library),'observer_symbols':bad})
 receipt={'scope':'Exact final-ELF static NAPI import/trampoline coverage; does not prove APIs executed on target or env invalidity. Same-signature generated wrappers record immediately before __real calls.','generator_manifest':ref(manifest_path),'readelf':ref(readelf),'test_libraries':records,'ordinary_libraries':ordinary_records,'errors':errors,'passed':not errors}
 output.write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({'receipt':ref(output),'passed':not errors,'errors':errors}))
 if errors:raise RuntimeError('; '.join(errors))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--manifest',type=Path,required=True);p.add_argument('--readelf',type=Path,required=True);p.add_argument('--bootstrap',type=Path,required=True);p.add_argument('--provider',type=Path,required=True);p.add_argument('--ordinary',type=Path,action='append',default=[]);p.add_argument('--output',type=Path,required=True);a=p.parse_args();check(a.manifest.resolve(strict=True),a.readelf.absolute(),{'bootstrap':a.bootstrap,'provider':a.provider},a.output,a.ordinary)

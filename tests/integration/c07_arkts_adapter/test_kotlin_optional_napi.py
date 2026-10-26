"""Real host loader and generated call interception, with explicit NAPI ABI fixtures."""
from pathlib import Path
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
OBSERVER = ROOT / 'sdk/kotlin/arkts-owned/test-observer'
FIXTURE = Path(__file__).parent / 'optional_napi'
NAMES = ['napi_create_callsite_info', 'napi_delete_callsite_info',
         'napi_get_property_with_callsite_info', 'napi_set_property_with_callsite_info']

class KotlinOptionalNapi(unittest.TestCase):
    def test_missing_partial_present_and_mutants(self):
        compiler = shutil.which('clang++')
        self.assertIsNotNone(compiler)
        receipt = json.loads((OBSERVER/'source-receipt.json').read_text())
        original = (FIXTURE/'property.cpp').read_text()
        self.assertEqual(hashlib.sha256(original.encode()).hexdigest(), receipt['call_shim']['base_sha256'])
        schema = json.loads((OBSERVER/'observer-schema.json').read_text())
        fields = next(x['per_api_fields'] for x in schema['roles'] if x['role']=='provider')
        positions = [next(x['index'] for x in fields if x['name']==n) for n in NAMES]
        generated = (OBSERVER/'provider_observer.cpp').read_text()
        chunks = []
        for name in NAMES:
            match = re.search(r'using MadbArg_'+name+r'_0 = .*?SDK signature mismatch: '+name+r'"\);', generated, re.S)
            self.assertIsNotNone(match)
            chunks.append(match[0])
        with tempfile.TemporaryDirectory(prefix='madb-kn-optional-') as tmp:
            output = Path(tmp)
            cases = [('fixed', mask, True) for mask in (0,14,13,11,7,15)]
            cases += [('ordinary',0,True),('ordinary',15,True),('weak-ldwrap',0,False),('object-like',0,False),('strong-delegate',0,False)]
            results = []
            for mode, mask, success in cases:
                folder = output/(mode+'-'+str(mask));folder.mkdir()
                shim = (OBSERVER/'provider-call-shim.h').read_text()
                if mode=='object-like':
                    for name in NAMES:shim=shim.replace('#define '+name+'(...) __wrap_'+name+'(__VA_ARGS__)', '#define '+name+' __wrap_'+name)
                (folder/'madb_napi_call_shim.h').write_text(shim)
                source = original
                if mode not in ('ordinary','weak-ldwrap'):
                    source=source.replace(receipt['call_shim']['insert_before'], '\n#include "madb_napi_call_shim.h"\n'+receipt['call_shim']['insert_before'])
                (folder/'property.cpp').write_text(source)
                wrappers='\n'.join(chunks)
                if mode=='strong-delegate':wrappers=wrappers.replace(' __attribute__((weak))','')
                if mode=='weak-ldwrap':
                    for name in NAMES:
                        wrappers=wrappers.replace('decltype('+name+') '+name+';', 'decltype('+name+') __real_'+name+';').replace('return '+name+'(', 'return __real_'+name+'(')
                if mode=='ordinary':wrappers=''
                (folder/'wrappers.cpp').write_text('#include "property.h"\n#include <type_traits>\nstatic unsigned counts[101]{};\nstatic void recordAttempt(int n,int,void*) { ++counts[n+18]; }\nextern "C" unsigned metric(int n) { return counts[n]; }\n'+wrappers)
                declarations=[]
                bodies=[
                    'if(env!=E||!out)bad=1;*out=I;return status;',
                    'if(env!=E||info!=I)bad=1;return status;',
                    'if(env!=E||obj!=O||key!=K||info!=I||!out||hit)bad=1;*out=OPT;return status;',
                    'if(env!=E||obj!=O||key!=K||value!=V||info!=I||hit)bad=1;return status;'
                ]
                params=['napi_env env,napi_callsite_info* out','napi_env env,napi_callsite_info info','napi_env env,napi_value obj,napi_value key,napi_callsite_info info,napi_value* out,bool* hit','napi_env env,napi_value obj,napi_value key,napi_value value,napi_callsite_info info,bool* hit']
                for i,name in enumerate(NAMES):
                    if not mask & (1<<i):continue
                    delegate='__real_'+name if mode=='weak-ldwrap' else name
                    check='' if mode=='ordinary' else f'if(metric({positions[i]})!=calls[{i}]+1)bad=1;'
                    declarations.append(f'extern "C" napi_status {delegate}({params[i]}) {{ {check} ++calls[{i}]; {bodies[i]} }}')
                test='''#include "property.h"
#include <cstdio>
extern "C" unsigned metric(int);
static void *E=(void*)1,*O=(void*)2,*K=(void*)3,*V=(void*)4,*I=(void*)5,*OPT=(void*)6,*FALL=(void*)7;
static unsigned calls[4]{},fg=0,fs=0,bad=0;static napi_status status=napi_ok;
extern "C" napi_status napi_get_property_names(napi_env,napi_value,napi_value*) { return napi_ok; }
extern "C" napi_status napi_get_named_property(napi_env,napi_value,const char*,napi_value*) { return napi_ok; }
extern "C" napi_status napi_set_named_property(napi_env,napi_value,const char*,napi_value) { return napi_ok; }
extern "C" napi_status napi_get_property(napi_env env,napi_value obj,napi_value key,napi_value*out) {if(env!=E||obj!=O||key!=K||!out)bad=1;++fg;*out=FALL;return napi_ok;}
extern "C" napi_status napi_set_property(napi_env env,napi_value obj,napi_value key,napi_value value) {if(env!=E||obj!=O||key!=K||value!=V)bad=1;++fs;return napi_ok;}
'''+ '\n'.join(declarations)+f'''
extern "C" int exercise() {{
  if(hasCallsiteInfoSupport()!=({mask}==15))return 11;
  if(hasCallsiteInfoSupport()){{auto info=createCallsiteInfo(E);if(info!=I)return 12;deleteCallsiteInfo(E,info);}}
  if(getPropertyWithCallsiteInfo(E,O,K,I)!=(({mask}&4)?OPT:FALL))return 13;
  setPropertyWithCallsiteInfo(E,O,K,V,I);
  if(fg!=(({mask}&4)?0:1)||fs!=(({mask}&8)?0:1)||bad)return 14;
  unsigned expected[4]={{ {int(mask==15)}, {int(mask==15)}, {int(bool(mask&4))}, {int(bool(mask&8))} }};
  for(int i=0;i<4;++i)if(calls[i]!=expected[i])return 15;
'''
                if mode!='ordinary':
                    for i,index in enumerate(positions):test+=f'  if(metric({index})!=expected[{i}])return 16;\n'
                if mode!='ordinary' and mask==15:
                    declarations='\n'.join('extern "C" decltype('+n+') __wrap_'+n+';' for n in NAMES)
                    test=test.replace('extern "C" int exercise()', declarations+'\nextern "C" int exercise()')
                    args=['E,&copy','E,I','E,O,K,I,&copy,nullptr','E,O,K,V,I,nullptr']
                    test+='  status=napi_generic_failure; napi_value copy=nullptr;\n'
                    for i,(name,args_) in enumerate(zip(NAMES,args)):
                        test+=f'  if(__wrap_{name}({args_})!=napi_generic_failure)return 17;\n'
                        test+=f'  if(calls[{i}]!=2||metric({positions[i]})!=2||bad)return 18;\n'
                test+='  return 0;\n}\n'
                (folder/'fixture.cpp').write_text(test)
                lib=folder/('probe.dylib' if sys.platform=='darwin' else 'probe.so')
                cmd=[compiler,'-std=c++17','-O1','-fPIC','-I'+str(FIXTURE/'include'),'-I'+str(folder),str(folder/'property.cpp'),str(folder/'wrappers.cpp'),str(folder/'fixture.cpp'),'-o',str(lib)]
                cmd+=['-dynamiclib','-Wl,-undefined,dynamic_lookup'] if sys.platform=='darwin' else ['-shared']
                if mode=='weak-ldwrap':
                    cmd += (['-Wl,-alias,___wrap_'+n+',_'+n for n in NAMES] if sys.platform=='darwin' else ['-Wl,--wrap='+n for n in NAMES])
                build=subprocess.run(cmd,text=True,capture_output=True,timeout=30)
                self.assertEqual(build.returncode,0,build.stderr)
                run=subprocess.run([sys.executable,'-c','import ctypes,sys\ntry: lib=ctypes.CDLL(sys.argv[1], mode=2)\nexcept OSError as e: print("LOAD_FAILURE",str(e));sys.exit(30)\nr=lib.exercise();print("EXERCISE",r);sys.exit(r)',str(lib)],text=True,capture_output=True,timeout=10)
                results.append((mode,mask,run.returncode,run.stdout.strip()))
                if success:self.assertEqual(run.returncode,0,repr(results))
                elif mode=='strong-delegate':
                    self.assertEqual(run.returncode,30,repr(results))
                    self.assertIn('LOAD_FAILURE',run.stdout)
                    self.assertTrue(any(name in run.stdout for name in NAMES),run.stdout)
                else:self.assertEqual(run.returncode,11,repr(results))
            print(json.dumps(results))

if __name__=='__main__':unittest.main()

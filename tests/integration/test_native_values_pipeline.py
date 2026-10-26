"""Finite values identity, lowering and old-reader preservation; no synthetic SDK certification."""
from copy import deepcopy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT))
sys.path.insert(0,str(ROOT/'adapters/generators/rust_c'))
from native_ast import abi_type, _value_record_layouts, read_header
from support import NativeError
from values_lowering import BASE_CONSTANTS, interface_identity, lower, validate_surface
import values_driver


def example():
    loc=dict(path='author.rs',line=1,column=1)
    plan=dict(namespace='other_business',records=[dict(name='Packet',fields=[dict(name='count',kind='i32',location=loc),dict(name='text',kind='String',location=loc)],location=loc)],
              errors=[dict(name='Fault',variants=[dict(name='Limit',value=7,location=loc)],location=loc)],
              functions=[dict(name='transform',record='Packet',parameters=[dict(name='packet',kind='Packet'),dict(name='change',kind='i32')],error='Fault',location=loc)],witness='')
    surface=dict(functions={
        'other_business_bootstrap_v1':dict(result='i32',parameters=[dict(name='version',type='u32'),dict(name='out_identity',type='u8 *'),dict(name='capacity',type='u64')],location=loc),
        'other_business_raw_transform':dict(result='i32',parameters=[dict(name='input',type='const other_business_Packet *'),dict(name='change',type='i32'),dict(name='out_value',type='other_business_Packet *'),dict(name='out_error',type='MadbValueError *')],location=loc)},
        records={'MadbValueError':[dict(name='domain',type='u32'),dict(name='code',type='i32')],
                 'MadbValueUtf8_64':[dict(name='byte_count',type='u32'),dict(name='bytes',type='u8[64]')],
                 'other_business_Packet':[dict(name='count',type='i32'),dict(name='text',type='MadbValueUtf8_64')]},
        aliases={'MadbValueStatus':'i32','MadbValueCallKind':'u32','MadbValueError':'MadbValueError','MadbValueUtf8_64':'MadbValueUtf8_64','other_business_Packet':'other_business_Packet'},
        layouts={'MadbValueError':dict(size=8,alignment=4,fields=[dict(name='domain',offset=0,size=4),dict(name='code',offset=4,size=4)]),
                 'MadbValueUtf8_64':dict(size=68,alignment=4,fields=[dict(name='byte_count',offset=0,size=4),dict(name='bytes',offset=4,size=64)]),
                 'other_business_Packet':dict(size=72,alignment=4,fields=[dict(name='count',offset=0,size=4),dict(name='text',offset=4,size=68)])},
        constants={**BASE_CONSTANTS,'other_business_Fault_Limit':7})
    return plan,surface,dict(provider='rust',semantics='madb.native-values/1',source='author.rs')


class ValuePipelineTests(unittest.TestCase):
    def test_new_names_are_structural_not_sample_whitelist(self):
        plan,surface,config=example();validate_surface(surface,plan,config)
        request=dict(boundary_id='independent',selector={},inputs=[dict(path='author.rs',sha256='sha256:'+'1'*64,size=12)])
        seed=lower(surface,plan,config,request,dict(sdk_namespace='other_business',project='custom'))
        shape=next(s['shape'] for s in seed['symbols'] if s['shape']['link_name']=='other_business_raw_transform')
        self.assertEqual(shape['throws'],'Fault');self.assertEqual(shape['parameters'][0],dict(name='packet',type_id='Packet'))
        self.assertEqual(next(t['definition'] for t in seed['types'] if t['id']=='madb.recipe.String'),dict(kind='string',encoding='utf-8',max_bytes=64))
        self.assertEqual(seed['observations'],[])

    def test_author_bootstrap_does_not_collide_with_recipe_source_identity(self):
        plan,surface,config=example()
        plan['functions'][0]['name']='bootstrap'
        surface['functions']['other_business_raw_bootstrap']=surface['functions'].pop('other_business_raw_transform')
        seed=lower(surface,plan,config,dict(boundary_id='x',selector={},inputs=[]),dict(sdk_namespace='other_business',project='custom'))
        self.assertEqual({s['source_key']['qualified_name'] for s in seed['symbols']},{'bootstrap','$recipe.bootstrap.v1'})
        self.assertEqual(len({s['public_id'] for s in seed['symbols']}),2)

    def test_natural_type_names_do_not_collide_with_recipe_types(self):
        for name in ('unit','identity_bytes','u8','i32'):
            plan,surface,config=example();plan['records'][0]['name']=name
            plan['functions'][0]['record']=name;plan['functions'][0]['parameters'][0]['kind']=name
            old='other_business_Packet';new='other_business_'+name
            for key in ('records','layouts'):surface[key][new]=surface[key].pop(old)
            surface['aliases'].pop(old);surface['aliases'][new]=new
            for parameter in surface['functions']['other_business_raw_transform']['parameters']:
                parameter['type']=parameter['type'].replace(old,new)
            seed=lower(surface,plan,config,dict(boundary_id='x',selector={},inputs=[]),dict(sdk_namespace='other_business',project='custom'))
            self.assertEqual(len({t['id'] for t in seed['types']}),len(seed['types']))
            self.assertIn(name,{t['id'] for t in seed['types']})

    def test_exact_physical_drift_is_rejected(self):
        plan,surface,config=example()
        mutations=[lambda s:s['constants'].__setitem__('other_business_Fault_Limit',8),
                   lambda s:s['constants'].__setitem__('other_business_hidden',1),
                   lambda s:s['records']['other_business_Packet'][0].__setitem__('type','u32'),
                   lambda s:s['layouts']['other_business_Packet'].__setitem__('size',76),
                   lambda s:s['functions']['other_business_raw_transform']['parameters'][1].__setitem__('name','delta'),
                   lambda s:s['aliases'].__setitem__('Unexpected','u32')]
        for mutate in mutations:
            changed=deepcopy(surface);mutate(changed)
            with self.subTest(mutate=mutate),self.assertRaises(NativeError):validate_surface(changed,plan,config)

    def test_error_tag_changes_interface_digest_and_location_does_not(self):
        plan,surface,config=example();identity=interface_identity(surface,plan,config,'sha256:'+'2'*64)
        altered=deepcopy(surface);altered['functions']['other_business_raw_transform']['location']['line']=99
        self.assertEqual(identity,interface_identity(altered,plan,config,'sha256:'+'2'*64))
        changed_plan=deepcopy(plan);changed_plan['errors'][0]['variants'][0]['value']=8;altered['constants']['other_business_Fault_Limit']=8
        self.assertNotEqual(identity['interface_digest'],interface_identity(altered,changed_plan,config,'sha256:'+'2'*64)['interface_digest'])

    def test_old_reader_still_rejects_array(self):
        with self.assertRaises(NativeError):abi_type('uint8_t[64]',{})
        self.assertEqual(abi_type('uint8_t[64]',{},value_abi=True),'u8[64]')
        for kind in ('uint8_t[65]','uint32_t[64]'):
            with self.assertRaises(NativeError):abi_type(kind,{},value_abi=True)

    def test_nested_layout_uses_direct_field_not_child_same_name(self):
        text='''*** Dumping AST Record Layout
         0 | struct Example
         0 |   uint32_t count
         4 |   struct Inner text
         4 |     uint32_t count
         8 |     uint8_t[64] bytes
           | [sizeof=72, align=4]
'''
        actual=_value_record_layouts(text,{'Example':[dict(name='count',type='u32'),dict(name='text',type='Inner')]},{'Example':'Example'},{'Inner':68})
        self.assertEqual(actual['Example']['fields'],[dict(name='count',offset=0,size=4),dict(name='text',offset=4,size=68)])

    def test_extra_nonprefixed_macro_rejected_even_if_clang_accepts(self):
        with tempfile.TemporaryDirectory() as d:
            header=Path(d)/'value.h';header.write_text('#ifndef VALUE_H\n#define VALUE_H\n#define foreign_extra 3\n#endif\n')
            with patch('native_ast.run_tool',side_effect=[b'{"inner":[]}',b'#define foreign_extra 3\n']):
                with self.assertRaisesRegex(NativeError,'unexpected value ABI macro'):
                    read_header(header,dict(clang_arguments=[]),value_abi=True,constant_names=set())

    def test_cli_export_is_transactional_after_final_checks(self):
        # Exercise actual main/export control flow and file writes; native work
        # is stubbed here and is covered separately by LiveValuePipelineTests.
        from contextlib import ExitStack, redirect_stdout
        import importlib.util
        import io
        spec=importlib.util.spec_from_file_location('value_export_test',ROOT/'scripts/build_native_values.py')
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
        real_mkdtemp=tempfile.mkdtemp;real_copytree=shutil.copytree
        cases=[('success','candidate'),('success','madb-host'),('source-after-package','audit'),
               ('source-after-copy','audit'),('clean-pre-export','audit'),('clean-after-copy','audit'),
               ('copy-failure','audit'),('destination-arrives','audit'),('destination-symlink-arrives','audit')]
        for mode,variant in cases:
            with self.subTest(mode=mode,variant=variant),tempfile.TemporaryDirectory(prefix='value-export-test-') as temporary:
                case=Path(temporary);repo=case/'repo';(repo/'target/debug').mkdir(parents=True)
                (repo/'target/debug/madb').write_bytes(b'upstream work is stubbed')
                author=case/'author.rs';author.write_text('author bytes');output=case/'output';checks=[];packaged=[]
                def require_revision(revision):
                    checks.append(revision)
                    if (mode=='clean-pre-export' and len(checks)==3) or (mode=='clean-after-copy' and len(checks)==4):
                        raise RuntimeError('changed producer')
                def prepare_fixture(repo,root,observation,source,*args):
                    root.mkdir();shutil.copyfile(source,root/'author.rs');return {}
                class UpstreamFixture:
                    def __init__(self,*args):pass
                    def metadata(self):pass
                    def generate(self):pass
                    def build(self):pass
                    def package(self,destination,*args,**kwargs):
                        destination.mkdir();(destination/'candidate.json').write_text('fixture');packaged.append(destination)
                        if mode=='source-after-package':author.write_text('changed author')
                        return {'candidate_digest':'sha256:'+'1'*64}
                def copytree(source,destination,*args,**kwargs):
                    if mode=='copy-failure':
                        Path(destination).mkdir();(Path(destination)/'partial').write_text('partial');raise OSError('copy failed')
                    result=real_copytree(source,destination,*args,**kwargs)
                    if mode=='source-after-copy':author.write_text('changed author')
                    if mode=='destination-arrives':output.mkdir();(output/'owned-by-other').write_text('preserve')
                    if mode=='destination-symlink-arrives':output.symlink_to('missing-target')
                    return result
                def mkdtemp(*args,**kwargs):
                    if len(args)<3:kwargs.setdefault('dir',str(case))
                    return real_mkdtemp(*args,**kwargs)
                argv=['build_native_values.py','--source',str(author),'--namespace','audit','--variant',variant,
                      '--required-case','fixture.export','--output',str(output),'--bindgen','unused','--cbindgen','unused']
                with ExitStack() as stack:
                    patches=[(module,'ROOT',repo),(module,'revision',lambda:'a'*40),(module,'require_revision',require_revision),
                        (module,'verify_sources',lambda*a:None),(module,'discover',lambda*a:{}),(module,'prepare',prepare_fixture),
                        (module,'Pipeline',UpstreamFixture),(module.subprocess,'run',lambda*a,**k:None),
                        (module.tempfile,'mkdtemp',mkdtemp),(module.shutil,'copytree',copytree),(sys,'argv',argv)]
                    for obj,key,value in patches:stack.enter_context(patch.object(obj,key,value))
                    with redirect_stdout(io.StringIO()):
                        if mode=='success':module.main()
                        else:
                            with self.assertRaises((RuntimeError,OSError)):module.main()
                self.assertTrue(packaged[0].exists())
                if mode=='success':self.assertEqual({p.name for p in output.iterdir()},{'candidate.json'})
                elif mode=='destination-arrives':self.assertEqual({p.name for p in output.iterdir()},{'owned-by-other'})
                elif mode=='destination-symlink-arrives':self.assertEqual(output.readlink(),Path('missing-target'))
                else:self.assertFalse(output.exists())

    def test_old_semantics_are_not_accepted_by_new_lowerer(self):
        plan,surface,config=example();config['semantics']='madb.first-route/1'
        with self.assertRaises(NativeError):validate_surface(surface,plan,config)


# Explicit opt-in: these are real C04/native transactions, not the default unit suite.
# All host/compiler images must already be built and locked by the coordinator.
import os
import shutil
import subprocess
from adapters.toolchains.rust_c.common import save, ref, project_digest, profile_digest, sha_file
from adapters.toolchains.rust_c.values_project import prepare
from adapters.toolchains.rust_c.values_pipeline import Pipeline
from adapters.toolchains.rust_c.tools import discover


def assert_responsibility_package_rejection(test, pipeline, destination, required):
    current=(pipeline.root/'domain/current.json').read_bytes()
    committed=pipeline.layer('candidate')
    existing={p.relative_to(committed).as_posix():sha_file(p) for p in committed.rglob('*') if p.is_file()}
    selected=pipeline.public_generated
    pipeline.public_generated=lambda:[item for item in selected() if item['path']!='responsibilities.md']
    revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    try:
        with test.assertRaises(RuntimeError):pipeline.package(destination,revision,required,provisional=True)
        test.assertFalse(destination.exists())
        test.assertEqual((pipeline.root/'domain/current.json').read_bytes(),current)
        test.assertEqual({p.relative_to(committed).as_posix():sha_file(p) for p in committed.rglob('*') if p.is_file()},existing)
    finally:pipeline.public_generated=selected


@unittest.skipUnless(os.environ.get('MADB_VALUES_LIVE_HOST'), 'requires explicit locked native host/reader')
class LiveValuePipelineTests(unittest.TestCase):
    def test_author_bootstrap_name_passes_actual_witness_and_generation(self):
        workspace=Path(os.environ['MADB_VALUES_LIVE_WORKSPACE']+'-bootstrap').absolute()
        workspace.mkdir(parents=True,exist_ok=False)
        author=workspace/'author-input.rs'
        author.write_text('pub struct Reading { pub value: u32, pub label: String }\npub fn bootstrap(value: Reading) -> Reading { value }\npub struct unit { pub value: u32 }\npub fn identity_unit(value: unit) -> unit { value }\npub struct identity_bytes { pub value: u32 }\npub fn identity_record(value: identity_bytes) -> identity_bytes { value }\npub struct u8 { pub value: u32 }\npub fn identity_u8(value: u8) -> u8 { value }\npub struct i32 { pub value: u32 }\npub fn identity_i32(value: i32) -> i32 { value }\n')
        observation=discover(ROOT,Path(os.environ['MADB_BINDGEN']),Path(os.environ['MADB_CBINDGEN']))
        prepared=prepare(ROOT,workspace/'managed',observation,author,'reading_demo','named-bootstrap',Path(os.environ['MADB_VALUES_LIVE_READER']))
        pipeline=Pipeline(ROOT,workspace/'managed',prepared,Path(os.environ['MADB_VALUES_LIVE_HOST']))
        pipeline.metadata();pipeline.generate()
        self.assertEqual({s['source_key']['qualified_name'] for s in pipeline.contract['symbols']},{'bootstrap','$recipe.bootstrap.v1','identity_unit','identity_record','identity_u8','identity_i32'})
        native=(pipeline.source/'include/reading_demo_native.h').read_text()
        self.assertIn('reading_demo_bootstrap_v1(',native);self.assertIn('reading_demo_raw_bootstrap(',native)
        invalid=workspace/'shadowed-field.rs'
        invalid.write_text('pub struct i32 { pub value: u32 }\npub struct Reading { pub value: i32 }\npub fn same(value: Reading) -> Reading { value }\n')
        with self.assertRaisesRegex(NativeError,'rustc exited'):
            prepare(ROOT,workspace/'invalid-field',observation,invalid,'reading_demo','invalid-field',Path(os.environ['MADB_VALUES_LIVE_READER']))

    def test_same_root_regeneration_and_failure_preservation(self):
        host=Path(os.environ['MADB_VALUES_LIVE_HOST']).resolve(strict=True)
        reader=Path(os.environ['MADB_VALUES_LIVE_READER']).resolve(strict=True)
        fixtures=Path(os.environ['MADB_VALUES_FIXTURES']).resolve(strict=True)
        workspace=Path(os.environ['MADB_VALUES_LIVE_WORKSPACE']).absolute()
        workspace.mkdir(parents=True,exist_ok=False)
        observation=discover(ROOT,Path(os.environ['MADB_BINDGEN']),Path(os.environ['MADB_CBINDGEN']))
        managed=workspace/'managed';managed.mkdir();results=[];fixed_selector=None
        for number in range(5):
            variant='author-'+str(number)
            stage=workspace/('prepared-'+variant)
            prepared=prepare(ROOT,stage,observation,fixtures/(variant+'.rs'),'reading_demo','same-root-values',reader)
            selector=prepared['read']['selector']
            stable_selector={k:v for k,v in selector.items() if k!='profile_digest'}
            if fixed_selector is None:fixed_selector=stable_selector
            else:self.assertEqual(stable_selector,fixed_selector)
            self.assertEqual(selector['profile_digest'],profile_digest(prepared['profile']))
            expected_features=['core.exact-numbers@1','core.values@1']
            if number in (1,2,3):expected_features+=['core.errors@1','core.variants@1']
            self.assertEqual(prepared['profile']['required_features'],sorted(expected_features))
            self.assertEqual(prepared['profile']['supported_features'],sorted(expected_features))
            self.assertEqual(prepared['project']['boundaries'][0]['id'],'same-root-values')
            self.assertEqual(prepared['project']['sdk_namespace'],'reading_demo')
            # Update only recipe input files. The existing domain/Candidate and
            # transaction owner are retained across every declaration change.
            shutil.copytree(stage,managed,dirs_exist_ok=True)
            pipeline=Pipeline(ROOT,managed,prepared,host)
            pipeline.metadata();pipeline.generate();pipeline.build()
            required=['l02.native-values.'+variant+'.'+case for case in ('values','raw-rejections','bootstrap','exports','helper-protocol')]
            if number==2:required.append('l02.native-values.author-2.mixed-candidates')
            if number==4:required.append('l02.native-values.author-4.removed-api')
            candidate=workspace/'candidates'/variant;candidate.parent.mkdir(exist_ok=True)
            revision=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
            manifest=pipeline.package(candidate,revision,required,provisional=True)
            current_before=(managed/'domain/current.json').read_bytes()
            candidate_before={p.relative_to(candidate).as_posix():sha_file(p) for p in candidate.rglob('*') if p.is_file()}
            def unchanged():
                self.assertEqual((managed/'domain/current.json').read_bytes(),current_before)
                self.assertEqual({p.relative_to(candidate).as_posix():sha_file(p) for p in candidate.rglob('*') if p.is_file()},candidate_before)
            negatives=[]
            if number==2:
                assert_responsibility_package_rejection(self,pipeline,workspace/'omitted-responsibilities',required)
                negatives.append('package-omitted-responsibilities')
                author=managed/'author.rs';original=author.read_bytes()
                # The actual compiler dependency check remains independently
                # effective even if a private parser were to miss a macro alias.
                plan=json.loads((pipeline.source/'reader-plan.json').read_text())
                real_run=values_driver.run_tool
                external=workspace/'hidden.txt';external.write_text('hidden input')
                for extra in ('const HIDDEN: &str = include_str!("'+str(external)+'");', 'const HIDDEN: &str = env!("LC_ALL");'):
                    author.write_bytes(original+('\n'+extra+'\n').encode())
                    def reader_bypass(config,name,args,**kwargs):
                        if name=='values_reader':return json.dumps(plan).encode()
                        return real_run(config,name,args,**kwargs)
                    try:
                        with tempfile.TemporaryDirectory(prefix='actual-dependency-',dir=workspace) as scratch:
                            with patch('values_driver.run_tool',side_effect=reader_bypass):
                                with self.assertRaisesRegex(NativeError,'undeclared author inputs or environment'):
                                    values_driver.read_plan(prepared['config'],managed,Path(scratch),'reading_demo')
                        unchanged()
                    finally:author.write_bytes(original)
                negatives.append('compiler-input-and-environment-closure')
                try:
                    author.write_bytes(original+b'\n// changed after freeze\n')
                    with self.assertRaises(RuntimeError):Pipeline(ROOT,managed,prepared,host).metadata()
                    unchanged();negatives.append('changed-frozen-author')
                finally:author.write_bytes(original)
                for kind in ('unknown-policy-selector','conflicting-policy'):
                    bad=deepcopy(prepared);project=bad['project']
                    if kind=='unknown-policy-selector':project['policies'][0]['selector']['id']='reading_demo.missing'
                    else:
                        duplicate=deepcopy(next(p for p in project['policies'] if p['slot']['kind']=='call'))
                        duplicate['values']['blocking']='bounded';duplicate['origin']['id']='conflicting.call'
                        project['policies'].append(duplicate)
                    save(managed,'project.json',project)
                    bad['lock']['project_digest']=project_digest(project);save(managed,'madb.lock',bad['lock'])
                    bad['read']['lock']=ref(managed,'madb.lock','madb.lock/1');save(managed,'read.json',bad['read'])
                    try:
                        with self.assertRaises(RuntimeError):
                            Pipeline(ROOT,managed,bad,host).metadata()
                        # The owner exposes PIPE003, retaining the concrete
                        # adapter failure. Rerun the exact locked reader/finalizer
                        # envelope to check its policy diagnostic as well.
                        with tempfile.TemporaryDirectory(prefix='policy-rejection-',dir=workspace) as scratch:
                            with self.assertRaisesRegex(NativeError,'POL001' if kind=='unknown-policy-selector' else 'POL004'):
                                values_driver.metadata(bad['read'],bad['config'],managed,Path(scratch))
                        unchanged();negatives.append(kind)
                    finally:
                        for key,name in (('project','project.json'),('lock','madb.lock'),('read','read.json')):save(managed,name,prepared[key])
                bad=Pipeline(ROOT,managed,deepcopy(prepared),host);bad.contract=pipeline.contract
                bad.read['selector']['target']='unsupported-value-target'
                with self.assertRaises(RuntimeError):bad.generate()
                unchanged();negatives.append('generation-selector-mismatch')
                invalid=workspace/'unsupported.rs';invalid.write_bytes(original+b'\npub type Hidden = u64;\n')
                with self.assertRaises(NativeError):
                    prepare(ROOT,workspace/'unsupported-preparation',observation,invalid,'reading_demo','unsupported',reader)
                unchanged();negatives.append('unsupported-author-declaration')
            if number==4:
                native=(candidate/'include/reading_demo_native.h').read_text()
                helper=(candidate/'include/reading_demo.h').read_text()
                for removed in ('normalize','offset'):
                    self.assertNotIn('reading_demo_raw_'+removed,native)
                    self.assertNotIn('reading_demo_'+removed+'(',helper)
            results.append(dict(variant=variant,selector=selector,required_features=sorted(expected_features),candidate_digest=manifest['candidate_digest'],candidate=str(candidate),preserved_after_rejections=negatives))
        self.assertEqual(results[0]['selector']['profile_digest'],results[4]['selector']['profile_digest'])
        self.assertEqual(len({results[i]['selector']['profile_digest'] for i in (1,2,3)}),1)
        self.assertNotEqual(results[0]['selector']['profile_digest'],results[1]['selector']['profile_digest'])
        save(workspace,'live-result.json',dict(scope='development-same-managed-root-boundary-profile-id',profile_semantics='actual required features change with Result declarations',results=results))

if __name__=='__main__':unittest.main()

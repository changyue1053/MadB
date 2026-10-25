"""Language SDK assembly and genuine independent producer smoke consumers."""
from pathlib import Path
import os
import shutil
from .worker_support import canonical, copy_file, input_path, load, file_ref
from .rust_build import build_provider

JARS=['jna-5.19.1.jar','kotlinx-coroutines-core-jvm-1.11.0.jar','kotlin-stdlib-2.4.20.jar','annotations-23.0.0.jar']
KLIBS=['kotlinx-coroutines-core-macosarm64-1.11.0.klib','atomicfu-macosarm64-0.32.1.klib','atomicfu-macosarm64-0.32.1-cinterop-interop.klib']


def files(root):return sorted(p for p in root.rglob('*') if p.is_file())

def dependencies(config):return Path(config['toolchain']['trees']['runtime-dependencies']['root'])

def native_flags(commands):
    (commands.scratch/'konan-cache').mkdir(exist_ok=True)
    return ['-target','macos_arm64','-opt','-Xoverride-konan-properties=minVersion.macos=14.0;airplaneMode=true',
            '-Xdebug-prefix-map='+str(commands.scratch)+'=/madb/build,'+str(commands.inputs)+'=/madb/inputs',
            '-Xklib-normalize-absolute-path','-Xklib-relative-path-base='+str(commands.scratch)+','+str(commands.inputs),
            '-Xcache-directory='+str(commands.scratch/'konan-cache')]


def native_sdk(config,generated,output,commands,*,bindings=True,facade=None):
    headers=list((generated/'nativeInterop').rglob('*.h'))
    if len(headers)!=1:raise ValueError('expected one Gobley cinterop header')
    header=headers[0];output.mkdir(parents=True,exist_ok=True)
    definition=commands.scratch/('p03.def' if not bindings else 'boundary.def')
    definition.write_text('headers = '+header.name+'\npackage = madb_boundary.cinterop\n')
    temporary=commands.scratch/('p03-interop' if not bindings else 'sdk-interop')
    temporary.mkdir(exist_ok=True)
    interop=temporary/'madb_boundary_interop'
    commands.run('cinterop',['-def',definition,'-compiler-option','-I'+str(header.parent),'-o',interop,'-target','macos_arm64',
        '-Xkotlinc-option','-Xklib-normalize-absolute-path','-Xkotlinc-option','-Xklib-relative-path-base='+str(commands.scratch)+','+str(commands.inputs)])
    copy_file(interop.with_suffix('.klib'),output/'madb_boundary_interop.klib')
    if bindings:
        common=sorted((generated/'commonMain').rglob('*.kt'));native=sorted((generated/'nativeMain').rglob('*.kt'))
        args=[*common,*native,*([facade] if facade else []),'-produce','library','-module-name','madb_boundary','-o',temporary/'madb_boundary',
              '-Xmulti-platform','-Xexpect-actual-classes','-Xcommon-sources='+','.join(map(str,common)),
              '-library',interop.with_suffix('.klib'),*native_flags(commands)]
        for name in KLIBS:args+=['-library',dependencies(config)/name]
        commands.run('konanc',args)
        copy_file(temporary/'madb_boundary.klib',output/'madb_boundary.klib')


def compile_sdk(config,inputs,work,commands):
    variant=config['variant'];public=work/'objects';public.mkdir()
    provider=build_provider(config,inputs,commands)
    copy_file(provider,public/'lib'/provider.name)
    runtime_protocol='sdk/rust/'+('gobley' if variant=='kotlin-native-rust' else 'uniffi')+'/runtime-protocol.json'
    copy_file(input_path(inputs,runtime_protocol),public/'runtime-protocol.json')
    generated=inputs/'generated'/('gobley' if variant=='kotlin-native-rust' else 'uniffi')
    if variant=='kotlin-jvm-rust':
        sources=sorted(generated.rglob('*.kt'))
        facade=input_path(inputs,'generated/facade/MadbBoundaryClient.kt')
        copy_file(facade,public/'kotlin/MadbBoundaryClient.kt')
        if not sources:raise ValueError('missing generated JVM sources')
        for p in sources:copy_file(p,public/'kotlin'/p.relative_to(generated))
        for p in generated.rglob('*.h'):copy_file(p,public/'include'/p.name)
        for name in JARS:copy_file(dependencies(config)/name,public/'dependencies'/name)
        (public/'sdk').mkdir()
        commands.run('kotlinc',[*sources,facade,'-jvm-target','21','-classpath',os.pathsep.join(str(dependencies(config)/n) for n in JARS),'-d',public/'sdk/madb-boundary.jar'])
    elif variant=='kotlin-native-rust':
        for p in files(generated):
            copy_file(p,public/'kotlin'/p.relative_to(generated))
            if p.suffix=='.h':copy_file(p,public/'include'/p.name)
        for name in KLIBS:copy_file(dependencies(config)/name,public/'dependencies'/name)
        facade=input_path(inputs,'generated/facade/MadbBoundaryClient.kt')
        copy_file(facade,public/'kotlin/MadbBoundaryClient.kt')
        native_sdk(config,generated,public/'klib',commands,facade=facade)
        # P03 is a separately built, clearly named test-support image. Its coroutine
        # scope and lifecycle are not part of production Engine.awaitClosed().
        fixture=public/'fixtures/p03';library=build_provider(config,inputs,commands,test_support=True)
        copy_file(library,fixture/'lib'/library.name)
        generated_fixture=commands.scratch/'generated-p03'
        commands.run('gobley',['--library',library,'--out-dir',generated_fixture,'--config',input_path(inputs,'sdk/rust/gobley/uniffi.toml'),
             '--crate-paths','madb_boundary_gobley='+str(inputs/'sdk/rust/uniffi')])
        for p in files(generated_fixture):copy_file(p,fixture/p.relative_to(generated_fixture))
        native_sdk(config,generated_fixture,fixture/'klib',commands,bindings=False)
        p03_provenance(config,inputs,fixture,library.name)
    elif variant=='swift-rust':
        for p in files(generated):
            if p.suffix=='.swift':copy_file(p,public/'swift'/p.name)
            elif p.suffix in ('.h','.modulemap'):copy_file(p,public/'include'/p.name)
        facade=input_path(inputs,'generated/facade/MadbBoundaryClient.swift')
        copy_file(facade,public/'swift/MadbBoundaryClient.swift')
        swift_typecheck(config,public,commands)
    else:raise ValueError('unknown Rust route variant: '+variant)
    mismatch=build_provider(config,inputs,commands,profile='sha256:'+'f'*64)
    copy_file(mismatch,public/'fixtures/wrong-profile/lib'/mismatch.name)
    (public/'build-locks.json').write_bytes(canonical(commands.lock_derivation)+b'\n')
    inventory=[p.relative_to(public).as_posix() for p in files(public)]
    (work/'sdk-inventory.json').write_bytes(canonical(dict(schema='madb.sdk-inventory/1',files=inventory))+b'\n')
    return ['sdk-inventory.json',*['objects/'+n for n in inventory]]


def p03_provenance(config, inputs, fixture, library_name):
    sources = []
    for name in ('sdk/rust/gobley/src/unsigned_fixture.rs', 'sdk/rust/uniffi/src/test_support.rs', 'sdk/rust/uniffi/src/boundary.udl'):
        target = fixture/'source'/Path(name).name
        copy_file(input_path(inputs,name),target)
        sources.append(file_ref(target,fixture))
    generator = config['toolchain']['tools']['gobley']
    metadata = {
        'role': 'P03 conformance fixture; not the production engine API',
        'component_metadata': {'format': 'UniFFI 0.29.4 embedded library metadata',
                               'library': file_ref(fixture/'lib'/library_name,fixture)},
        'generator': {key: generator[key] for key in ('id','version','sha256','size','source')},
        'exported_test_implementations': sources,
        'generated_bindings': [file_ref(path,fixture) for path in files(fixture) if path.suffix in ('.kt','.h')],
        'interop': file_ref(fixture/'klib/madb_boundary_interop.klib',fixture),
    }
    (fixture/'binding-provenance.json').write_bytes(canonical(metadata)+b'\n')


def swift_arguments(config,public,commands):
    maps=list((public/'include').glob('*.modulemap'))
    if len(maps)!=1:raise ValueError('expected one Swift C module map')
    return ['-swift-version','6','-strict-concurrency=complete','-O','-parse-as-library','-module-name','MadbBoundary',
            '-target','arm64-apple-macosx14.0','-sdk',config['toolchain']['sdk'],
            '-module-cache-path',commands.scratch/'swift-module-cache','-I',public/'include',
            '-Xcc','-fmodule-map-file='+str(maps[0]),'-debug-prefix-map',str(commands.scratch)+'=/madb/build',
            '-debug-prefix-map',str(commands.inputs)+'=/madb/inputs',*sorted((public/'swift').glob('*.swift'))]


def swift_typecheck(config,public,commands):
    commands.run('swiftc',[*swift_arguments(config,public,commands),'-typecheck'])


def link_smoke(config,inputs,work,commands):
    if config['variant']=='kotlin-jvm-rust':
        return _link_smoke_at(config,inputs,work,commands)
    from .smoke_workspace import stable_link
    source='adapters/toolchains/kotlin_rust/smoke.'+('swift' if config['variant']=='swift-rust' else 'kt')
    return stable_link(config,inputs,work,commands,_link_smoke_at,[source])


def _link_smoke_at(config,inputs,work,commands):
    inventory=load(input_path(inputs,'sdk-inventory.json'))
    names=inventory['files']
    for name in names:copy_file(input_path(inputs,'objects/'+name),work/name)
    variant=config['variant'];smoke=work/'native';smoke.mkdir()
    if variant=='swift-rust':
        source=input_path(inputs,'adapters/toolchains/kotlin_rust/smoke.swift')
        commands.run('swiftc',[*swift_arguments(config,work,commands),source,'-L',work/'lib','-lmadb_boundary_uniffi',
             '-Xlinker','-rpath','-Xlinker','@executable_path/../lib','-Xlinker','-reproducible',
             '-o',smoke/'smoke'])
        names=names+['native/smoke']
    elif variant=='kotlin-jvm-rust':
        source=input_path(inputs,'adapters/toolchains/kotlin_rust/smoke.kt')
        cp=[work/'sdk/madb-boundary.jar',*[work/'dependencies'/n for n in JARS]]
        commands.run('kotlinc',[source,'-jvm-target','21','-classpath',os.pathsep.join(map(str,cp)),'-d',smoke/'smoke.jar'])
        names=names+['native/smoke.jar']
    else:
        source=input_path(inputs,'adapters/toolchains/kotlin_rust/smoke.kt')
        # The linker UUID is computed before konanc's debug-map stripping. Random
        # intermediate object path lengths change that UUID even with -reproducible.
        # Keep those objects inside the existing exclusive cold workspace as well.
        args=[source,'-entry','madb.producer.main','-o',smoke/'smoke',*native_flags(commands),
              '-Xtemporary-files-dir='+str(commands.scratch/'native-temp')]
        for p in sorted((work/'klib').glob('*.klib'))+sorted((work/'dependencies').glob('*.klib')):args+=['-library',p]
        args+=['-linker-option','-L'+str(work/'lib'),'-linker-option','-lmadb_boundary_gobley',
               '-linker-option','-rpath','-linker-option=@executable_path/../lib','-linker-option','-Wl,-reproducible','-linker-option','-liconv']
        commands.run('konanc',args)
        # konanc adds .kexe; the public runnable name is stable across phases.
        (smoke/'smoke.kexe').rename(smoke/'smoke')
        names=names+['native/smoke']
    return names


def run_smoke(config,inputs,work,commands):
    target=commands.scratch/'consumer'
    for directory in ('lib','dependencies','sdk','klib','native'):
        root=inputs/directory
        if root.exists():
            for p in files(root):copy_file(input_path(inputs,p.relative_to(inputs).as_posix()),target/p.relative_to(inputs))
    identity=load(input_path(inputs,'identity.json'))
    if config['variant']=='kotlin-jvm-rust':
        cp=[target/'native/smoke.jar',target/'sdk/madb-boundary.jar',*[target/'dependencies'/n for n in JARS]]
        actual=commands.run('java',['-Djna.nosys=true','-Djna.library.path='+str(target/'lib'),'-classpath',os.pathsep.join(map(str,cp)),
                                    'madb.producer.SmokeKt',identity['profile_digest']],cwd=target)
    else:
        binary=target/'native/smoke';binary.chmod(0o700)
        actual=commands.run('smoke',[identity['profile_digest']],cwd=target,executable=binary)
    if actual!=b'MADB_C06_SMOKE_OK\n':raise RuntimeError('producer smoke returned an unexpected marker')
    (work/'native').mkdir(exist_ok=True)
    (work/'native/run.json').write_bytes(canonical(dict(status='passed',exit_code=0,variant=config['variant'],target=config['rust_target']))+b'\n')
    return ['native/run.json']

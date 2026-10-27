#!/usr/bin/env python3
"""Focused runtime proof; consumes only a public candidate and fixed Dart SDK.

The C shim and manual ABI adapter are test doubles for generated *bindings*, not
for the native engine. Injected lifecycle failures run separately from native
loading/normal-close evidence. This does not certify the extension generator.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time


RUNTIME = Path(__file__).resolve().parents[1]
CANDIDATE = "sha256:9a09515422305c2bf09a452c68d6afe373c27e277b510509f986c55395446959"
DART_SHA = "5f36d10cbdb3206fd6951e3090975d23b2f9659c41c2d17a7c8e183888f3f08d"
CLANG = Path('/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/clang')
SDK = '/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk'


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record(path: Path) -> dict:
    return dict(path=str(path), sha256=digest(path), size=path.stat().st_size)


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n')


def sources() -> list[dict]:
    return [record(p) for p in sorted(RUNTIME.rglob('*'))
            if p.is_file() and '.test-work' not in p.parts and '__pycache__' not in p.parts]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['dart', 'pub-cache', 'candidate', 'output']:
        parser.add_argument('--' + flag, required=True, type=Path)
    args = parser.parse_args()
    dart, candidate = args.dart.resolve(strict=True), args.candidate.resolve(strict=True)
    output = args.output.resolve()
    # All writable test output stays in this source owner's explicitly ignored area.
    output.relative_to(RUNTIME / '.test-work')
    output.mkdir(parents=True, exist_ok=False)
    report = dict(scope='runtime-only; test bindings; public native engine', result='failed',
                  commands=[], sourcesBefore=sources(), toolsBefore=[record(dart),record(CLANG)])
    # pub may maintain cache metadata even in offline mode: never write the
    # shared public tool/cache input. Use a private copy of the four packages.
    cache_input = args.pub_cache.resolve(strict=True)
    cache = output/'pub-cache'
    for name in ['crypto-3.0.8','ffi-2.2.0','typed_data-1.4.0','collection-1.19.1']:
        shutil.copytree(cache_input/'hosted/pub.dev'/name,cache/'hosted/pub.dev'/name)
        source=cache_input/'hosted-hashes/pub.dev'/(name+'.sha256')
        target=cache/'hosted-hashes/pub.dev'/source.name
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(source,target)
    env = dict(PATH='/usr/bin:/bin:/usr/sbin:/sbin', LANG='C.UTF-8', CI='true',
               PUB_CACHE=str(cache), TMPDIR=str(output / 'owned-tmp'))
    Path(env['TMPDIR']).mkdir()
    package = output / 'package'
    package.mkdir()

    def run(label: str, command: list, *, cwd: Path = package) -> str:
        command = [str(c) for c in command]
        start = time.monotonic()
        completed = subprocess.run(command, cwd=cwd, env=env, capture_output=True, timeout=120)
        stem = f'{len(report["commands"]):03d}-{label}'
        stdout, stderr = output / (stem + '.stdout'), output / (stem + '.stderr')
        stdout.write_bytes(completed.stdout)
        stderr.write_bytes(completed.stderr)
        report['commands'].append(dict(argv=command,cwd=str(cwd),exit=completed.returncode,
            elapsedSeconds=time.monotonic()-start,stdout=record(stdout),stderr=record(stderr)))
        write_json(output/'result.json',report)
        if completed.returncode:
            raise RuntimeError(f'{label} exit {completed.returncode}: {completed.stderr.decode(errors="replace")}')
        return completed.stdout.decode()

    try:
        if digest(dart) != DART_SHA:
            raise ValueError('fixed Dart binary mismatch')
        manifest = json.loads((candidate/'candidate.json').read_text())
        if manifest['candidate_digest'] != CANDIDATE:
            raise ValueError('public candidate mismatch')
        report['candidateBefore'] = [record(candidate/'candidate.json')]
        for item in manifest['files']:
            path = candidate/item['path']
            if path.is_symlink() or path.resolve().parent == candidate.parent:
                raise ValueError('candidate path violation')
            path.resolve().relative_to(candidate)
            if digest(path) != item['sha256'].removeprefix('sha256:') or path.stat().st_size != item['size']:
                raise ValueError(f'candidate bytes mismatch: {path}')
            report['candidateBefore'].append(record(path))
        run('dart-version',[dart,'--version'])
        for name in ['madb_runtime.dart', 'madb_value_runtime.dart']:
            target=package/'lib/src'/name
            target.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(RUNTIME/name,target)
        for source,target in [('library.dart.in','lib/madb_dart_ffi.dart'),
                              ('test_calls.dart.in','lib/src/test_calls.dart'),
                              ('checks.dart.in','lib/src/checks.dart')]:
            shutil.copyfile(RUNTIME/'test'/source,package/target)
        (package/'pubspec.yaml').write_text("name: madb_runtime_checks\npublish_to: none\nenvironment:\n  sdk: '>=3.13.7 <3.14.0'\ndependencies:\n  crypto: 3.0.8\n  ffi: 2.2.0\n")
        run('pub-offline',[dart,'--suppress-analytics','pub','get','--offline'])
        run('analyze',[dart,'--suppress-analytics','analyze'])
        bridge = output/'bridge.dylib'
        native = candidate/'lib/libmadb_engine.dylib'
        run('compile-bridge',[CLANG,'-target','arm64-apple-macos11.0','-isysroot',SDK,
            '-dynamiclib','-O2','-Wall','-Wextra','-Werror','-I',candidate/'include',
            RUNTIME/'test/native_bridge.c',native,'-Wl,-headerpad_max_install_names',
            '-Wl,-install_name,@loader_path/bridge.dylib','-o',bridge])
        run('bridge-dependencies',['/usr/bin/otool','-L',bridge])
        run('link-local',['/usr/bin/install_name_tool','-change','@rpath/libmadb_engine.dylib',
            '@loader_path/native.dylib',bridge])
        run('sign-local',['/usr/bin/codesign','--force','--sign','-',bridge])
        foreign = output/'foreign.dylib'
        shutil.copyfile(bridge,foreign)
        run('link-foreign',['/usr/bin/install_name_tool','-change','@loader_path/native.dylib',native,foreign])
        run('sign-foreign',['/usr/bin/codesign','--force','--sign','-',foreign])
        identity = json.loads((candidate/'identity.json').read_text())
        fixtures=[]

        def fixture(name: str, mode: str, mutate=None, *, stale_index=False) -> None:
            folder=output/'fixtures'/name
            folder.mkdir(parents=True)
            for source,target in [(bridge,'bridge.dylib'),(native,'native.dylib'),
                                  (candidate/'identity.json','identity.json')]:
                shutil.copyfile(source,folder/target)
            index=dict(bridge='bridge.dylib',native='native.dylib',identity='identity.json',files=[])
            pins=dict(identity)
            def seal():
                index['files']=[dict(path=p,sha256=digest(folder/p),size=(folder/p).stat().st_size)
                                for p in ['bridge.dylib','native.dylib','identity.json']]
            seal()
            if mutate:
                mutate(folder,index,pins)
            if not stale_index and name not in {'escape','duplicate','unknown-key','case-alias'}:
                seal()
            write_json(folder/'install.json',index)
            fixtures.append(dict(name=name,mode=mode,path=str(folder),installDigest=digest(folder/'install.json'),
                interfaceDigest=pins['interface_digest'],profileDigest=pins['profile_digest']))

        fixture('normal','normal')
        fixture('foreign','load:imageGraphMismatch',lambda f,i,p:shutil.copyfile(foreign,f/'bridge.dylib'))
        fixture('bad-bytes','load:digestMismatch',lambda f,i,p:(f/'native.dylib').write_bytes(b'changed'),stale_index=True)
        fixture('missing','load:missingAsset',lambda f,i,p:(f/'native.dylib').unlink(),stale_index=True)
        def symlink(f,i,p):
            (f/'native.dylib').unlink()
            (f/'native.dylib').symlink_to(native)
        fixture('symlink','load:pathViolation',symlink,stale_index=True)
        fixture('extra','load:pathViolation',lambda f,i,p:(f/'unlisted').write_text('extra'))
        fixture('escape','load:pathViolation',lambda f,i,p:i.update(native='../native.dylib'))
        fixture('duplicate','load:invalidPackage',lambda f,i,p:i['files'].__setitem__(1,dict(i['files'][0])))
        fixture('unknown-key','load:invalidPackage',lambda f,i,p:i.update(unrecognized=True))
        fixture('case-alias','load:pathViolation',lambda f,i,p:i.update(native='Native.dylib'))
        for field,kind in [('interface_digest','interfaceMismatch'),('profile_digest','profileMismatch')]:
            def identity_mismatch(f,i,p,field=field):
                p[field]='sha256:'+'1'*64
                write_json(f/'identity.json',p)
            fixture(kind,'identity:'+kind,identity_mismatch)
        write_json(output/'fixtures.json',fixtures)
        run('jit-policy',[dart,'--suppress-analytics','run','lib/madb_dart_ffi.dart','policy'])
        for f in fixtures:
            run('jit-'+f['name'],[dart,'--suppress-analytics','run','lib/madb_dart_ffi.dart',
                f['mode'],f['path'],f['installDigest'],f['interfaceDigest'],f['profileDigest']])
        exe=output/'runtime-checks'
        run('compile-aot',[dart,'--suppress-analytics','compile','exe','lib/madb_dart_ffi.dart','-o',exe])
        run('aot-policy',[exe,'policy'])
        for f in fixtures:
            run('aot-'+f['name'],[exe,f['mode'],f['path'],f['installDigest'],f['interfaceDigest'],f['profileDigest']])
        report['generatedTestPackage']=[record(p) for p in sorted((package/'lib').rglob('*.dart'))]
        report['pubLock']=record(package/'pubspec.lock')
        report['result']='passed'
    finally:
        report['sourcesAfter']=sources()
        report['toolsAfter']=[record(dart),record(CLANG)]
        report['candidateAfter']=[record(Path(p['path'])) for p in report.get('candidateBefore',[])]
        report['inputsUnchanged']=(report['sourcesBefore']==report['sourcesAfter'] and
            report['toolsBefore']==report['toolsAfter'] and report.get('candidateBefore')==report['candidateAfter'])
        report['privateCacheRetained']=env['TMPDIR']
        if not report['inputsUnchanged']:
            report['result']='failed'
        write_json(output/'result.json',report)
    if report['result']!='passed':
        raise RuntimeError('runtime proof failed')
    print(json.dumps(dict(result=report['result'],report=str(output/'result.json'),sha256=digest(output/'result.json'))))


if __name__=='__main__':
    main()

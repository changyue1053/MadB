"""Resolve the explicitly selected local native toolchain and record actual images."""
from pathlib import Path
import json
import os
import platform
import shutil
import subprocess

from .common import sha_file


def query(*args):
    return subprocess.check_output(list(map(str, args)), text=True, timeout=30).strip()


def discover(root, bindgen, cbindgen):
    recipe = json.loads((root / 'adapters/toolchains/rust_c/toolchain.json').read_text())
    rustc = Path(query('rustup', 'which', '--toolchain', recipe['rust'], 'rustc')).resolve(strict=True)
    host = next(x.split(': ', 1)[1] for x in query(rustc, '-vV').splitlines() if x.startswith('host: '))
    if query(rustc, '--version').split()[1] != recipe['rust']:
        raise RuntimeError('Rust version differs from the first-route lock')
    apple = platform.system() == 'Darwin'
    if apple:
        clang = Path(query('xcrun', '--find', 'clang')).resolve(strict=True)
        sdk = Path(query('xcrun', '--show-sdk-path')).resolve(strict=True)
        if (query('xcodebuild', '-version') != recipe['xcode'] or query('xcrun', '--show-sdk-version') != recipe['sdk_version']
                or query('xcrun', '--show-sdk-build-version') != recipe['sdk_build']):
            raise RuntimeError('Apple first-route requires the reviewed Xcode/SDK lock')
        if query(clang, '--version').splitlines()[0] != recipe['clang']:
            raise RuntimeError('Apple Clang differs from the reviewed lock')
        libclang = (clang.parent.parent / 'lib/libclang.dylib').resolve(strict=True)
        ar = Path(query('xcrun', '--find', 'ar')).resolve(strict=True)
        nm = Path(query('xcrun', '--find', 'nm')).resolve(strict=True)
        flags = ['-target', 'arm64-apple-macosx14.0' if host == 'aarch64-apple-darwin' else 'x86_64-apple-macosx14.0', '-isysroot', str(sdk), '-mmacosx-version-min=14.0', '-resource-dir', query(clang, '--print-resource-dir'), '-std=c11', '-Wall', '-Wextra', '-Werror']
        sdk_inputs = [sdk / 'SDKSettings.json', sdk / 'usr/lib/libSystem.tbd']
    else:
        if platform.system() != 'Linux':
            raise RuntimeError('C05 native producer supports the reviewed macOS/Linux hosts only')
        clang = Path('/usr/bin/clang' if Path('/usr/bin/clang').is_file() else shutil.which('clang') or '').resolve(strict=True)
        ar = Path(shutil.which('llvm-ar') or shutil.which('ar') or '').resolve(strict=True)
        nm = Path(shutil.which('llvm-nm') or shutil.which('nm') or '').resolve(strict=True)
        candidates = sorted(Path('/usr/lib').glob('llvm-*/lib/libclang.so*')) + sorted(Path('/usr/lib/x86_64-linux-gnu').glob('libclang-*.so*'))
        override = os.environ.get('LIBCLANG_PATH')
        if override:
            candidates = sorted(Path(override).glob('libclang.so*'))
        if not candidates:
            raise RuntimeError('libclang is required; install the native Clang development package')
        libclang = candidates[-1].resolve(strict=True)
        flags = ['-target', host, '-resource-dir', query(clang, '--print-resource-dir'), '-std=c11', '-Wall', '-Wextra', '-Werror']
        sdk_inputs = []
    files = {'rustc': rustc, 'clang': clang, 'ar': ar, 'nm': nm, 'libclang': libclang,
             'bindgen': Path(bindgen).resolve(strict=True), 'cbindgen': Path(cbindgen).resolve(strict=True),
             'python': Path(os.sys.executable).resolve(strict=True), 'finalizer': (root / 'target/debug/madb-native-finalize').resolve(strict=True)}
    for name in ['bindgen', 'cbindgen']:
        expected = recipe['generators'][name]
        receipt = json.loads((files[name].parent.parent / 'receipt.json').read_text())
        recorded = receipt['generators'][name]
        if receipt['rust'] != recipe['rust'] or any(recorded[k] != expected[k] for k in expected) or recorded['binary_sha256'] != sha_file(files[name]):
            raise RuntimeError(name + ' provenance/installed image receipt differs')
        if (query(files[name], '--version') != name + ' ' + expected['version']
                or recorded['binary_sha256'] != sha_file(files[name])):
            raise RuntimeError(name + ' version or post-execution image differs from the reviewed lock')
    tools = {}
    for name, path in files.items():
        version = recipe['generators'][name]['version'] if name in recipe['generators'] else recipe['rust'] if name == 'rustc' else platform.python_version() if name == 'python' else query(clang, '--version').splitlines()[0] if name in ['clang', 'libclang', 'ar', 'nm'] else '0.1.0'
        tools[name] = dict(path=str(path), sha256=sha_file(path), size=path.stat().st_size, id='c05-' + name, version=version)
    sysroot = Path(query(rustc, '--print', 'sysroot'))
    sdk_inputs += sorted((sysroot / 'lib/rustlib' / host / 'lib').glob('*.rlib'))
    observation = dict(schema='madb.native-toolchain/1', target=host, system=platform.system(),
                       tools=tools, clang_arguments=flags, sdk_inputs=[dict(path=str(p),sha256=sha_file(p),size=p.stat().st_size) for p in sdk_inputs],
                       environment={'LIBCLANG_PATH': str(libclang.parent), 'LC_ALL': 'C'})
    return observation

"""Build all three providers from repository source; no prebuilt Rust input."""
from pathlib import Path
import importlib.util
import os
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
TESTS = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location(
    'c07_native_check', ROOT / 'tests/integration/c07_native_protocol/check.py')
NATIVE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(NATIVE)
SDK = NATIVE.SDK

import shlex
import tomllib


def main():
    compilers = NATIVE.compilers_required()
    cargo = shutil.which('cargo')
    rustc = shutil.which('rustc')
    if not cargo or not rustc:
        raise RuntimeError('Project Rust and Cargo are required; no prebuilt provider fallback')
    output = Path(tempfile.mkdtemp(prefix='madb-c07-provider-check-'))
    print('C07 actual provider host logs: ' + str(output), flush=True)
    environment = NATIVE.checked_environment()
    records = []
    for name, compiler in zip(('clang', 'clangxx'), compilers):
        records.append(NATIVE.run([compiler, '--version'], output, name, environment))
    records.append(NATIVE.run([rustc, '--version'], output, 'rustc', environment))
    expected = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    if (output / 'rustc.log').read_text().split()[1] != expected:
        raise RuntimeError('Provider must use project Rust ' + expected)
    package = ROOT / 'sdk/rust/arkts-provider'
    manifest = package / 'Cargo.toml'
    manifest_args = ['--manifest-path', str(manifest)]
    # Cargo resolves the repository toolchain; no global rustup setting changes.
    records.append(NATIVE.run([cargo, 'fmt', *manifest_args, '--', '--check'],
                              output, 'rust-format', environment))
    records.append(NATIVE.run([cargo, 'clippy', *manifest_args, '--offline', '--locked',
                              '--all-targets', '--target-dir', str(output / 'rust'),
                              '--', '-D', 'warnings'], output, 'rust-clippy', environment))
    records.append(NATIVE.run([cargo, 'rustc', *manifest_args, '--lib', '--release',
                              '--offline', '--locked', '--target-dir', str(output / 'rust'),
                              '--', '--print', 'native-static-libs'],
                             output, 'rust-build', environment))
    lines = (output / 'rust-build.log').read_text().splitlines()
    prefix = 'note: native-static-libs:'
    dependencies = [line.split(prefix, 1)[1].strip() for line in lines if prefix in line]
    if len(dependencies) != 1:
        raise RuntimeError('Rust did not report one exact native static link dependency list')
    native_libraries = shlex.split(dependencies[0])
    name = tomllib.loads(manifest.read_text())['package']['name'].replace('-', '_')
    archive = output / 'rust/release' / ('lib' + name + '.a')
    if not archive.is_file():
        raise RuntimeError('Cargo did not produce the actual repository provider archive')
    c_source = ROOT / 'sdk/c/arkts-provider/src/provider.c'
    cpp_source = ROOT / 'sdk/cpp/arkts-provider/src/provider.cpp'
    for configuration, flags in NATIVE.CONFIGURATIONS:
        common = ['-Wall', '-Wextra', '-Werror', *flags, '-I', str(SDK / 'include')]
        objects = []
        for language, compiler, standard, source in (
            ('c', compilers[0], 'c17', c_source),
            ('cpp', compilers[1], 'c++17', cpp_source),
        ):
            obj = output / (configuration + '-' + language + '.o')
            records.append(NATIVE.run([compiler, '-std=' + standard, *common,
                                       '-c', str(source), '-o', str(obj)], output,
                                      configuration + '-' + language, environment))
            objects.append(str(obj))
        executable = output / configuration
        records.append(NATIVE.run([compilers[1], '-std=c++17', '-pthread', *common,
                                   str(SDK / 'src/protocol.cpp'), str(TESTS / 'provider_tests.cpp'),
                                   *objects, str(archive), *native_libraries, '-o', str(executable)],
                                  output, configuration + '-link', environment))
        records.append(NATIVE.run([str(executable)], output, configuration + '-behavior', environment))
    inputs = [Path(__file__), Path(NATIVE.__file__), manifest, package / 'Cargo.lock',
              package / 'src/lib.rs', ROOT / 'rust-toolchain.toml',
              c_source, cpp_source, TESTS / 'provider_tests.cpp', SDK / 'src/protocol.cpp']
    inputs.extend((SDK / 'include').glob('*'))
    NATIVE.write_result(output,
        'Three actual providers; host Release and C/C++ ASan/UBSan; Rust is not sanitizer-instrumented; no OHOS certification',
        records, inputs, providers=['C17 malloc/free', 'C++17 RAII new/delete', 'Rust Box/String'],
        rust_archive=str(archive), rust_native_libraries=native_libraries)
    print('C07 real three-provider host checks passed; target gateway/metadata verification remains separate.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1) from error

"""Real worker, external join ACK and adversarial host lifetime checks."""
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

import platform


def main():
    compilers = NATIVE.compilers_required()
    host = platform.system()
    if host not in ('Darwin', 'Linux'):
        raise RuntimeError('Worker failure injection requires Darwin interpose or Linux LD_PRELOAD; unsupported host is not a pass')
    output = Path(tempfile.mkdtemp(prefix='madb-c07-worker-check-'))
    print('C07 actual worker host logs: ' + str(output), flush=True)
    environment = NATIVE.checked_environment()
    records = []
    for name, compiler in zip(('clang', 'clangxx'), compilers):
        records.append(NATIVE.run([compiler, '--version'], output, name, environment))
    c_source = ROOT / 'sdk/c/arkts-provider/src/provider.c'
    for configuration, flags in NATIVE.CONFIGURATIONS:
        common = ['-Wall', '-Wextra', '-Werror', *flags, '-I', str(SDK / 'include')]
        obj = output / (configuration + '-provider.o')
        records.append(NATIVE.run([compilers[0], '-std=c17', *common, '-c', str(c_source),
                                   '-o', str(obj)], output, configuration + '-c', environment))
        for case in ('worker_tests', 'stale_thread_id', 'tls_destructor_self_join'):
            executable = output / (configuration + '-' + case)
            records.append(NATIVE.run([compilers[1], '-std=c++17', '-pthread', *common,
                                       str(SDK / 'src/protocol.cpp'), str(SDK / 'src/worker.cpp'),
                                       str(TESTS / (case + '.cpp')), str(obj), '-o', str(executable)],
                                      output, configuration + '-' + case + '-compile', environment))
            records.append(NATIVE.run([str(executable)], output, configuration + '-' + case, environment))
    # Deliberately isolated Release process: a global pthread interposer would
    # also deny sanitizer/runtime helper threads and is never loaded into the test runner.
    dynamic_flags = ['-dynamiclib'] if host == 'Darwin' else ['-shared', '-fPIC']
    library = output / ('fail-pthread.dylib' if host == 'Darwin' else 'fail-pthread.so')
    records.append(NATIVE.run([compilers[0], '-std=c17', '-Wall', '-Wextra', '-Werror',
                               '-O2', *dynamic_flags, str(TESTS / 'fail_pthread_create.c'),
                               '-o', str(library)], output, 'thread-failure-interposer', environment))
    executable = output / 'failed-thread-creation'
    link_libraries = [] if host == 'Darwin' else ['-ldl']
    records.append(NATIVE.run([compilers[1], '-std=c++17', '-pthread', '-Wall', '-Wextra', '-Werror',
                               '-O2', '-DNDEBUG', '-I', str(SDK / 'include'), str(SDK / 'src/protocol.cpp'),
                               str(SDK / 'src/worker.cpp'), str(TESTS / 'failed_thread_creation.cpp'),
                               *link_libraries, '-o', str(executable)],
                              output, 'thread-failure-compile', environment))
    interposed = dict(environment)
    interposed['DYLD_INSERT_LIBRARIES' if host == 'Darwin' else 'LD_PRELOAD'] = str(library)
    records.append(NATIVE.run([str(executable)], output, 'real-pthread-EAGAIN-rollback', interposed))
    inputs = [Path(__file__), Path(NATIVE.__file__), c_source,
              SDK / 'src/protocol.cpp', SDK / 'src/worker.cpp']
    inputs.extend((SDK / 'include').glob('*'))
    inputs.extend(path for path in TESTS.iterdir() if path.suffix in ('.cpp', '.c'))
    NATIVE.write_result(output,
        'Host worker behavior, C/C++ Release and ASan/UBSan; pthread EAGAIN injection Release only; no target/env certification',
        records, inputs, host=host, cases_per_configuration=12, release_only_cases=1)
    print('C07 real worker host checks passed; actual target async cleanup remains separate.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1) from error

"""Real C/C++ host protocol checks; not OHOS execution or candidate acceptance."""
from pathlib import Path
import hashlib
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
SDK = ROOT / 'sdk/arkts/native-runtime'
TESTS = Path(__file__).resolve().parent
CONFIGURATIONS = (
    ('release', ['-O2', '-DNDEBUG']),
    ('asan-ubsan', ['-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer']),
)


def compilers_required():
    compilers = [shutil.which(name) for name in ('clang', 'clang++')]
    if not all(compilers):
        raise RuntimeError('Clang C and C++ compilers are required; see tests/integration/c07_native_protocol/README.md')
    return compilers


def checked_environment():
    return dict(os.environ, ASAN_OPTIONS='halt_on_error=1:abort_on_error=1:detect_leaks=0',
                UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')


def write_result(output, scope, records, inputs, **details):
    sources = []
    for path in sorted(set(inputs)):
        data = path.read_bytes()
        sources.append(dict(path=path.relative_to(ROOT).as_posix(), size=len(data),
                            sha256=hashlib.sha256(data).hexdigest()))
    (output / 'result.json').write_text(json.dumps(dict(
        scope=scope, sources=sources, records=records, **details), indent=2) + '\n')


def run(command, output, name, environment):
    log = output / (name + '.log')
    started = time.monotonic()
    with log.open('wb') as stream:
        process = subprocess.Popen(command, cwd=ROOT, env=environment,
                                   stdin=subprocess.DEVNULL, stdout=stream,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            code = process.wait(timeout=120)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=10)
            raise RuntimeError('native check timed out; log: ' + str(log))
    if code:
        raise RuntimeError('native check failed; log: ' + str(log))
    print(name + ': passed', flush=True)
    observed_environment = {key: environment[key] for key in (
        'ASAN_OPTIONS', 'UBSAN_OPTIONS', 'DYLD_INSERT_LIBRARIES', 'LD_PRELOAD') if key in environment}
    return dict(name=name, command=command, exit_code=code, log=str(log),
                duration_ms=round((time.monotonic() - started) * 1000),
                environment=observed_environment)


def main():
    compilers = compilers_required()
    output = Path(tempfile.mkdtemp(prefix='madb-c07-native-check-'))
    environment = os.environ.copy()
    records = []
    print('C07 host protocol logs: ' + str(output), flush=True)
    for name, compiler in zip(('clang', 'clangxx'), compilers):
        records.append(run([compiler, '--version'], output, name, environment))
    for suite in ('author', 'independent', 'typed'):
        sources = TESTS / (suite if suite != 'typed' else 'author')
        test = TESTS / suite / ('typed_clone_tests.cpp' if suite == 'typed' else 'protocol_tests.cpp')
        for configuration, flags in CONFIGURATIONS:
            label = suite + '-' + configuration
            obj = output / (label + '.o')
            executable = output / label
            common = ['-Wall', '-Wextra', '-Werror', *flags]
            records.append(run([compilers[0], '-std=c17', *common, '-I', str(sources),
                                '-c', str(sources / 'c_provider.c'), '-o', str(obj)],
                               output, label + '-c', environment))
            records.append(run([compilers[1], '-std=c++17', *common, '-pthread',
                                '-I', str(SDK / 'include'), '-I', str(sources),
                                str(SDK / 'src/protocol.cpp'), str(test),
                                str(obj), '-o', str(executable)], output, label + '-cpp', environment))
            test_environment = checked_environment()
            if suite != 'independent':
                records.append(run([str(executable)], output, label + '-behavior', test_environment))
            else:
                # The independent oracle isolates every case in its own process.
                records.append(run([str(executable), '--list'], output, label + '-list', test_environment))
                cases = (output / (label + '-list.log')).read_text().splitlines()
                if len(cases) != 25 or len(cases) != len(set(cases)) or any(not name.isidentifier() for name in cases):
                    raise RuntimeError('independent native case discovery is invalid')
                for case in cases:
                    records.append(run([str(executable), case], output, label + '-' + case, test_environment))
    inputs = [Path(__file__), SDK / 'include/protocol.hpp', SDK / 'src/protocol.cpp']
    inputs.extend(path for path in TESTS.rglob('*') if path.suffix in ('.cpp', '.c', '.h'))
    write_result(output, 'Host Release and ASan/UBSan; no LeakSanitizer, OHOS target or SDK certification',
                 records, inputs, cases_per_configuration=dict(author=20, independent=25, typed=11))
    print('C07 private protocol host checks passed; target verification remains separate.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1) from error

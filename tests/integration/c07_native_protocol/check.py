"""Real C/C++ host protocol checks; not OHOS execution or candidate acceptance."""
from pathlib import Path
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
SDK = ROOT / 'sdk/arkts/native-runtime'
TESTS = Path(__file__).resolve().parent


def run(command, output, name, environment):
    log = output / (name + '.log')
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
    return dict(name=name, command=command, exit_code=code, log=str(log))


def main():
    compilers = [shutil.which(name) for name in ('clang', 'clang++')]
    if not all(compilers):
        raise RuntimeError('Clang C and C++ compilers are required; see tests/integration/c07_native_protocol/README.md')
    output = Path(tempfile.mkdtemp(prefix='madb-c07-native-check-'))
    environment = os.environ.copy()
    records = []
    print('C07 host protocol logs: ' + str(output), flush=True)
    for name, compiler in zip(('clang', 'clangxx'), compilers):
        records.append(run([compiler, '--version'], output, name, environment))
    for suite in ('author', 'independent'):
        sources = TESTS / suite
        for configuration, flags in (
            ('release', ['-O2', '-DNDEBUG']),
            ('asan-ubsan', ['-O1', '-g', '-fsanitize=address,undefined', '-fno-omit-frame-pointer']),
        ):
            label = suite + '-' + configuration
            obj = output / (label + '.o')
            executable = output / label
            common = ['-Wall', '-Wextra', '-Werror', *flags]
            records.append(run([compilers[0], '-std=c17', *common, '-I', str(sources),
                                '-c', str(sources / 'c_provider.c'), '-o', str(obj)],
                               output, label + '-c', environment))
            records.append(run([compilers[1], '-std=c++17', *common, '-pthread',
                                '-I', str(SDK / 'include'), '-I', str(sources),
                                str(SDK / 'src/protocol.cpp'), str(sources / 'protocol_tests.cpp'),
                                str(obj), '-o', str(executable)], output, label + '-cpp', environment))
            checked_environment = dict(environment, ASAN_OPTIONS='halt_on_error=1:abort_on_error=1:detect_leaks=0',
                                       UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1')
            if suite == 'author':
                records.append(run([str(executable)], output, label + '-behavior', checked_environment))
            else:
                # The independent oracle isolates every case in its own process.
                records.append(run([str(executable), '--list'], output, label + '-list', checked_environment))
                cases = (output / (label + '-list.log')).read_text().splitlines()
                if not cases or len(cases) != len(set(cases)) or any(not name.isidentifier() for name in cases):
                    raise RuntimeError('independent native case discovery is invalid')
                for case in cases:
                    records.append(run([str(executable), case], output, label + '-' + case, checked_environment))
    (output / 'result.json').write_text(json.dumps(dict(
        scope='Host Release and ASan/UBSan; no LeakSanitizer, OHOS target or SDK certification',
        records=records), indent=2) + '\n')
    print('C07 private protocol host checks passed; target verification remains separate.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1) from error

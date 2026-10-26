"""Compile actual Kotlin owners with explicit JVM native/NAPI test fixtures.
No platform/worker/finalizer or physical-ACK certification is made by this runner.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--repository', type=Path, required=True)
parser.add_argument('--overlay', type=Path)
parser.add_argument('--managed-boundary', type=Path, required=True)
parser.add_argument('--maven-root', type=Path, required=True)
parser.add_argument('--java', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--mutant', choices=['ignore-handoff', 'drop-entered', 'wake-abandon'])
args = parser.parse_args()
root = Path(__file__).resolve().parent
out = args.output
out.mkdir(parents=True, exist_ok=False)

def source(name):
    relative = Path('sdk/kotlin/arkts-owned') / name
    candidate = args.overlay / relative if args.overlay else None
    return candidate if candidate and candidate.exists() else args.repository / relative

core = source('src/common/OwnedCore.kt')
core_text = core.read_text()
if args.mutant:
    before, after = {
        'ignore-handoff': ('if (!abandonPending) return', 'return'),
        'drop-entered': ('if (wait.entered || waitStatus(e, wait) == NOT_READY)', 'if (!wait.entered && waitStatus(e, wait) == NOT_READY)'),
        'wake-abandon': ('if (committed) wakeExecutor(executor)', 'wakeExecutor(executor)'),
    }[args.mutant]
    assert core_text.count(before) == 1
    core_text = core_text.replace(before, after)
    core = out / 'OwnedCore.kt'
    core.write_text(core_text)
managed = out / 'ManagedOwnedBoundary.kt'
managed.write_text(args.managed_boundary.read_text().split('\nfun reportManagedBoundaryError', 1)[0] + '\n')
files = [core, source('src/common/OwnedModel.kt'), source('src/common/OwnedExports.kt'), source('src/test/ExecutionProbe.kt'), managed, root / 'HandoffTest.kt', *sorted((root / 'stubs').glob('*.kt'))]
names = [
 'org/jetbrains/kotlin/kotlin-compiler-embeddable/2.2.21-1.1.0/kotlin-compiler-embeddable-2.2.21-1.1.0.jar',
 'org/jetbrains/kotlin/kotlin-stdlib/2.2.21-1.1.0/kotlin-stdlib-2.2.21-1.1.0.jar',
 'org/jetbrains/kotlin/kotlin-script-runtime/2.2.21-1.1.0/kotlin-script-runtime-2.2.21-1.1.0.jar',
 'org/jetbrains/kotlin/kotlin-reflect/1.6.10/kotlin-reflect-1.6.10.jar',
 'org/jetbrains/kotlinx/kotlinx-coroutines-core-jvm/1.8.0/kotlinx-coroutines-core-jvm-1.8.0.jar',
 'org/jetbrains/annotations/13.0/annotations-13.0.jar',
]
jars = [args.maven_root / n for n in names]
cp = ':'.join(str(p) for p in jars)
classes = out / 'classes'
records = []
def run(command):
    p = subprocess.run(command, capture_output=True, text=True, timeout=90)
    records.append(dict(command=command, exit_code=p.returncode, stdout=p.stdout, stderr=p.stderr))
    print(p.stdout, p.stderr, flush=True)
    return p.returncode

scenarios = ['healthy','upgrade','repeat','no-new-wait','full32','not-ready','entered-success','entered-failure','pre-guard','invalid-mode','stale-epoch','abort-stopped','reject-commit-stopped','commit-wake','real-wake-error']
if args.mutant:
    scenarios = [{'ignore-handoff':'pre-guard','drop-entered':'entered-success','wake-abandon':'abort-stopped'}[args.mutant]]
try:
    compile_command = [str(args.java), '-cp', cp, 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler', '-Xallow-kotlin-package', '-no-stdlib', '-no-reflect', '-classpath', cp, '-d', str(classes), *[str(p) for p in files]]
    assert run(compile_command) == 0, 'compile failed'
    for name in scenarios:
        code = run([str(args.java), '-cp', str(classes)+':'+cp, 'com.madb.c07.HandoffTestKt', name])
        if args.mutant:
            assert code != 0 and 'Check failed' in records[-1]['stderr'], 'mutant must fail an ownership assertion'
        else:
            assert code == 0, name
finally:
    inputs = files + jars + [args.java, args.managed_boundary]
    (out/'receipt.json').write_text(json.dumps(dict(scope='Actual production Kotlin ownership source with explicit JVM native/NAPI/executor fixtures; not target or physical ACK proof', mutant=args.mutant, scenarios=scenarios, inputs=[dict(path=str(p), sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in inputs], commands=records), indent=2)+'\n')

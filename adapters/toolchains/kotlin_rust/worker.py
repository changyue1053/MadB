"""C06 frozen compile/link/run ports; only BuildEvidence is emitted on stdout."""
from __future__ import annotations
import argparse
import hashlib
import importlib
import json
import os
from pathlib import Path
import sys
import tempfile
import types

# Isolate recipe-local modules when preview imports this worker in a process that
# already loaded another checkout. Formal execution uses the same frozen files.
PACKAGE='_madb_c06_worker_'+hashlib.sha256(str(Path(__file__).resolve().parent).encode()).hexdigest()[:16]
if PACKAGE not in sys.modules:
    package=types.ModuleType(PACKAGE);package.__path__=[str(Path(__file__).resolve().parent)];sys.modules[PACKAGE]=package
support=importlib.import_module(PACKAGE+'.worker_support')
languages=importlib.import_module(PACKAGE+'.language_build')
tool_verifier=importlib.import_module(PACKAGE+'.tools')


def run_phase(config,phase,input_root,work):
    inputs,work=Path(input_root).resolve(strict=True),Path(work).resolve(strict=True)
    if inputs==work or inputs.is_relative_to(work) or work.is_relative_to(inputs):raise ValueError('input/work roots must be disjoint')
    if any(work.iterdir()):raise ValueError('phase work directory must initially be empty')
    if config.get('schema')!='madb.kotlin-rust-backend/1':raise ValueError('unexpected C06 backend config')
    if config['variant']=='swift-kotlin':
        helper=importlib.import_module(PACKAGE+'.skie_build')
        return helper.run_phase(config,phase,inputs,work)
    tool_verifier.verify(config['toolchain'])
    try:
        with tempfile.TemporaryDirectory(prefix='managed-',dir=work) as temporary:
            commands=support.Commands(config,inputs,work,Path(temporary))
            if phase=='compile':outputs=languages.compile_sdk(config,inputs,work,commands)
            elif phase=='link':outputs=languages.link_smoke(config,inputs,work,commands)
            elif phase=='run':outputs=languages.run_smoke(config,inputs,work,commands)
            else:raise ValueError('unknown C06 phase')
        actual={p.relative_to(work).as_posix() for p in work.rglob('*') if p.is_file()}
        if actual!=set(outputs)|{support.LOG_PATH}:raise ValueError('phase output closure differs: '+repr(sorted(actual^set(outputs)^ {support.LOG_PATH})))
        return sorted(outputs)
    finally:tool_verifier.verify(config['toolchain'])


def verify_inputs(task,root):
    for ref in task['inputs']:
        path=support.input_path(root,ref['path'])
        if path.stat().st_size!=ref['size'] or support.sha(path)!=ref['sha256']:raise ValueError('frozen input changed: '+ref['path'])


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--config',required=True,type=Path);args=parser.parse_args()
    try:
        task=support.decode(sys.stdin.buffer.read(support.MAX_JSON+1))
        work=Path.cwd().resolve();inputs=(work.parent/'inputs').resolve(strict=True)
        script=Path(__file__).resolve();config_path=args.config.resolve(strict=True)
        if not script.is_relative_to(inputs) or not config_path.is_relative_to(inputs):raise ValueError('worker/config must be frozen inputs')
        config=support.load(config_path)
        expected=config['tools']['python'].get('invocation_arguments',[])+[os.path.relpath(script,work),'--config',os.path.relpath(config_path,work)]
        if task.get('schema')!='madb.build-task/1' or task.get('arguments')!=expected:raise ValueError('unexpected BuildTask/arguments')
        if task['selector']['target']!=config['rust_target']:raise ValueError('BuildTask target mismatch')
        required={script.relative_to(inputs).as_posix(),config_path.relative_to(inputs).as_posix()}
        if not required<={r['path'] for r in task['inputs']}:raise ValueError('worker/config omitted from declared inputs')
        verify_inputs(task,inputs)
        outputs=run_phase(config,task['phase'],inputs,work)
        verify_inputs(task,inputs)
        if sorted(task['outputs'])!=outputs:raise ValueError('declared output closure mismatch')
        evidence=dict(schema='madb.build-evidence/1',task_id=task['id'],task_digest=task['task_digest'],input_digest=task['input_digest'],
            profile_digest=task['selector']['profile_digest'],generation_id=task['generation_id'],tool=task['tool'],phase=task['phase'],
            target=task['selector']['target'],status='passed',exit_code=0,outputs=[support.file_ref(work/n,work) for n in outputs],
            logs=[support.file_ref(work/support.LOG_PATH,work)])
        sys.stdout.buffer.write(support.canonical(evidence)+b'\n')
    except (KeyError,OSError,RuntimeError,TypeError,ValueError,ImportError) as error:
        sys.stderr.write('C06 build: '+str(error)+'\n');raise SystemExit(2) from error


if __name__=='__main__':main()

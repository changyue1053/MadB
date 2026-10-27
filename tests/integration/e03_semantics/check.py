"""Actual @2 reader/generator Source checks; no SDK, HAR or target execution."""
from copy import deepcopy
from pathlib import Path
import argparse
import importlib.util
import json
import shutil
import sys

sys.dont_write_bytecode = True
import inputs as fixture
from inputs import base, chain, d

_spec = importlib.util.spec_from_file_location('chain_checks', fixture.CHAIN / 'check.py')
checks = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(checks)


def observe_roles(root, run, mode):
    """Inspect the submitted table and real finalized IR independently of host output."""
    composition = base.support.read_json(root / 'composition.json')
    ir = base.support.read_json(run / 'generate-frozen/contract.json')
    rows = composition['responsibilities']
    ids = {s['public_id'] for s in ir['symbols']}
    assert len(ids) == 16
    expected = 107 if mode == 'enhanced' else 95
    assert len(rows) == expected, len(rows)
    keys = [{k: r[k] for k in ('boundary', 'subject', 'stage', 'scope')} for r in rows]
    assert len({json.dumps(k, sort_keys=True) for k in keys}) == expected
    for stage, owner in [('type-read', {'kind': 'component', 'id': fixture.PACKAGE + '/reader'}),
                         ('abi-lowering', {'kind': 'component', 'id': fixture.PACKAGE + '/generator'})]:
        selected = [r for r in rows if r['stage'] == stage]
        assert {r['subject']['public_id'] for r in selected} == ids
        assert all(r['owner'] == owner for r in selected)
    assert not any(r['stage'] == 'physical-assembly' for r in rows)
    assets = {a['id'] for a in composition['assets']}
    assert assets == {'facade', 'native-runtime', 'native-worker', 'native-gateway', 'native-provider'} | ({'state', 'arkui'} if mode == 'enhanced' else set())
    assert len(composition['dependencies']) == (10 if mode == 'enhanced' else 5)
    if mode == 'enhanced':
        assert any(r['subject'] == {'kind': 'sdk-export', 'asset': 'arkui', 'name': 'ArkUIStateModel.snapshot'}
                   and r['stage'] == 'projection' for r in rows)
    return {'symbols': 16, 'declared_roles': expected, 'assets': sorted(assets),
            'dependencies': len(composition['dependencies']), 'physical_assembly': False}


def run(args):
    assert '.'.join(map(str, sys.version_info[:3])) == (fixture.chain.ROOT / '.python-version').read_text().strip()
    output = args.output.absolute()
    protected = [fixture.chain.ROOT.resolve(), *(p.resolve().parent for p in (args.bridge, args.probe, args.peer)), args.observation.resolve().parent]
    if output.exists() or output.is_symlink() or any(output.resolve().is_relative_to(p) or p.is_relative_to(output.resolve()) for p in protected):
        raise ValueError('new output must not overlap source, observations or tool inputs')
    output.mkdir(parents=True)
    before = {n: checks.file_id(getattr(args, n).resolve()) for n in ('bridge', 'probe', 'peer', 'observation')}
    before['python'] = checks.file_id(Path(sys.executable).resolve())
    before['sources'] = checks.inventory(Path(__file__).parent)
    d.save(output, 'inputs-before.json', before)
    records = []
    mode_roots = {}
    for mode in chain.MODES:
        mode_output = output / mode
        mode_output.mkdir()
        root = mode_output / 'inputs'
        tools = fixture.prepare(args, root, mode)
        mode_roots[mode] = (root, tools)
        frozen = checks.inventory(root)
        d.save(mode_output, 'source-before.json', frozen)
        run_root = mode_output / 'run-ok'
        result = checks.command([args.probe, root, tools, run_root, 'ok'], mode_output / 'ok')
        checks.check_run(result, run_root, 'ok')
        observation = observe_roles(root, run_root, mode)
        observed = checks.command([sys.executable, '-I', '-S', '-B', fixture.CHAIN / 'consume_source.py', run_root / 'public-source', mode], mode_output / 'consume')
        assert observed['consumed_payloads'] == (17 if mode == 'enhanced' else 15)
        assert checks.inventory(root) == frozen
        assert not (root / 'contract.json').exists() and not (root / 'generate-request.json').exists()
        records.append({'case': mode + '-two-session-source', 'result': 'passed', **observation, 'payloads': observed['consumed_payloads']})
        d.save(output, 'progress.json', records)
    # The rejection matrix below never supplies the host's required collection.
    if not args.positives_only:
        records.extend(rejections(args, output, mode_roots))
    after = {n: checks.file_id(getattr(args, n).resolve()) for n in ('bridge', 'probe', 'peer', 'observation')}
    after['python'] = checks.file_id(Path(sys.executable).resolve())
    after['sources'] = checks.inventory(Path(__file__).parent)
    d.save(output, 'inputs-after.json', after)
    assert before == after, 'source or image changed during execution'
    d.save(output, 'checks.json', {'checks': records, 'scope': 'finite C07 source semantics @2',
        'not_claimed': ['SDK build', 'HAR', 'target runtime', 'physical assembly', 'replacement backend implementation', 'whole-package acceptance']})
    print(json.dumps({'checks': len(records), 'output': str(output)}))


def metadata_probe(args, root, seed, output, expected):
    """Direct public metadata/semantic boundary negative, not a reader run."""
    import subprocess
    started = checks.now()
    seed_bytes = (json.dumps(seed, sort_keys=True, separators=(',', ':')) + '\n').encode()
    (output.parent / (output.name + '.seed.json')).write_bytes(seed_bytes)
    result = subprocess.run([str(args.bridge), 'metadata', str(root)], input=seed_bytes,
                            capture_output=True, timeout=60, env=base.support.clean_environment())
    output.with_suffix('.stdout').write_bytes(result.stdout)
    output.with_suffix('.stderr').write_bytes(result.stderr)
    d.save(output.parent, output.name + '.execution.json', {'started_at': started, 'finished_at': checks.now(),
        'command': [str(args.bridge), 'metadata', str(root)], 'exit_code': result.returncode,
        'stdin': checks.file_id(output.parent / (output.name + '.seed.json')),
        'stdout': checks.file_id(output.with_suffix('.stdout')), 'stderr': checks.file_id(output.with_suffix('.stderr'))})
    if expected is None:
        assert result.returncode == 0, result.stderr
        return json.loads(result.stdout)
    assert result.returncode != 0 and not result.stdout, (result.returncode, result.stdout, result.stderr)
    for fragment in expected:
        assert fragment in result.stderr.decode(), (expected, result.stderr)
    return None


def rejections(args, output, mode_roots):
    records = []
    missing = ['type-read', 'abi-lowering', 'projection', 'async-bridge', 'dispatch', 'subscription', 'stop', 'drain', 'cancel', 'release']
    cases = [('missing-' + stage, 'ordinary', ('InputInvalid', 'responsibilities', 'missing, extra, or mismatched responsibility')) for stage in missing]
    cases += [(name, mode, fragments) for name, mode, fragments in [
        ('missing-input-release', 'ordinary', ('InputInvalid', 'responsibilities', 'missing, extra, or mismatched responsibility')),
        ('missing-source-retirement', 'ordinary', ('InputInvalid', 'responsibilities', 'missing, extra, or mismatched responsibility')),
        ('missing-residual-release', 'ordinary', ('InputInvalid', 'responsibilities', 'missing, extra, or mismatched responsibility')),
        ('wrong-release-owner', 'ordinary', ('InputInvalid', 'responsibilities', 'missing, extra, or mismatched responsibility')),
        ('wrong-subscription-anchor', 'ordinary', ('InputInvalid', 'responsibilities', 'missing, extra, or mismatched responsibility')),
        ('duplicate-key-other-owner', 'ordinary', ('HASH004', 'duplicate collection identity')),
        ('physical-assembly', 'ordinary', ('InputInvalid', 'responsibilities', 'physical-assembly is outside')),
        ('api22', 'ordinary', ('CapabilityUnsupported', 'deployment_target')),
        ('changed-abi', 'ordinary', ('CapabilityUnsupported', 'abi')),
        ('changed-consumer', 'ordinary', ('CapabilityUnsupported', 'consumer')),
        ('changed-frame-layout', 'ordinary', ('InputInvalid', 'frame-layout.json', 'unsupported finite semantic source bytes')),
        ('changed-validator', 'ordinary', ('InputInvalid', 'validate_snapshot.h', 'unsupported finite semantic source bytes')),
        ('unclaimed-source-asset', 'ordinary', ('InputInvalid', 'assets', 'finite source asset closure differs')),
        ('disabled-optional-source', 'disabled', ('InputInvalid', 'assets', 'disabled source recipe contains optional SDK bytes')),
        ('missing-snapshot', 'enhanced', ('InputInvalid', 'responsibilities', 'missing, extra, or mismatched responsibility')),
        ('weakened-cancel-policy', 'ordinary', ('CapabilityUnsupported', 'policies', 'finite source ownership')),
    ]]
    for name, mode, expected in cases:
        destination = output / name
        destination.mkdir()
        baseline, tools = mode_roots[mode]
        root = destination / 'inputs'
        shutil.copytree(baseline, root)
        composition = base.support.read_json(root / 'composition.json')
        profile = base.support.read_json(root / 'profile.json')
        read = base.support.read_json(root / 'read-request.json')
        rows = composition['responsibilities']
        def remove(predicate):
            index = next(i for i, r in enumerate(rows) if predicate(r))
            rows.pop(index)
        if name in ['missing-' + stage for stage in missing]:
            remove(lambda r: r['stage'] == name.removeprefix('missing-'))
        elif name == 'missing-input-release':
            remove(lambda r: r['scope']['layer'] == 'input-payload')
        elif name == 'missing-source-retirement':
            remove(lambda r: r['scope']['layer'] == 'source-ticket')
        elif name == 'missing-residual-release':
            remove(lambda r: r['scope']['layer'] == 'residual-payload')
        elif name == 'missing-snapshot':
            remove(lambda r: r['subject'].get('name') == 'ArkUIStateModel.snapshot')
        elif name == 'wrong-release-owner':
            next(r for r in rows if r['scope']['layer'] == 'input-payload')['owner']['asset'] = 'native-runtime'
        elif name == 'wrong-subscription-anchor':
            row = next(r for r in rows if r['stage'] == 'drain' and r['scope']['kind'] == 'subscription')
            row['scope']['anchor'] = {'kind': 'public-symbol', 'public_id': 'madb.c07.Runtime.connect.v1'}
        elif name == 'duplicate-key-other-owner':
            row = deepcopy(rows[0])
            row['owner'] = {'kind': 'sdk-asset', 'package': fixture.PACKAGE, 'asset': 'facade'}
            rows.append(row)
        elif name == 'physical-assembly':
            row = deepcopy(rows[0]); row['stage'] = 'physical-assembly'; rows.append(row)
        elif name == 'api22': profile['deployment_target'] = 'API22'
        elif name == 'changed-abi': profile['abi'] = 'unsupported-abi'
        elif name == 'changed-consumer': profile['consumer']['runtime'] = 'other'
        elif name in ('changed-frame-layout', 'changed-validator'):
            path = 'sdk/arkts/facade/frame-layout.json' if name == 'changed-frame-layout' else 'sdk/arkts/native-runtime/include/validate_snapshot.h'
            data = (root / path).read_bytes()
            if name == 'changed-frame-layout':
                frame = json.loads(data)
                # Real enum values remain contiguous and unique, but move the
                # receipt/join meaning away from the fixed RuntimeFrame consumer.
                a = next(k for k, v in frame.items() if v == 'WorkerJoined')
                b = next(k for k, v in frame.items() if v == 'ReceiptConsumed')
                frame[a], frame[b] = frame[b], frame[a]
                data = (json.dumps(frame, sort_keys=True) + '\n').encode()
            else:
                assert b'1024' in data
                data = data.replace(b'1024', b'2048')
            (root / path).write_bytes(data)
            (root / base.PREFIX / path).write_bytes(data)
        elif name == 'unclaimed-source-asset':
            path = 'sdk/arkts/native-runtime/include/validate_snapshot.h'
            composition['assets'].append({'id': 'extra-source', 'package': fixture.PACKAGE,
                'source': {'kind': 'source', 'files': [d.ref(root / base.PREFIX, path)]}})
        elif name == 'disabled-optional-source':
            path = 'sdk/arkts/observation/StateObservation.ets'
            for prefix in ('', base.PREFIX):
                dest = root / prefix / path
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(chain.ROOT / path, dest)
                read['inputs'].append(d.ref(root, str(Path(prefix) / path), 'madb.source-input/1'))
            composition['packages'][0]['source']['files'].append(d.ref(root / base.PREFIX, path))
        elif name == 'weakened-cancel-policy':
            project = base.support.read_json(root / 'project.json')
            policy = next(p for p in project['policies'] if p['selector'].get('id') == 'madb.c07.Operation.requestCancel.v1' and p['slot']['kind'] == 'call')
            policy['values']['cancellation'] = 'none'
            policy['values']['cancellation_confirmation'] = 'not-supported'
            d.save(root, 'project.json', project)
        else: raise AssertionError(name)
        composition['packages'][0]['source']['files'].sort(key=lambda f: f['path'])
        read['inputs'].sort(key=lambda f: f['path'])
        d.save(root, 'composition.json', composition)
        d.save(root, 'profile.json', profile)
        if name == 'duplicate-key-other-owner':
            frozen = checks.inventory(root)
            checks.command([args.bridge, 'composition-digest', root], destination / 'rejection', expected_error='HASH004')
            raw = (destination / 'rejection.stderr').read_text()
            assert all(fragment in raw for fragment in expected), raw
            assert checks.inventory(root) == frozen
            records.append({'case': name, 'result': 'passed', 'scope': 'duplicate semantic key rejected before a new composition digest can be pinned', 'expected': list(expected)})
            d.save(output, 'rejection-progress.json', records)
            continue
        read = chain.repin(root, args.bridge, read)
        seed = base.support.read_json(baseline.parent / 'run-ok/generate-frozen/contract.json')
        seed['selector'] = read['selector']
        seed['source_digest'] = d.semantic('madb.source/1', base.support.read_json(root / 'lock.json')['inputs'])
        seed['policies'] = []
        frozen = checks.inventory(root)
        metadata_probe(args, root, seed, destination / 'rejection', expected)
        assert checks.inventory(root) == frozen
        assert not (destination / 'public-source').exists()
        records.append({'case': name, 'result': 'passed', 'scope': 'coherently repinned metadata/semantic boundary rejection', 'expected': list(expected)})
        d.save(output, 'rejection-progress.json', records)
        if name in ('missing-release', 'missing-snapshot'):
            # Two actual public reader failures demonstrate that the pipeline
            # cannot publish Source or start the generator after semantic refusal.
            run_root = destination / 'run'
            result = checks.command([args.probe, root, tools, run_root, 'ok'], destination / 'reader')
            checks.check_run(result, run_root, 'reader-business-failure')
            assert checks.inventory(root) == frozen
            records.append({'case': name + '-reader-no-commit', 'result': 'passed', 'scope': 'real AuthorSession reader refusal; no generator or Source commit'})
    return records

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('output', 'bridge', 'probe', 'peer', 'observation'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--positives-only', action='store_true', help='explicit development subset; not a complete matrix')
    run(parser.parse_args())

"""Materialize the synthetic C02 contract with real, independently hashed files.

This is test input, not output from a compiler reader or a usable native SDK.
The ASCII-only fixture permits Python's JSON ordering as a JCS oracle.
"""
from copy import deepcopy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / 'tests/fixtures/contracts'


def raw(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':')).encode()


def semantic(domain, value):
    return raw(domain.encode() + b'\n' + canonical(value))


def save(root, name, value):
    path = root / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def ref(root, name, schema=None):
    data = (root / name).read_bytes()
    result = dict(path=name, sha256=raw(data), size=len(data))
    if schema:
        result['schema'] = schema
    return result


def project_digest(project):
    value = deepcopy(project)
    value.pop('extensions', None)
    value['boundaries'].sort(key=lambda v: v['id'])
    value['policy_families'].sort(key=lambda v: v['id'])
    for family in value['policy_families']:
        family.pop('origin', None)
    for policy in value['policies']:
        policy.pop('origin', None)
        policy['families'].sort()
    value['policies'].sort(key=canonical)
    return semantic('madb.project/1', value)


def task_digest(task):
    value = deepcopy(task)
    value.pop('task_digest')
    for name, key in [('inputs', 'path'), ('environment', 'name')]:
        value[name].sort(key=lambda v: v[key])
    for name in ['dependencies', 'outputs']:
        value[name].sort()
    return semantic('madb.task/1', value)


def materialize(root):
    values = {name: json.loads((FIXTURES / f'{name}.json').read_text())
              for name in ['project', 'profile', 'lock', 'read-request', 'contract-ir', 'build-task']}
    (root / 'native').mkdir()
    (root / 'native/fixture.rs').write_bytes(b'pub fn fixture() {}\n')
    (root / 'schemas').mkdir()
    (root / 'schemas/common.schema.json').write_bytes((ROOT / 'docs/schemas/common.schema.json').read_bytes())
    save(root, 'profiles/rust-c.json', values['profile'])
    profile = ref(root, 'profiles/rust-c.json', 'madb.profile/1')
    values['project']['boundaries'][0]['profile'] = profile
    values['lock']['profiles'][0]['file'] = profile
    values['lock']['schemas'] = [ref(root, 'schemas/common.schema.json')]
    values['lock']['project_digest'] = project_digest(values['project'])
    save(root, 'project.json', values['project'])
    save(root, 'madb.lock', values['lock'])
    values['read-request']['lock'] = ref(root, 'madb.lock', 'madb.lock/1')
    values['read-request']['profile'] = profile
    save(root, 'read.json', values['read-request'])
    save(root, 'contract.json', values['contract-ir'])
    for name, phase, dependencies in [('metadata', 'metadata', []), ('generate', 'generate', ['metadata']), ('compile', 'compile', ['generate'])]:
        task = deepcopy(values['build-task'])
        task.update(id=name, phase=phase, dependencies=dependencies, outputs=[name + '.out'])
        task['task_digest'] = task_digest(task)
        save(root, name + '.json', task)
    return values

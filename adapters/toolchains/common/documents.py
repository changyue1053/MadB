"""Bounded public protocol projections; Rust validates every submitted document."""
from copy import deepcopy
import hashlib
import json
from pathlib import Path


def canonical(value):
    def normalized(item):
        if isinstance(item, dict):
            return {k: normalized(item[k]) for k in sorted(item, key=lambda k: k.encode('utf-16-be'))}
        if isinstance(item, list):
            return [normalized(v) for v in item]
        if isinstance(item, float) or isinstance(item, int) and not isinstance(item, bool) and abs(item) > 9007199254740991:
            raise ValueError('exact wire values must use the declared typed representation')
        return item
    return json.dumps(normalized(value), ensure_ascii=False, separators=(',', ':'), allow_nan=False)


def semantic(domain, value):
    return 'sha256:' + hashlib.sha256((domain + '\n' + canonical(value)).encode()).hexdigest()


def sha_file(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(block)
    return 'sha256:' + digest.hexdigest()


def save(root, name, value):
    path = Path(root) / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(canonical(value) + '\n')
    return path


def ref(root, name, schema=None):
    path = Path(root) / name
    result = dict(path=name, sha256=sha_file(path), size=path.stat().st_size)
    if schema:
        result['schema'] = schema
    return result


def profile_digest(value):
    value = deepcopy(value)
    value.pop('extensions', None)
    value['defaults'] = value['defaults']['values']
    for name, key in [('tools', 'id'), ('macros', 'name'), ('restrictions', 'feature')]:
        value[name].sort(key=lambda x: x[key])
    for name in ['required_features', 'supported_features']:
        value[name].sort()
    return semantic('madb.profile/1', value)


def project_digest(value):
    value = deepcopy(value)
    value.pop('extensions', None)
    value['boundaries'].sort(key=lambda x: x['id'])
    value['policy_families'].sort(key=lambda x: x['id'])
    for family in value['policy_families']:
        family.pop('origin', None)
    for policy in value['policies']:
        policy.pop('origin', None)
        policy['families'].sort()
    value['policies'].sort(key=canonical)
    return semantic('madb.project/1', value)


def task_digest(value):
    value = deepcopy(value)
    value.pop('task_digest', None)
    for name, key in [('inputs', 'path'), ('environment', 'name')]:
        value[name].sort(key=lambda x: x[key])
    for name in ['dependencies', 'outputs']:
        value[name].sort()
    return semantic('madb.task/1', value)


def generation_task(request, recipe):
    request = deepcopy(request)
    request.pop('task_digest', None)
    request['inputs'].sort(key=lambda x: x['path'])
    execution = {key: deepcopy(recipe[key]) for key in ['id', 'tool', 'arguments', 'environment', 'working_directory', 'response_path', 'inputs']}
    execution['environment'].sort(key=lambda x: x['name'])
    execution['inputs'].sort(key=lambda x: x['path'])
    return semantic('madb.task/1', dict(operation='generate', request=request, execution=execution))


def unique(references):
    found = {}
    for item in references:
        key = item['path']
        if key in found and found[key] != item:
            # Schema tags describe a port reference, not different content.
            if any(found[key][k] != item[k] for k in ('sha256', 'size')):
                raise ValueError('conflicting frozen input: ' + key)
            if 'schema' not in found[key] and 'schema' in item:
                found[key] = item
        else:
            found[key] = item
    return [found[k] for k in sorted(found)]

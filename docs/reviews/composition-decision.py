#!/usr/bin/env python3
"""Design specimens only; no production schema/registry/acceptance authority.

--write generates the companion specimen/receipt JSON; default revalidates it.
Only the companion .json is writable. Real SDK files are read and byte pinned.
"""
from __future__ import annotations
import argparse
from copy import deepcopy
import hashlib
import json
import jsonschema
import re
from pathlib import Path
import subprocess

ROOT = Path('/private/tmp/madb-chief-round3/madb-baseline')
OUT = Path(__file__).with_suffix('.json')
SNAPSHOT = Path('/private/tmp/madb-chief-round3/madb-input.json')
FEATURE = 'core.extension-composition@1'  # NEW PROPOSAL, absent from CORE_FEATURES.
PACKAGE = 'madb.arkts-native'  # NEW PROPOSAL, not registered.
VERSION = '0.1.0-design'
POLICY = 'adapters/generators/arkts/profiles/snapshot-v1.json'
FACADE = 'sdk/arkts/facade/Boundary.ets.in'
STATE = 'sdk/arkts/observation/StateObservation.ets'
UI = 'sdk/arkts/observation/ArkUIStateModel.ets'
GEN = 'adapters/generators/arkts/driver.py'
PATHS = [POLICY, FACADE, STATE, UI, GEN,
 'sdk/arkts/facade/RuntimeFrame.ets', 'sdk/arkts/native-runtime/src/protocol.cpp',
 'sdk/arkts/native-gateway/src/runtime.cpp', 'sdk/arkts/native-gateway/src/api.cpp', 'sdk/c/arkts-provider/src/provider.c',
 'adapters/toolchains/arkts/native.py', 'adapters/toolchains/arkts/project.py',
 'adapters/contracts/src/projections.rs', 'docs/adr/0013-arkts-runtime-and-completion.md',
 'docs/adr/0017-arkts-extension-delivery.md', 'crates/madb-model/src/contract.rs',
 'docs/schemas/extension-manifest.schema.json']
MANIFEST_PATH = 'extensions/arkts-native/extension.design.json'
PUBLIC = {
 'roundtrip': 'madb.c07.Engine.roundtrip.v1',
 'observe': 'madb.c07.Engine.observeState.v1',
 'stop': 'madb.c07.Subscription.requestStop.v1',
 'close': 'madb.c07.Subscription.close.v1',
}

def canonical(value):
    # Exact for these deliberately ASCII-key/string + safe-integer specimens.
    # This is NOT a replacement implementation of all RFC 8785/JCS inputs.
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False)

def byte_digest(data):
    return 'sha256:' + hashlib.sha256(data).hexdigest()

def semantic(domain, value):
    return byte_digest((domain + '\n' + canonical(value)).encode())

def ref(path, data, schema=None):
    result = {'path': path, 'sha256': byte_digest(data), 'size': len(data)}
    if schema is not None:
        result['schema'] = schema
    return result

def check(ok, code):
    if not ok:
        raise ValueError(code)

def exact(value, keys, code):
    check(isinstance(value, dict) and set(value) == set(keys.split()), code)

def safe_path(value):
    check(isinstance(value, str) and value and '\\' not in value and ':' not in value
          and all(x not in ('', '.', '..') for x in value.split('/')), 'PathRejected')

def pins(root):
    captured = json.loads(SNAPSHOT.read_text())
    result = {}
    for path in PATHS:
        data = (root / path).read_bytes()
        check(byte_digest(data) == 'sha256:' + captured['files'][path], 'SourceDrift:' + path)
        result[path] = ref(path, data)
    return result

def subject(public_id):
    return {'kind': 'public-symbol', 'public_id': public_id}

def export(asset, name):
    return {'kind': 'sdk-export', 'asset': asset, 'name': name}

def row(s, stage, kind, anchor, layer, owner):
    return {'boundary': 'c-arkts', 'subject': s, 'stage': stage,
            'scope': {'kind': kind, 'anchor': anchor, 'layer': layer},
            'owner': {'kind': 'sdk-asset', 'package': PACKAGE, 'asset': owner}}

def required(mode, policy):
    """Independent finite rule fixture; never consumes candidate responsibilities.

    Production must consume compiler-checked ContractIR + reviewed SDK binding
    facts. Here the existing policy/sources are pinned; no compiler proof claimed.
    """
    calls = policy['calls']
    check(calls['Engine.roundtrip']['kind'] == 'sync', 'PolicyRoundtrip')
    check(calls['Engine.observeState']['kind'] == 'stream', 'PolicyStream')
    for key, name in [('roundtrip', 'Engine.roundtrip'), ('observe', 'Engine.observeState'),
                      ('stop', 'Subscription.requestStop'), ('close', 'Subscription.close')]:
        check(calls[name]['public_id'] == PUBLIC[key], 'PolicyPublicIdentity')
    origin = subject(PUBLIC['observe'])
    rows = [
        row(subject(PUBLIC['roundtrip']), 'projection', 'call', subject(PUBLIC['roundtrip']), 'facade', 'facade'),
        row(origin, 'subscription', 'subscription', origin, 'source', 'native-protocol'),
        row(subject(PUBLIC['stop']), 'stop', 'subscription', origin, 'source', 'native-protocol'),
        row(subject(PUBLIC['close']), 'drain', 'subscription', origin, 'source', 'native-protocol'),
        row(subject(PUBLIC['close']), 'release', 'subscription', origin, 'facade', 'facade'),
    ]
    if mode == 'enhanced':
        anchor = export('state-observation', 'StateObservation.attach')
        rows += [row(export('state-observation', 'StateObservation.requestStop'), 'stop',
                     'receiver', anchor, 'receiver', 'state-observation'),
                 row(export('state-observation', 'StateObservation.close'), 'async-bridge',
                     'receiver', anchor, 'receiver', 'state-observation'),
                 row(export('arkui-state', 'ArkUIStateModel.attach'), 'projection',
                     'receiver', export('arkui-state', 'ArkUIStateModel.attach'), 'view-model', 'arkui-state')]
    return rows

def project(c):
    v = deepcopy(c)
    for name in ('packages', 'components', 'assets'):
        v[name].sort(key=lambda x: x['id'])
    for p in v['packages']:
        p['source']['files'].sort(key=lambda f: f['path'])
    for a in v['assets']:
        if a['source']['kind'] == 'source':
            a['source']['files'].sort(key=lambda f: f['path'])
        else:
            a['source']['paths'].sort()
    v['dependencies'].sort(key=lambda r: (r['from'], r['to'], r['scope']))
    v['responsibilities'].sort(key=lambda r: (r['boundary'], canonical(r['subject']), r['stage'], canonical(r['scope'])))
    return v

def make(mode, source_pins, policy):
    manifest = {'schema': 'madb.extension/1', 'example': True, 'id': PACKAGE, 'version': VERSION,
        'compatibility': {'protocol': ['1.0'], 'ir': ['1.0']}, 'languages': [],
        'components': [{'id': name, 'port': port, 'supported_features': [FEATURE],
                        'entrypoint': {'kind': 'process', 'path': GEN, 'args': [arg]}}
                       for name, port, arg in [('reader', 'contract-reader', 'metadata'), ('generator', 'generator', 'generate')]],
        'dependencies': [], 'routes': [],
        'assets': [{'kind': 'sdk', 'path': p} for p in ['sdk/arkts/facade', 'sdk/arkts/native-runtime',
                    'sdk/arkts/native-gateway', 'sdk/c/arkts-provider', 'sdk/arkts/observation']],
        'execution': {'transport': 'stdio-jsonl', 'network': 'none', 'env': []}}
    manifest_bytes = (canonical(manifest) + '\n').encode()
    manifest_ref = ref(MANIFEST_PATH, manifest_bytes, 'madb.extension/1')
    source_files = [source_pins[p] for p in PATHS if p.startswith(('sdk/', 'adapters/generators/'))]
    source_files += [{k: manifest_ref[k] for k in ('path', 'sha256', 'size')}]
    source = {'kind': 'file-set', 'files': sorted(source_files, key=lambda f: f['path'])}
    package_digest = semantic('madb.source/1', source['files'])
    package = {'id': PACKAGE, 'version': VERSION, 'manifest': manifest_ref,
               'source': source, 'source_digest': package_digest}
    components = [{'id': PACKAGE + '/' + c['id'], 'version': VERSION,
        'sha256': package_digest}
        for c in manifest['components']]
    def asset(name, path):
        return {'id': name, 'package': PACKAGE, 'source': {'kind': 'source', 'files': [source_pins[path]]}}
    assets = [asset('facade-template', FACADE), asset('native-protocol', 'sdk/arkts/native-runtime/src/protocol.cpp'),
              asset('native-gateway', 'sdk/arkts/native-gateway/src/runtime.cpp')]
    assets.append({'id': 'facade', 'package': PACKAGE, 'source': {'kind': 'generated',
        'producer': PACKAGE + '/generator', 'output_slot': 'ordinary-facade',
        'paths': ['generated/facade/Boundary.ets']}})
    deps = []
    if mode == 'enhanced':
        assets += [asset('state-observation', STATE), asset('arkui-state', UI)]
        deps += [{'from': 'state-observation', 'to': 'facade', 'scope': 'consumer-compile'},
                 {'from': 'arkui-state', 'to': 'state-observation', 'scope': 'optional-integration'}]
    # Disabled resolves to the identical ordinary recipe, not a hidden disabled flag.
    c = {'schema': 'madb.composition/1', 'id': 'c07-c-arkts-' + ('enhanced' if mode == 'enhanced' else 'ordinary'),
         'recipe': 'madb.c07-c-arkts/' + ('arkui-v2' if mode == 'enhanced' else 'ordinary') + '@1',
         'selection': {'profile_id': 'proposal-c07-c-arkts-' + ('enhanced' if mode == 'enhanced' else 'ordinary'),
            'route': 'madb.arkts-c-napi', 'direction': 'consumer-to-provider', 'target': 'aarch64-linux-ohos'},
         'packages': [package], 'components': components, 'assets': assets,
         'dependencies': deps, 'responsibilities': required(mode, policy)}
    return c, manifest, manifest_bytes

def validate(c, source_pins, policy, virtual):
    exact(c, 'schema id recipe selection packages components assets dependencies responsibilities', 'UnknownCompositionField')
    check(c['schema'] == 'madb.composition/1', 'UnknownCompositionVersion')
    check(c['recipe'] in ['madb.c07-c-arkts/ordinary@1', 'madb.c07-c-arkts/arkui-v2@1'], 'UnknownRecipe')
    mode = 'enhanced' if 'arkui-v2' in c['recipe'] else 'ordinary'
    exact(c['selection'], 'profile_id route direction target', 'UnknownSelectionField')
    expected_selection = {'profile_id': 'proposal-c07-c-arkts-' + ('enhanced' if mode == 'enhanced' else 'ordinary'), 'route': 'madb.arkts-c-napi', 'direction': 'consumer-to-provider', 'target': 'aarch64-linux-ohos'}
    check(c['selection'] == expected_selection, 'SelectionMismatch')
    check(c['id'] == 'c07-c-arkts-' + ('enhanced' if mode == 'enhanced' else 'ordinary'), 'CompositionIdMismatch')
    check(len(c['packages']) == 1 and c['packages'][0]['id'] == PACKAGE, 'PackageMismatch')
    package = c['packages'][0]
    exact(package, 'id version manifest source source_digest', 'UnknownPackageField')
    exact(package['manifest'], 'path sha256 size schema', 'UnknownManifestReferenceField')
    exact(package['source'], 'kind files', 'UnknownSourceField')
    inventory = package['source']['files']
    expected_paths = {p for p in PATHS if p.startswith(('sdk/', 'adapters/generators/'))} | {MANIFEST_PATH}
    check(package['source']['kind'] == 'file-set', 'UnknownSourceKind')
    check(len({f['path'] for f in inventory}) == len(inventory), 'DuplicateSource')
    for f in inventory:
        exact(f, 'path sha256 size', 'UnknownSourceFileField')
        safe_path(f['path'])
        check(f['path'] in source_pins or f['path'] in virtual, 'UnboundSourceOrCycle')
        expected = source_pins.get(f['path']) or ref(f['path'], virtual[f['path']])
        check(f == expected, 'SourceBytesMismatch')
    check({f['path'] for f in inventory} == expected_paths, 'PackageFileSetMismatch')
    expected_source = {'kind': 'file-set', 'files': sorted(inventory, key=lambda f: f['path'])}
    check(package['source_digest'] == semantic('madb.source/1', expected_source['files']), 'PackageDigestMismatch')
    mb = virtual[package['manifest']['path']]
    check(package['manifest'] == ref(MANIFEST_PATH, mb, 'madb.extension/1'), 'ManifestDigestMismatch')
    manifest = json.loads(mb)
    try:
        jsonschema.Draft202012Validator(json.loads((ROOT / 'docs/schemas/extension-manifest.schema.json').read_text())).validate(manifest)
    except jsonschema.ValidationError as error:
        raise ValueError('ManifestSchemaRejected') from error
    check(manifest['id'] == package['id'] and manifest['version'] == package['version'], 'ManifestIdentityMismatch')
    check(len({x['id'] for x in c['components']}) == len(c['components']), 'DuplicateComponent')
    check({x['id'] for x in c['components']} == {PACKAGE+'/reader',PACKAGE+'/generator'}, 'ComponentClosureMismatch')
    for x in c['components']:
        exact(x, 'id version sha256', 'UnknownComponentField')
        candidates = [m for m in manifest['components'] if PACKAGE + '/' + m['id'] == x['id']]
        check(len(candidates) == 1, 'UnknownComponent')
        expected_port = {PACKAGE+'/reader':'contract-reader', PACKAGE+'/generator':'generator'}[x['id']]
        check(candidates[0]['port'] == expected_port, 'ComponentPortMismatch')
        check(FEATURE in candidates[0]['supported_features'], 'ComponentMissingRequiredFeature')
        expected = package['source_digest']
        check(x['sha256'] == expected and x['version'] == package['version'], 'ComponentDigestMismatch')
    allowed_assets = {'facade-template', 'facade', 'native-protocol', 'native-gateway'}
    if mode == 'enhanced': allowed_assets |= {'state-observation', 'arkui-state'}
    check(len(c['assets']) == len(allowed_assets) and {a['id'] for a in c['assets']} == allowed_assets, 'SelectedAssetClosure')
    origins = []
    for a in c['assets']:
        exact(a, 'id package source', 'UnknownAssetField')
        so = a['source']
        origins.append(canonical(so))
    check(len(origins) == len(set(origins)), 'DuplicateAssetOrigin')
    bound_files = {'facade-template': FACADE, 'native-protocol': 'sdk/arkts/native-runtime/src/protocol.cpp', 'native-gateway': 'sdk/arkts/native-gateway/src/runtime.cpp', 'state-observation': STATE, 'arkui-state': UI}
    for a in c['assets']:
        check(a['package'] == PACKAGE, 'AssetPackageMismatch')
        s = a['source']
        if s['kind'] == 'source':
            exact(s, 'kind files', 'UnknownSourceAssetField')
            for f in s['files']:
                check(f in inventory, 'AssetNotInPackage')
            check(a['id'] in bound_files and s['files'] == [source_pins[bound_files[a['id']]]], 'AssetRoleMismatch')
        else:
            exact(s, 'kind producer output_slot paths', 'GeneratedAssetHasPrematureHash')
            check(s['kind'] == 'generated' and s['producer'] == PACKAGE + '/generator', 'WrongProducer')
            selected = [x for x in c['components'] if x['id'] == s['producer']]
            check(len(selected) == 1, 'ProducerNotSelected')
            declared = [m for m in manifest['components'] if PACKAGE + '/' + m['id'] == s['producer']]
            check(len(declared) == 1 and declared[0]['port'] == 'generator', 'WrongProducerPort')
            check(s['output_slot'] == 'ordinary-facade' and s['paths'] == ['generated/facade/Boundary.ets'], 'WrongOutputSlot')
    deps = [] if mode == 'ordinary' else [{'from': 'state-observation', 'to': 'facade', 'scope': 'consumer-compile'}, {'from': 'arkui-state', 'to': 'state-observation', 'scope': 'optional-integration'}]
    check(len(c['dependencies']) == len(deps) and {canonical(d) for d in c['dependencies']} == {canonical(d) for d in deps}, 'DependencyClosureMismatch')
    actual = [canonical(r) for r in c['responsibilities']]
    check(len(actual) == len(set(actual)), 'DuplicateResponsibility')
    check(set(actual) == {canonical(r) for r in required(mode, policy)}, 'RequiredResponsibilityMismatch')
    return semantic('madb.composition/1', project(c))

def profile_projection(composition_digest):
    # This is ONLY the new field projection; old /1 fields are not reimplemented.
    return {'id': 'c07-c-arkts-ordinary', 'semantic_digest': composition_digest}

def run(write=False):
    source_pins = pins(ROOT)
    policy = json.loads((ROOT / POLICY).read_text())
    examples, manifests, virtual = {}, {}, {}
    for mode in ('ordinary', 'enhanced', 'disabled'):
        c, m, data = make(mode, source_pins, policy)
        virtual[MANIFEST_PATH] = data
        examples[mode] = c
        manifests[MANIFEST_PATH] = m
    if not write:
        saved = json.loads(OUT.read_text())
        check(saved['source_pins'] == source_pins, 'SavedSourceDrift')
        examples = saved['examples']
    outcomes = []
    def positive(name, f):
        value = f(); outcomes.append({'name': name, 'result': 'passed'}); return value
    def negative(name, edit, error, change_manifest=None):
        c = deepcopy(examples['enhanced']); edited_virtual = deepcopy(virtual); edit(c)
        if change_manifest is not None:
            m = json.loads(edited_virtual[MANIFEST_PATH]); change_manifest(m)
            data = (canonical(m) + '\n').encode(); edited_virtual[MANIFEST_PATH] = data
            package = c['packages'][0]
            package['manifest'] = ref(MANIFEST_PATH, data, 'madb.extension/1')
            package['source']['files'] = [f if f['path'] != MANIFEST_PATH else ref(MANIFEST_PATH,data) for f in package['source']['files']]
            package['source_digest'] = semantic('madb.source/1', sorted(package['source']['files'],key=lambda f:f['path']))
            for item in c['components']: item['sha256'] = package['source_digest']
        try: validate(c, source_pins, policy, edited_virtual)
        except ValueError as exc:
            check(str(exc) == error, name + ':wrong-rejection:' + str(exc))
            outcomes.append({'name': name, 'result': 'rejected', 'code': str(exc)})
        else: raise AssertionError(name + ':incorrectly-accepted')
    digests = {m: positive(m, lambda c=c: validate(c, source_pins, policy, virtual)) for m, c in examples.items()}
    positive('enhanced-different-identity', lambda: check(digests['ordinary'] != digests['enhanced'], 'EnhancedDigestSame'))
    positive('disabled-identical-ordinary', lambda: check(examples['disabled'] == examples['ordinary'], 'DisabledDiff'))
    shuffled = deepcopy(examples['enhanced'])
    for k in ('assets', 'components', 'responsibilities', 'dependencies'): shuffled[k].reverse()
    shuffled['packages'][0]['source']['files'].reverse()
    positive('set-order-invariant', lambda: check(validate(shuffled, source_pins, policy, virtual) == digests['enhanced'], 'OrderDigest'))
    negative('unknown-version', lambda c: c.update(schema='madb.composition/2'), 'UnknownCompositionVersion')
    negative('unknown-field', lambda c: c.update(profile_digest='sha256:' + '0'*64), 'UnknownCompositionField')
    negative('old-aggregate-component-as-new-port', lambda c: c['components'][0].update(id='org.madb.arkts/reader-generator'), 'ComponentClosureMismatch')
    negative('manifest-hash-as-component-hash', lambda c: c['components'][0].update(sha256=c['packages'][0]['manifest']['sha256']), 'ComponentDigestMismatch')
    negative('wrong-package-byte', lambda c: c['packages'][0]['source']['files'][0].update(sha256='sha256:' + '0'*64), 'SourceBytesMismatch')
    negative('profile-byte-cycle', lambda c: c['packages'][0]['source']['files'].append(ref('profile.json', b'cycle')), 'UnboundSourceOrCycle')
    negative('generated-precomputed-outputset', lambda c: c['assets'][3]['source'].update(output_set=ref('outputs.json', b'cycle')), 'GeneratedAssetHasPrematureHash')
    negative('wrong-producer', lambda c: c['assets'][3]['source'].update(producer=PACKAGE + '/reader'), 'WrongProducer')
    negative('generator-not-selected-empty-components', lambda c: c.update(components=[]), 'ComponentClosureMismatch')
    negative('generator-not-selected-one-component', lambda c: c['components'].pop(), 'ComponentClosureMismatch')
    negative('repinned-generator-declares-toolchain-port', lambda c: None, 'ComponentPortMismatch',
             lambda m: m['components'][1].update(port='toolchain'))
    negative('repinned-reader-declares-toolchain-port', lambda c: None, 'ComponentPortMismatch',
             lambda m: m['components'][0].update(port='toolchain'))
    negative('repinned-generator-drops-composition-feature', lambda c: None, 'ComponentMissingRequiredFeature',
             lambda m: m['components'][1].update(supported_features=[]))
    negative('repinned-reader-drops-composition-feature', lambda c: None, 'ComponentMissingRequiredFeature',
             lambda m: m['components'][0].update(supported_features=[]))
    negative('fixed-recipe-wrong-direction', lambda c: c['selection'].update(direction='host-callback'), 'SelectionMismatch')
    negative('fixed-recipe-unrelated-profile', lambda c: c['selection'].update(profile_id='unrelated-profile'), 'SelectionMismatch')
    negative('unknown-asset-field', lambda c: c['assets'][0].update(unknown='value'), 'UnknownAssetField')
    negative('unknown-package-field', lambda c: c['packages'][0].update(unknown='value'), 'UnknownPackageField')
    negative('unknown-component-field', lambda c: c['components'][0].update(unknown='value'), 'UnknownComponentField')
    negative('unknown-selection-field', lambda c: c['selection'].update(unknown='value'), 'UnknownSelectionField')
    negative('two-asset-ids-one-origin', lambda c: c['assets'][2].update(source=deepcopy(c['assets'][1]['source'])), 'DuplicateAssetOrigin')
    negative('missing-enhanced-dependency', lambda c: c['dependencies'].pop(), 'DependencyClosureMismatch')
    negative('runtime-ref-falsely-assigned-to-subscription', lambda c: c['responsibilities'][4].update(scope={'kind':'subscription','anchor':subject(PUBLIC['observe']),'layer':'environment'}, owner={'kind':'sdk-asset','package':PACKAGE,'asset':'native-gateway'}), 'RequiredResponsibilityMismatch')
    negative('missing-drain', lambda c: c['responsibilities'].pop(3), 'RequiredResponsibilityMismatch')
    negative('scope-rename-hides-double-owner', lambda c: c['responsibilities'].append({**deepcopy(c['responsibilities'][3]), 'scope': {'kind':'subscription','anchor':subject(PUBLIC['observe']),'layer':'invented-source'}}), 'RequiredResponsibilityMismatch')
    negative('sdk-takes-native-drain', lambda c: c['responsibilities'][3]['owner'].update(asset='state-observation'), 'RequiredResponsibilityMismatch')
    negative('sync-call-gets-async-owner', lambda c: c['responsibilities'].append(row(subject(PUBLIC['roundtrip']),'async-bridge','call',subject(PUBLIC['roundtrip']),'facade','facade')), 'RequiredResponsibilityMismatch')
    negative('disabled-with-hidden-ui', lambda c: c.update(recipe=examples['ordinary']['recipe'], id=examples['ordinary']['id'], selection=deepcopy(examples['ordinary']['selection'])), 'SelectedAssetClosure')
    def qualify(package, file):
        check(re.fullmatch(r'[a-z][a-z0-9.-]*[.][a-z0-9][a-z0-9.-]*', package['id']) is not None, 'InvalidPackagePathId')
        check(re.fullmatch(r'(0|[1-9][0-9]*)[.](0|[1-9][0-9]*)[.](0|[1-9][0-9]*)(-[A-Za-z0-9.-]+)?', package['version']) is not None, 'InvalidPackageVersion')
        safe_path(file['path'])
        return {**file, 'path':'packages/'+package['id']+'/'+package['version']+'/'+file['path']}
    same_file = ref('src/lib.rs', b'example')
    qa = qualify({'id':PACKAGE,'version':VERSION}, same_file)
    qb = qualify({'id':'example.other','version':VERSION}, same_file)
    positive('package-relative-paths-have-distinct-lock-keys', lambda: check(qa['path'] != qb['path'] and qa['sha256'] == qb['sha256'], 'LockPathCollision'))
    # Declared-role dependency graph, not arbitrary JSON/key crawling.
    graph = {'composition': ['source-package'], 'profile': ['composition', 'sdk-input', 'backend-input'],
             'lock': ['profile', 'source-package'], 'contract': ['profile', 'lock'],
             'output-set': ['profile', 'contract', 'lock']}
    def acyclic(edges):
        remaining = {k: set(v) for k, v in edges.items()}
        done = set().union(*remaining.values()) - remaining.keys()
        while remaining:
            ready = [k for k, v in remaining.items() if v <= done]
            check(bool(ready), 'IdentityCycle')
            for k in ready: done.add(k); del remaining[k]
    positive('identity-before-full-semantic-admission-dag', lambda: acyclic(graph))
    for label, frm, to in [('composition-backref-output', 'composition', 'output-set'),
                           ('profile-sdk-backref-output', 'sdk-input', 'output-set'),
                           ('source-lock-backref', 'source-package', 'lock')]:
        wrong = deepcopy(graph); wrong.setdefault(frm, []).append(to)
        try: acyclic(wrong)
        except ValueError as exc:
            check(str(exc) == 'IdentityCycle', 'WrongCycleRejection')
            outcomes.append({'name':label, 'result':'rejected', 'code':'IdentityCycle'})
        else: raise AssertionError('cycle admitted')
    # The generated bytes exist only after input/profile identity is fixed.
    template = (ROOT / FACADE).read_text()
    rendered = template.replace('@MADB_NATIVE_MODULE@', '"libdesign-only.so"').replace('@MADB_EXPECTED_IDENTITY@', '"design-only-no-runtime-evidence"').encode()
    materialization = {'phase': 'design-render-only', 'producer': PACKAGE + '/generator',
       'composition_digest': digests['ordinary'], 'output_slot': 'ordinary-facade',
       'file': ref('generated/facade/Boundary.ets', rendered),
       'production_requires': ['checked OutputSet', 'exact task/profile/input identity', 'frozen bytes', 'no failed/cancelled commit']}
    def admit(value):
        check(value['producer'] == PACKAGE + '/generator', 'MaterializationProducer')
        check(value['composition_digest'] == digests['ordinary'], 'MaterializationComposition')
        check(value['file'] == ref('generated/facade/Boundary.ets', rendered), 'MaterializationBytes')
    positive('generated-identity-bound-after-input', lambda: admit(materialization))
    for field, bad, expected in [('producer', PACKAGE + '/reader','MaterializationProducer'),
          ('composition_digest',digests['enhanced'],'MaterializationComposition')]:
        wrong = deepcopy(materialization); wrong[field] = bad
        try: admit(wrong)
        except ValueError as exc:
            check(str(exc) == expected, 'WrongMaterializationRejection'); outcomes.append({'name':'generated-wrong-'+field,'result':'rejected','code':expected})
        else: raise AssertionError('materialization admitted')
    # New feature is mandatory only for participants interpreting the composition.
    core_text = (ROOT / 'crates/madb-model/src/contract.rs').read_text()
    core_slice = core_text.split('pub const CORE_FEATURES:')[1].split('];', 1)[0]
    old_features = set(re.findall(r'"(core[.][a-z.-]+@[0-9]+)"', core_slice))
    check(FEATURE not in old_features, 'UnexpectedAlreadyAdoptedFeature')
    def negotiate(required_features, known, advertised):
        check(set(required_features) <= known, 'UnknownMandatoryFeature')
        check(set(required_features) <= advertised, 'MissingMandatoryFeature')
    for label, required_features, known, advertised, expected in [
        ('new-host', [FEATURE], old_features | {FEATURE}, {FEATURE}, None),
        ('old-host', [FEATURE], old_features, {FEATURE}, 'UnknownMandatoryFeature'),
        ('peer-omits-feature', [FEATURE], old_features | {FEATURE}, set(), 'MissingMandatoryFeature'),
        ('plain-builder-no-profile-interpretation', [], old_features, set(), None)]:
        try: negotiate(required_features, known, advertised)
        except ValueError as exc:
            check(str(exc) == expected, 'FeatureRejectionMismatch')
            outcomes.append({'name': label, 'result': 'rejected', 'code': str(exc)})
        else:
            check(expected is None, 'FeatureIncorrectlyAccepted')
            outcomes.append({'name': label, 'result': 'passed'})
    manifest_schema = json.loads((ROOT / 'docs/schemas/extension-manifest.schema.json').read_text())
    positive('proposed-example-manifest-current-schema', lambda: jsonschema.Draft202012Validator(manifest_schema).validate(manifests[MANIFEST_PATH]))
    positive('example-manifest-remains-nonregistrable', lambda: check(manifests[MANIFEST_PATH]['example'] is True, 'DesignExamplePromoted'))
    raw = (canonical(examples['ordinary']) + '\n').encode()
    formatted = json.dumps(examples['ordinary'], indent=2).encode()
    positive('format-only-byte-change-semantic-stable', lambda: check(byte_digest(raw) != byte_digest(formatted) and semantic('madb.composition/1',project(json.loads(raw))) == semantic('madb.composition/1',project(json.loads(formatted))), 'Formatting'))
    result = {'revision': 3, 'previous_report': 'composition-decision-r2/composition-decision.json', 'original_report': 'composition-decision-r1/composition-decision.json', 'purpose': 'proposal-only; finite design vectors, NOT production parser/registration or full E01/E03/C07/L05 acceptance',
       'source_snapshot': {'head': json.loads(SNAPSHOT.read_text())['head'], 'inventory': ref(SNAPSHOT.name, SNAPSHOT.read_bytes()), 'repository': json.loads(SNAPSHOT.read_text())['source']},
       'source_pins': source_pins, 'virtual_proposal_manifests': manifests,
       'manifest_warning': 'example:true is intentional: driver is current private CLI, not a completed E02 package entrypoint; registration must reject this specimen',
       'examples': examples, 'digests': digests, 'profile2_composition_projection': profile_projection(digests['ordinary']),
       'generated_example': materialization, 'checks': outcomes, 'lock_path_examples': [qa, qb],
       'limits': ['Representative responsibility subset only', 'No compiler / target / source-package installation test', 'No claim that this selected source slice is full executable transitive closure', 'Production JCS and schemas still require shared vectors', 'No profile/1 implementation changed']}
    if write: OUT.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    else: check(json.loads(OUT.read_text()) == result, 'ReceiptDrift')
    print(json.dumps({'checks':len(outcomes), 'source_files':len(source_pins), 'digests':digests, 'written':write, 'source_root':str(ROOT)}, ensure_ascii=False))

if __name__ == '__main__':
    parser = argparse.ArgumentParser(); parser.add_argument('--write', action='store_true')
    parser.add_argument('--source-root', type=Path, default=ROOT)
    args = parser.parse_args(); ROOT = args.source_root
    run(args.write)

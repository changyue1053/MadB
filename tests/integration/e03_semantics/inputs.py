"""Untrusted applicant rows for the local source-semantics @2 recipe.

These declared rows are not requirements or authority. The Rust host derives its
own required set from fixed mechanisms and actual finalized IR, then compares.
The old @1 fixture and its five/eight-row meaning remain unchanged.
"""
from pathlib import Path
import sys

CHAIN = Path(__file__).resolve().parents[1] / 'e02_composition_chain'
sys.path.insert(0, str(CHAIN))
import prepare as chain
from prepare import base, d

PACKAGE = base.PACKAGE
ASSETS = {
    'native-runtime': ['sdk/arkts/native-runtime/src/protocol.cpp'],
    'native-worker': ['sdk/arkts/native-runtime/src/worker.cpp'],
    'native-gateway': ['sdk/arkts/native-gateway/src/api.cpp', 'sdk/arkts/native-gateway/src/runtime.cpp',
                       'sdk/arkts/native-runtime/include/provider_port.hpp'],
    'native-provider': ['sdk/c/arkts-provider/src/provider.c'],
}
DEPENDENCIES = [
    ('facade', 'native-gateway', 'target-runtime'),
    ('native-gateway', 'native-runtime', 'target-runtime'),
    ('native-gateway', 'native-worker', 'target-runtime'),
    ('native-gateway', 'native-provider', 'target-runtime'),
    ('native-worker', 'native-runtime', 'target-runtime'),
]
OPTIONAL_DEPENDENCIES = [
    ('state', 'facade', 'consumer-compile'), ('state', 'facade', 'target-runtime'),
    ('arkui', 'facade', 'consumer-compile'), ('arkui', 'state', 'consumer-compile'),
    ('arkui', 'state', 'target-runtime'),
]


def claims(policy, enhanced):
    rows = []
    public = lambda name: {'kind': 'public-symbol', 'public_id': policy['calls'][name]['public_id']}
    def add(name, stage, kind, anchor, layer, asset):
        owner = ({'kind': 'component', 'id': PACKAGE + '/' + asset}
                 if asset in ('reader', 'generator') else {'kind': 'sdk-asset', 'package': PACKAGE, 'asset': asset})
        rows.append({'boundary': 'c-arkts', 'subject': public(name), 'stage': stage,
                     'scope': {'kind': kind, 'anchor': public(anchor), 'layer': layer}, 'owner': owner})
    # Applicant's reviewable wiring declaration, independent of the host API.
    for name in policy['calls']:
        for stage, layer, owner in [('type-read', 'declaration', 'reader'), ('abi-lowering', 'generated-native', 'generator'),
                                    ('projection', 'facade', 'facade')]:
            add(name, stage, 'call', name, layer, owner)
    for name, kind, anchor in [
        ('Runtime.close', 'runtime', 'openRuntime'), ('Engine.close', 'engine', 'Runtime.connect'),
        ('Operation.result', 'operation', 'Engine.startOperation'), ('Operation.close', 'operation', 'Engine.startOperation'),
        ('Subscription.close', 'subscription', 'Subscription.close')]:
        add(name, 'async-bridge', kind, anchor, 'facade', 'facade')
    for row in [
        ('openRuntime', 'dispatch', 'runtime', 'openRuntime', 'env-notification', 'native-gateway'),
        ('Runtime.connect', 'dispatch', 'engine', 'Runtime.connect', 'worker', 'native-worker'),
        ('Engine.startOperation', 'dispatch', 'operation', 'Engine.startOperation', 'worker', 'native-worker'),
        ('Operation.requestCancel', 'cancel', 'operation', 'Engine.startOperation', 'source', 'native-runtime'),
        ('Engine.requestClose', 'stop', 'engine', 'Runtime.connect', 'source', 'native-runtime')]:
        add(*row)
    for observe, publish in [('Engine.observeState', 'Engine.publishState'), ('Engine.observeEvents', 'Engine.publishEvent')]:
        for name, stage, layer, owner in [
            (observe, 'subscription', 'source', 'native-runtime'), (publish, 'dispatch', 'source-queue', 'native-runtime'),
            (observe, 'dispatch', 'native-callback', 'native-gateway'), (observe, 'dispatch', 'js-listener', 'facade'),
            ('Subscription.requestStop', 'stop', 'source', 'native-runtime'), ('Subscription.close', 'drain', 'source', 'native-runtime'),
            ('Subscription.close', 'release', 'native-context', 'native-gateway'), ('Subscription.close', 'release', 'js-listener', 'facade')]:
            add(name, stage, 'subscription', observe, layer, owner)
    for name, kind, anchor in [('Engine.close', 'engine', 'Runtime.connect'), ('Operation.close', 'operation', 'Engine.startOperation')]:
        add(name, 'drain', kind, anchor, 'source', 'native-runtime')
        add(name, 'release', kind, anchor, 'public-record', 'facade')
    add('Engine.roundtrip', 'release', 'call', 'Engine.roundtrip', 'scratch', 'native-provider')
    add('Operation.result', 'release', 'operation', 'Engine.startOperation', 'result-payload', 'native-provider')
    add('Operation.result', 'release', 'operation', 'Engine.startOperation', 'result-ticket', 'native-runtime')
    for name, kind, anchor, payload, ticket in [
        ('Engine.startOperation', 'operation', 'Engine.startOperation', 'input-payload', 'input-ticket'),
        ('Engine.publishState', 'subscription', 'Engine.observeState', 'source-payload', 'source-ticket'),
        ('Engine.publishEvent', 'subscription', 'Engine.observeEvents', 'source-payload', 'source-ticket'),
        ('Operation.close', 'operation', 'Engine.startOperation', 'residual-payload', 'residual-ticket')]:
        add(name, 'release', kind, anchor, payload, 'native-provider')
        add(name, 'release', kind, anchor, ticket, 'native-runtime')
    for stage, layer, owner in [('stop', 'child-admission', 'native-gateway'), ('drain', 'physical-worker', 'native-worker'),
                                ('drain', 'env-notification', 'native-gateway'), ('release', 'env-reference', 'native-gateway'),
                                ('release', 'provider-context', 'native-provider'), ('release', 'runtime-receipt', 'facade')]:
        add('Runtime.close', stage, 'runtime', 'openRuntime', layer, owner)
    if enhanced:
        for asset, klass, layer in [('state', 'StateObservation', 'receiver'), ('arkui', 'ArkUIStateModel', 'view-model')]:
            subject = lambda member: {'kind': 'sdk-export', 'asset': asset, 'name': klass + '.' + member}
            members = [('attach', 'projection', layer), ('phase', 'projection', layer), ('closeFailure', 'projection', layer),
                       ('requestStop', 'stop', layer), ('close', 'async-bridge', layer)]
            if asset == 'arkui':
                members.append(('snapshot', 'projection', layer))
            if asset == 'state':
                members.append(('close', 'release', 'listener-reference'))
            for member, stage, detail in members:
                rows.append({'boundary': 'c-arkts', 'subject': subject(member), 'stage': stage,
                             'scope': {'kind': 'receiver', 'anchor': subject('attach'), 'layer': detail},
                             'owner': {'kind': 'sdk-asset', 'package': PACKAGE, 'asset': asset}})
    return rows


def prepare(args, root, mode):
    tools = chain.prepare(args, root, mode)
    composition = base.support.read_json(root / 'composition.json')
    profile_id = f'c07-c-arkts-source-semantics-{mode}-aarch64-ohos23'
    composition.update(id=f'c07-c-arkts-source-semantics-{mode}', recipe=f'madb.local.c07-c-arkts/source-semantics-{mode}@2')
    composition['selection']['profile_id'] = profile_id
    composition['assets'] = [a for a in composition['assets'] if a['id'] not in ASSETS]
    composition['assets'].extend({'id': asset, 'package': PACKAGE,
                                  'source': {'kind': 'source', 'files': [d.ref(root / base.PREFIX, path) for path in sorted(paths)]}}
                                 for asset, paths in ASSETS.items())
    edges = DEPENDENCIES + (OPTIONAL_DEPENDENCIES if mode == 'enhanced' else [])
    composition['dependencies'] = [{'from': a, 'to': b, 'scope': scope} for a, b, scope in edges]
    policy = base.support.read_json(root / 'adapters/generators/arkts/profiles/snapshot-v1.json')
    composition['responsibilities'] = claims(policy, mode == 'enhanced')
    d.save(root, 'composition.json', composition)
    profile = base.support.read_json(root / 'profile.json')
    profile['id'] = profile_id
    d.save(root, 'profile.json', profile)
    chain.repin(root, args.bridge, base.support.read_json(root / 'read-request.json'))
    return tools

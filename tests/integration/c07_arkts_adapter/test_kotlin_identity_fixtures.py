"""Producer negative-fixture construction tests; no target certification."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT))
from adapters.generators.arkts import support

fixture = support.load_module(ROOT / 'sdk/kotlin/arkts-owned/test-support/build_identity_fixtures.py')


def identity():
    values = ['sha256:' + char * 64 for char in 'abc']
    return dict(schema='madb.arkts-runtime-identity/1', bootstrap_version=1,
                abi_digest=values[0], profile_digest=values[1], build_id=values[2],
                expected_identity='madb-arkts/1:' + ':'.join(value[7:] for value in values))


class KotlinIdentityFixtureTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve(); self.inputs = self.root / 'inputs'; self.inputs.mkdir()
        config = dict(provider='kotlin', variant='test', kotlin={'test_support': True},
            native_module='libmadbc07sdkkotlintest.so', tools={'python': {'invocation_arguments': ['-I', '-S', '-B']}})
        self.write('backend.json', config)
        self.write('identity.json', identity())
        self.write('declarations.json', {'kotlin': {'test_support': True}})
        self.write(fixture.WORKER, '# actual execution is not part of this unit fixture\n')
        self.write(fixture.FACADE, 'const expected: string = ' + json.dumps(identity()['expected_identity']) + ';\n')
        self.write(fixture.BOOTSTRAP, (ROOT / fixture.BOOTSTRAP).read_text())
        self.task = self.root / 'compile.json'
        self.task_value = dict(schema='madb.build-task/1', phase='compile', dependencies=[],
            selector={'target': 'aarch64-linux-ohos', 'route': 'KN-ARKTS-NAPI'}, generation_id='test-only',
            arguments=['-I', '-S', '-B', '../inputs/' + fixture.WORKER, '--config', '../inputs/backend.json'])
        self.refresh()

    def write(self, name, value):
        p = self.inputs / name; p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(value if isinstance(value, str) else json.dumps(value))

    def refresh(self):
        self.task_value['inputs'] = [fixture.COMMON.reference(p, p.relative_to(self.inputs).as_posix())
                                    for p in sorted(self.inputs.rglob('*')) if p.is_file()]
        self.task.write_text(json.dumps(self.task_value))
        self.pin = support.sha(self.task)

    def test_each_fixture_changes_one_logical_component_and_derived_string(self):
        original = identity()
        for case, component in fixture.COMPONENTS.items():
            changed = fixture.derive_identity(original, case)
            self.assertEqual({k for k in original if changed[k] != original[k]}, {component, 'expected_identity'})
            self.assertEqual(original, identity())
            self.assertNotEqual(changed[component], original[component])

    def test_inconsistent_tuple_is_rejected(self):
        wrong = identity(); wrong['expected_identity'] += 'x'
        with self.assertRaisesRegex(ValueError, 'derived expected'):
            fixture.derive_identity(wrong, 'wrong-abi')

    def test_prepare_preserves_original_facade_and_only_changes_identity_file(self):
        receipts = fixture.build_fixtures(self.task, self.pin, self.inputs, self.root / 'output', prepare_only=True)
        self.assertEqual(len(receipts), 3)
        for item in receipts:
            receipt = json.loads(Path(item['path']).read_text())
            self.assertFalse(receipt['candidate']); self.assertFalse(receipt['target_executed'])
            self.assertEqual(receipt['build_status'], 'not-run')
            self.assertEqual(receipt['changed_files'], ['identity.json'])
            self.assertEqual(receipt['unchanged_facade']['sha256'], support.sha(self.inputs / fixture.FACADE))
            self.assertEqual(receipt['bootstrap_error'], 'IdentityMismatch')
            self.assertEqual(receipt['provider_only_error'], 'ProviderIdentityMismatch')
        self.assertEqual(json.loads((self.inputs / 'identity.json').read_text()), identity())

    def test_wrong_task_pin_and_route_are_rejected(self):
        with self.assertRaisesRegex(ValueError, 'caller pin'):
            fixture.validate_source(self.task, 'sha256:' + '0' * 64, self.inputs)
        self.task_value['selector']['route'] = 'ARKTS-C-NAPI'; self.refresh()
        with self.assertRaisesRegex(ValueError, 'original Kotlin'):
            fixture.validate_source(self.task, self.pin, self.inputs)

    def test_ordinary_or_fake_c_frontend_cannot_supply_kn_test_fixture(self):
        config = json.loads((self.inputs / 'backend.json').read_text())
        config['variant'] = 'ordinary'; self.write('backend.json', config); self.refresh()
        with self.assertRaisesRegex(ValueError, 'test companion'):
            fixture.validate_source(self.task, self.pin, self.inputs)
        config['variant'] = 'test'; self.write('backend.json', config)
        self.write('declarations.json', {'kotlin': {'test_support': True}, 'c_seam': {}}); self.refresh()
        with self.assertRaisesRegex(ValueError, 'actual test KSP'):
            fixture.validate_source(self.task, self.pin, self.inputs)

    def test_missing_facade_expectation_or_changed_input_is_rejected(self):
        self.write(fixture.FACADE, 'missing'); self.refresh()
        with self.assertRaisesRegex(ValueError, 'facade expectation'):
            fixture.validate_source(self.task, self.pin, self.inputs)
        self.write(fixture.FACADE, 'changed-again')
        with self.assertRaises(ValueError):
            fixture.validate_source(self.task, self.pin, self.inputs)

    def test_init_before_identity_check_is_rejected(self):
        source = (self.inputs / fixture.BOOTSTRAP).read_text()
        source = source.replace('std::memcmp(expected,MADB_KN_OWNED_IDENTITY,size)', 'changedIdentityCall')
        self.write(fixture.BOOTSTRAP, source); self.refresh()
        with self.assertRaisesRegex(ValueError, 'admission order'):
            fixture.validate_source(self.task, self.pin, self.inputs)

    def test_existing_output_is_never_overwritten(self):
        output = self.root / 'existing'; output.mkdir(); marker = output / 'keep'; marker.write_text('keep')
        with self.assertRaisesRegex(ValueError, 'fresh separate'):
            fixture.build_fixtures(self.task, self.pin, self.inputs, output, prepare_only=True)
        self.assertEqual(marker.read_text(), 'keep')


if __name__ == '__main__':
    unittest.main()

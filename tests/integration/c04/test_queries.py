"""Real CLI proofs for the C04 read-only slice; no SDK/transaction claim."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from fixture import ROOT, materialize, raw, ref, save, task_digest


class Queries(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        subprocess.run(['cargo', 'build', '--bin', 'madb', '--locked'], cwd=ROOT, check=True, capture_output=True)
        cls.binary = ROOT / 'target/debug/madb'

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='madb-query-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.values = materialize(self.root)
        # Any accidental compiler/process lookup would create a visible marker.
        (self.root / 'bin').mkdir()
        for tool in ['rustc', 'cargo', 'clang', 'bindgen', 'cbindgen']:
            script = self.root / 'bin' / tool
            script.write_text('#!/bin/sh\nprintf called > "$HOME/process-called"\nexit 91\n')
            script.chmod(0o755)

    def tree(self):
        return {str(p.relative_to(self.root)): (p.lstat().st_mode, p.lstat().st_mtime_ns,
                os.readlink(p) if p.is_symlink() else raw(p.read_bytes()) if p.is_file() else None)
                for p in self.root.rglob('*')}

    def run_query(self, operation='check', extra=(), expected=0, marker=None, snapshot='contract.json'):
        before = self.tree()
        result = subprocess.run([str(self.binary), operation, '--root', str(self.root), '--project', 'project.json',
                                 '--request', 'read.json', '--snapshot', snapshot, *extra],
                                cwd=self.root, env={'PATH': str(self.root / 'bin'), 'HOME': str(self.root)},
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, expected, result.stdout + result.stderr)
        if marker:
            self.assertIn(marker, result.stdout + result.stderr)
        self.assertEqual(before, self.tree(), 'read-only command modified workspace contents or metadata')
        self.assertFalse((self.root / 'process-called').exists())
        return result.stdout

    def test_inspect_and_check_existing_snapshot(self):
        output = self.run_query('inspect', marker='fixture.checksum.v1')
        self.assertIn('native/fixture.rs:', output)
        self.assertIn('(project-policy)', output)
        self.assertIn('madb.toml:100:1', output)
        self.assertIn('dependency closure not certified', output)
        self.run_query(marker='Contract checks passed')

    def test_missing_snapshot_returns_preparation_without_execution(self):
        for operation in ['inspect', 'check', 'plan']:
            self.run_query(operation, expected=2, marker='MetadataRequired', snapshot='absent.json')

    def test_missing_or_changed_inputs_require_preparation(self):
        for change in ['changed', 'missing']:
            with self.subTest(change=change):
                path = self.root / 'native/fixture.rs'
                if change == 'changed':
                    path.write_text('different source')
                else:
                    path.unlink()
                self.run_query(expected=2, marker='META001 at native/fixture.rs')

    def test_untracked_file_is_not_mistaken_for_a_complete_dependency_proof(self):
        (self.root / 'new-header.h').write_text('not inventoried')
        self.run_query(marker='dependency closure not certified')

    def test_changed_lock_or_profile_bytes_are_rejected(self):
        for name in ['madb.lock', 'profiles/rust-c.json']:
            with self.subTest(name=name):
                path = self.root / name
                original = path.read_bytes()
                path.write_bytes(original + b'\n')
                self.run_query(expected=2, marker='FILE004')
                path.write_bytes(original)

    def test_malformed_snapshot_is_not_missing_metadata(self):
        (self.root / 'contract.json').write_text('{"schema":1,"schema":2}')
        self.run_query(expected=2, marker='FMT005')

    def test_semantic_failure_is_not_success(self):
        ir = self.values['contract-ir']
        ir['symbols'][0]['shape_digest'] = 'sha256:' + '0' * 64
        save(self.root, 'contract.json', ir)
        self.run_query(expected=1, marker='HASH003')

    def test_stale_snapshot_identity_requires_metadata(self):
        ir = self.values['contract-ir']
        ir['source_digest'] = 'sha256:' + '0' * 64
        save(self.root, 'contract.json', ir)
        self.run_query(expected=2, marker='MetadataRequired')

    def test_static_plan_orders_tasks_and_selects_dependencies(self):
        files = ('--task-file', 'compile.json', '--task-file', 'metadata.json', '--task-file', 'generate.json')
        output = self.run_query('plan', files, marker='Static task plan')
        self.assertLess(output.index('metadata [Metadata]'), output.index('generate [Generate]'))
        self.assertLess(output.index('generate [Generate]'), output.index('compile [Compile]'))
        output = self.run_query('plan', (*files, '--task', 'generate'))
        self.assertNotIn('compile [Compile]', output)
        self.run_query('plan', (*files, '--active-task', 'metadata'), expected=1, marker='recursive')
        self.run_query('plan', (*files, '--task', 'missing'), expected=1, marker='unknown named task')
        self.run_query('plan', expected=2, marker='PLAN003')

    def test_plan_rejects_cycle_output_alias_and_tampered_identity(self):
        files = ('--task-file', 'metadata.json', '--task-file', 'generate.json')
        original = json.loads((self.root / 'metadata.json').read_text())
        edits = [dict(phase='generate', dependencies=['generate']), dict(outputs=['generate.out']), dict(arguments=['not locked task'])]
        for index, edit in enumerate(edits):
            with self.subTest(index=index):
                task = dict(original, **edit)
                if index < 2:
                    task['task_digest'] = task_digest(task)
                save(self.root, 'metadata.json', task)
                self.run_query('plan', files, expected=1 if index < 2 else 2, marker='PLAN001' if index < 2 else 'PLAN002')

    def test_task_tool_must_belong_to_the_selected_profile(self):
        for target in ['x86_64-apple-darwin', 'aarch64-apple-darwin']:
            with self.subTest(target=target):
                lock = self.values['lock'].copy()
                tool = dict(lock['tools'][0], id='foreign-tool', target=target)
                lock['tools'] = [*lock['tools'], tool]
                save(self.root, 'madb.lock', lock)
                request = self.values['read-request'].copy()
                request['lock'] = ref(self.root, 'madb.lock', 'madb.lock/1')
                save(self.root, 'read.json', request)
                task = json.loads((self.root / 'metadata.json').read_text())
                task['tool'] = {key: tool[key] for key in ['id', 'version', 'sha256']}
                task['task_digest'] = task_digest(task)
                save(self.root, 'metadata.json', task)
                self.run_query('plan', ('--task-file', 'metadata.json'), expected=2, marker='LOCK004')

    def test_confined_reads_reject_links_special_files_and_limits(self):
        original = (self.root / 'contract.json').read_bytes()
        outside = self.root.parent / (self.root.name + '-outside')
        outside.write_bytes(original)
        self.addCleanup(outside.unlink)
        path = self.root / 'contract.json'
        for kind in ['symlink', 'hardlink', 'fifo', 'large']:
            with self.subTest(kind=kind):
                path.unlink()
                if kind == 'symlink':
                    path.symlink_to(outside)
                elif kind == 'hardlink':
                    os.link(outside, path)
                elif kind == 'fifo':
                    os.mkfifo(path)
                else:
                    with path.open('wb') as file:
                        file.truncate(1024 * 1024 + 1)
                self.run_query(expected=2, marker='HOST003' if kind == 'large' else 'HOST002')
        self.run_query(expected=2, marker='FILE001', snapshot='../escape.json')

    def test_symlink_in_parent_and_read_only_permissions(self):
        native = self.root / 'native'
        moved = self.root / 'actual-native'
        native.rename(moved)
        native.symlink_to(moved, target_is_directory=True)
        self.run_query(expected=2, marker='HOST002')
        native.unlink()
        moved.rename(native)
        paths = list(self.root.rglob('*')) + [self.root]
        modes = {p: p.stat().st_mode for p in paths}
        try:
            for p in paths:
                p.chmod(0o555 if p.is_dir() else 0o444)
            self.run_query(marker='Contract checks passed')
        finally:
            for p, mode in modes.items():
                p.chmod(mode)


if __name__ == '__main__':
    unittest.main()

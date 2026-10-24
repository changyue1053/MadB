"""CRIT-06/09: actual processes, immutable file batches, index barriers and leases."""
import json
import struct

from support import PackageFixture, build_binary, byte_digest, tree_bytes, wait_json, write_json

LAYERS = ("source", "build", "candidate")
EMPTY = {"revision": 0, "source": None, "build": None, "candidate": None}


def content_id(spec, files):
    """Independent implementation of the documented internal storage projection."""
    encoded = bytearray(b"madb.storage-content/1\0")
    def append(value):
        raw = value.encode()
        encoded.extend(struct.pack("<Q", len(raw)))
        encoded.extend(raw)
    append(spec["layer"])
    for key in ("input_digest", "task_digest", "profile_digest", "generation_id"):
        append(spec[key])
    append(spec["parent"] or "")
    for file in sorted(files, key=lambda item: item["path"]):
        append(file["path"])
        append(file["sha256"])
        encoded.extend(struct.pack("<Q", file["size"]))
    return byte_digest(encoded)


def strict_object(items):
    value = {}
    for key, item in items:
        if key in value:
            raise AssertionError(f"duplicate persisted key: {key}")
        value[key] = item
    return value


class TransactionProcesses(PackageFixture):
    @classmethod
    def setUpClass(cls):
        cls.driver = build_binary("package_probe", example=True)

    def setUp(self):
        super().setUp()
        self.serial = 0
        self.expected_batches = {}

    def config(self, operation, **extra):
        value = {"operation": operation, "domain": str(self.domain)}
        value.update(extra)
        return value

    def commit_config(self, version="v1", layer="source", parent=None, payload=None, **extra):
        self.serial += 1
        source = self.root / f"input-{self.serial}"
        source.mkdir()
        payload = payload or {"api.h": f"declaration:{version}".encode(),
                              "nested/implementation.c": f"implementation:{version}".encode()}
        files = []
        for name, data in sorted(payload.items()):
            path = source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            files.append({"path": name, "sha256": byte_digest(data), "size": len(data)})
        spec = {"layer": layer, "input_digest": byte_digest(("input:" + version).encode()),
                "task_digest": byte_digest(("task:" + layer + ":" + version).encode()),
                "profile_digest": byte_digest(b"test-profile"),
                "generation_id": byte_digest(("generation:" + version).encode()), "parent": parent}
        self.expected_batches[content_id(spec, files)] = (spec, files, payload)
        return self.config("commit", input_root=str(source), files=files, spec=spec, **extra)

    def start(self, config):
        self.serial += 1
        path = self.root / f"command-{self.serial}.json"
        write_json(path, config)
        return self.spawn([str(self.driver), str(path)])

    def invoke(self, config):
        out, err = self.finish(self.start(config))
        self.assertEqual(err, "")
        return json.loads(out)

    def commit(self, config, disposition="Committed"):
        result = self.invoke(config)
        self.assertEqual(result["outcome"]["status"], "ok", result)
        self.assertEqual(result["outcome"]["disposition"], disposition, result)
        self.assertEqual(result["content_id"], content_id(config["spec"], config["files"]))
        self.assertEqual(result["late_cancel"], "AlreadyFinished")
        self.assert_index(result["outcome"]["index"])
        return result

    def current(self):
        path = self.domain / "current.json"
        if not path.exists():
            return dict(EMPTY)
        value = json.loads(path.read_text(), object_pairs_hook=strict_object)
        self.assertEqual(value.pop("schema"), "madb.storage-index/1")
        self.assertEqual(set(value), set(EMPTY))
        return value

    def assert_index(self, expected):
        """Validate physical current→metadata→files and all active parent edges."""
        current = self.current()
        self.assertEqual(current, expected)
        for layer in LAYERS:
            instance = current[layer]
            if instance is None:
                continue
            self.assertTrue(instance.startswith("sha256:"))
            directory = self.domain / "batches" / instance.removeprefix("sha256:")
            metadata = json.loads((directory / "metadata.json").read_text(), object_pairs_hook=strict_object)
            self.assertEqual(metadata["schema"], "madb.storage-batch/1")
            self.assertEqual(metadata["id"], instance)
            self.assertEqual(metadata["spec"]["layer"], layer)
            spec, files, payload = self.expected_batches[metadata["content_id"]]
            self.assertEqual(metadata["spec"], spec)
            self.assertEqual(metadata["files"], files)
            self.assertEqual(metadata["content_id"], content_id(spec, files))
            file_root = directory / "files"
            actual_files = sorted(p.relative_to(file_root).as_posix() for p in file_root.rglob("*") if p.is_file())
            self.assertEqual(actual_files, sorted(payload))
            for file in files:
                raw = (file_root / file["path"]).read_bytes()
                self.assertEqual(raw, payload[file["path"]])
                self.assertEqual(len(raw), file["size"])
                self.assertEqual(byte_digest(raw), file["sha256"])
            if layer != "source":
                previous = "source" if layer == "build" else "build"
                self.assertEqual(spec["parent"], current[previous])

    def barrier(self, config, point, name):
        ready, release = self.root / f"{name}-ready.json", self.root / f"{name}-release"
        config.update(gate_at=point, ready=str(ready), release=str(release))
        process = self.start(config)
        observed = wait_json(ready, process)
        self.assertEqual(observed["point"], point)
        self.assertEqual(observed["pid"], process.pid)
        return process, release

    def finish_json(self, process):
        out, err = self.finish(process)
        self.assertEqual(err, "")
        return json.loads(out)

    def test_source_commit_has_independently_verified_index_metadata_and_files(self):
        result = self.commit(self.commit_config())
        self.assertEqual(result["current"]["revision"], 1)
        self.assertIsNone(result["current"]["build"])
        self.assertIsNone(result["current"]["candidate"])

    def test_all_three_layers_keep_fixed_parent_pairings(self):
        source = self.commit(self.commit_config())["outcome"]["batch_id"]
        build = self.commit(self.commit_config(layer="build", parent=source,
                            payload={"libfixture.a": b"real bytes validated separately by CLI suite"}))["outcome"]["batch_id"]
        candidate = self.commit(self.commit_config(layer="candidate", parent=build,
                                payload={"package/header.h": b"public declaration", "package/lib.a": b"bound library"}))
        self.assertEqual(candidate["current"], {"revision": 3, "source": source, "build": build,
                                                "candidate": candidate["outcome"]["batch_id"]})

    def test_new_source_clears_stale_build_and_candidate_without_mixing(self):
        source = self.commit(self.commit_config())["outcome"]["batch_id"]
        build = self.commit(self.commit_config(layer="build", parent=source))["outcome"]["batch_id"]
        self.commit(self.commit_config(layer="candidate", parent=build))
        result = self.commit(self.commit_config(version="v2"))
        self.assertEqual(result["current"]["revision"], 4)
        self.assertIsNone(result["current"]["build"])
        self.assertIsNone(result["current"]["candidate"])

    def test_old_source_cannot_parent_a_new_build(self):
        old = self.commit(self.commit_config())["outcome"]["batch_id"]
        new = self.commit(self.commit_config(version="v2"))["current"]
        result = self.invoke(self.commit_config(layer="build", parent=old))
        self.assertEqual(result["outcome"]["codes"], ["STORE003"], result)
        self.assertEqual(result["current"], new)
        self.assert_index(new)

    def test_identical_late_result_is_verified_and_reused_without_revision_change(self):
        first = self.commit(self.commit_config())
        duplicate = self.commit(self.commit_config(expected=dict(EMPTY)), disposition="Reused")
        self.assertEqual(duplicate["current"], first["current"])
        self.assertEqual(duplicate["outcome"]["batch_id"], first["outcome"]["batch_id"])
        self.assertNotEqual(duplicate["staged_id"], first["outcome"]["batch_id"])

    def test_conflicting_late_result_cannot_replace_current(self):
        first = self.commit(self.commit_config())
        result = self.invoke(self.commit_config(version="v2", expected=dict(EMPTY)))
        self.assertEqual(result["outcome"]["codes"], ["STORE003"], result)
        self.assert_index(first["current"])

    def test_old_process_staging_does_not_hold_domain_lock_or_overwrite_new_process(self):
        self.commit(self.commit_config(version="v0"))
        old_config = self.commit_config(version="v1")
        old, release = self.barrier(old_config, "Staged", "old")
        new = self.commit(self.commit_config(version="v2"))
        self.assertIsNone(old.poll(), "old process must remain paused while newer one commits")
        release.write_text("resume")
        late = self.finish_json(old)
        self.assertEqual(late["outcome"]["codes"], ["STORE003"], late)
        self.assertEqual(late["current"], new["current"])
        self.assert_index(new["current"])

    def test_two_processes_with_same_content_converge_on_one_committed_instance(self):
        a, release_a = self.barrier(self.commit_config(), "Staged", "a")
        b, release_b = self.barrier(self.commit_config(), "Staged", "b")
        release_b.write_text("first")
        first = self.finish_json(b)
        self.assertEqual(first["outcome"]["disposition"], "Committed")
        release_a.write_text("second")
        second = self.finish_json(a)
        self.assertEqual(second["outcome"]["disposition"], "Reused")
        self.assertEqual(first["current"], second["current"])
        self.assertEqual(first["current"]["revision"], 1)
        self.assert_index(first["current"])

    def test_cancel_before_index_acceptance_prevents_publication(self):
        old = self.commit(self.commit_config())["current"]
        result = self.invoke(self.commit_config(version="v2", cancel_at="BeforeIndexReplace"))
        self.assertEqual(result["outcome"]["codes"], ["STORE002"], result)
        self.assertEqual(result["cancellations"], ["Requested"])
        self.assertEqual(result["late_cancel"], "AlreadyFinished")
        self.assert_index(old)

    def test_cancel_after_index_acceptance_does_not_retract_commit(self):
        result = self.commit(self.commit_config(cancel_at="AfterIndexReplace"))
        self.assertEqual(result["cancellations"], ["AlreadyFinished"])
        self.assertEqual(result["current"]["revision"], 1)

    def test_precancelled_commit_cannot_publish_even_after_staging(self):
        result = self.invoke(self.commit_config(cancel_at="before-stage"))
        self.assertEqual(result["outcome"]["codes"], ["STORE002"], result)
        self.assertEqual(result["cancellations"], ["Requested"])
        self.assertEqual(self.current(), EMPTY)

    def test_source_revalidation_failure_prevents_index_change(self):
        old = self.commit(self.commit_config())["current"]
        guard = self.root / "current-source"
        guard.write_bytes(b"changed")
        result = self.invoke(self.commit_config(version="v2", guard={"path": str(guard), "sha256": byte_digest(b"original")}))
        self.assertEqual(result["outcome"]["codes"], ["TEST_INPUT_CHANGED"], result)
        self.assert_index(old)

    def crash(self, point, accepted):
        old = self.commit(self.commit_config(version="v0"))["current"]
        config = self.commit_config(version="v1")
        process, _ = self.barrier(config, point, "crash")
        process.kill()
        out, err = process.communicate(timeout=3)
        self.assertLess(process.returncode, 0)
        self.assertEqual((out, err), ("", ""))
        current = self.current()
        self.assertEqual(current["revision"], 2 if accepted else 1)
        if not accepted:
            self.assertEqual(current, old)
        recovered = self.invoke(self.config("recover"))
        self.assertEqual(recovered["index"], current, recovered)
        self.assert_index(current)
        self.assertEqual(list((self.domain / "staging").iterdir()), [])
        visible = {value.removeprefix("sha256:") for layer, value in current.items()
                   if layer in LAYERS and value is not None}
        self.assertEqual({path.name for path in (self.domain / "batches").iterdir()}, visible)

    def test_crash_during_file_writes_never_publishes_partial_batch(self):
        self.crash("FileWritten", accepted=False)

    def test_crash_after_staging_before_index_keeps_previous_generation(self):
        self.crash("Staged", accepted=False)

    def test_crash_immediately_before_index_replace_keeps_previous_generation(self):
        self.crash("BeforeIndexReplace", accepted=False)

    def test_crash_after_index_replace_recovers_complete_new_generation(self):
        self.crash("AfterIndexReplace", accepted=True)

    def test_pinned_old_generation_survives_new_commit_and_gc_until_release(self):
        old = self.commit(self.commit_config())
        ready, release = self.root / "pin-ready.json", self.root / "pin-release"
        pinned = self.start(self.config("pin", layer="source", ready=str(ready), release=str(release)))
        lease = wait_json(ready, pinned)
        self.assertEqual(lease["identity"]["id"], old["outcome"]["batch_id"])
        old_path = self.domain / "batches" / old["outcome"]["batch_id"].removeprefix("sha256:")
        before = tree_bytes(old_path)
        new = self.commit(self.commit_config(version="v2"))
        recovery = self.invoke(self.config("recover"))
        self.assertEqual(recovery["index"], new["current"])
        self.assertTrue(old_path.is_dir())
        self.assertEqual(tree_bytes(old_path), before)
        self.assertIn("batches/" + old_path.name, recovery["retained_active"])
        release.write_text("lease may finish")
        result = self.finish_json(pinned)
        self.assertTrue(result["exists"])
        self.assertEqual(result["before"], result["after"])
        expected_payload = self.expected_batches[old["content_id"]][2]
        self.assertEqual({item["path"]: bytes(item["bytes"]) for item in result["after"]}, expected_payload)
        self.invoke(self.config("recover"))
        self.assertFalse(old_path.exists())
        self.assert_index(new["current"])

    def test_corrupt_current_index_fails_closed_without_recovery_guessing(self):
        self.commit(self.commit_config())
        current = self.domain / "current.json"
        current.chmod(0o600)
        current.write_bytes(b'{"schema":"madb.storage-index/1","revision":1,"revision":0}')
        before = tree_bytes(self.domain)
        result = self.invoke(self.config("recover"))
        self.assertEqual(result["codes"], ["STORE004"], result)
        self.assertEqual(tree_bytes(self.domain), before)

    def test_pinning_detects_corrupt_file_in_current_batch(self):
        committed = self.commit(self.commit_config())
        file = self.domain / "batches" / committed["outcome"]["batch_id"].removeprefix("sha256:") / "files/api.h"
        file.chmod(0o600)
        file.write_bytes(b"corrupt")
        result = self.invoke(self.config("pin", layer="source"))
        self.assertEqual(result["codes"], ["STORE004"], result)

    def test_source_changed_at_final_commit_barrier_is_revalidated_before_switch(self):
        old = self.commit(self.commit_config())["current"]
        guard = self.root / "guard-input"
        guard.write_bytes(b"initial")
        config = self.commit_config(version="v2", guard={"path": str(guard), "sha256": byte_digest(b"initial")})
        process, release = self.barrier(config, "BeforeIndexReplace", "final-guard")
        guard.write_bytes(b"changed after staging and before acceptance")
        release.write_text("continue validation")
        result = self.finish_json(process)
        self.assertEqual(result["outcome"]["codes"], ["TEST_INPUT_CHANGED"])
        self.assertEqual(result["current"], old)
        self.assert_index(old)

    def test_candidate_lease_keeps_entire_old_parent_chain_alive_until_release(self):
        source = self.commit(self.commit_config())["outcome"]["batch_id"]
        build = self.commit(self.commit_config(layer="build", parent=source))["outcome"]["batch_id"]
        candidate = self.commit(self.commit_config(layer="candidate", parent=build))["outcome"]["batch_id"]
        old_paths = [self.domain / "batches" / identity.removeprefix("sha256:") for identity in [source, build, candidate]]
        previous = {path: tree_bytes(path) for path in old_paths}
        ready, release = self.root / "candidate-ready.json", self.root / "candidate-release"
        process = self.start(self.config("pin", layer="candidate", ready=str(ready), release=str(release)))
        wait_json(ready, process)
        newest = self.commit(self.commit_config(version="next"))["current"]
        recovered = self.invoke(self.config("recover"))
        self.assertEqual(recovered["index"], newest)
        for path in old_paths:
            self.assertTrue(path.exists())
            self.assertEqual(tree_bytes(path), previous[path])
            self.assertIn("batches/" + path.name, recovered["retained_active"])
        release.write_text("release fixed candidate and parents")
        self.finish_json(process)
        self.invoke(self.config("recover"))
        self.assertTrue(all(not path.exists() for path in old_paths))
        self.assert_index(newest)

    def test_unowned_nonempty_domain_is_never_claimed_or_cleaned(self):
        unrelated = self.domain / "staging/photos/sentinel"
        unrelated.parent.mkdir(parents=True)
        unrelated.write_bytes(b"not MadB data")
        before = tree_bytes(self.domain)
        for config in [self.config("recover"), self.commit_config()]:
            with self.subTest(operation=config["operation"]):
                result = self.invoke(config)
                self.assertEqual(result["codes"], ["STORE004"], result)
                self.assertEqual(tree_bytes(self.domain), before)

    def test_missing_owner_marker_does_not_allow_recovery_to_guess_ownership(self):
        self.commit(self.commit_config())
        (self.domain / ".owner.json").unlink()
        before = tree_bytes(self.domain)
        result = self.invoke(self.config("recover"))
        self.assertEqual(result["codes"], ["STORE004"], result)
        self.assertEqual(tree_bytes(self.domain), before)

    def test_corrupt_owner_marker_prevents_all_recovery_deletions(self):
        self.commit(self.commit_config())
        owner = self.domain / ".owner.json"
        owner.chmod(0o600)
        owner.write_bytes(b'{"schema":"different-tool/1"}')
        before = tree_bytes(self.domain)
        result = self.invoke(self.config("recover"))
        self.assertEqual(result["codes"], ["STORE004"], result)
        self.assertEqual(tree_bytes(self.domain), before)

# Ordinary ArkTS composition Source session

This fixed local development root binds the actual ordinary C→ArkTS source
package and separately installed peer image, negotiates its real IR requirements
and `core.extension-composition@1`, invokes the existing production generator via
public E02 `AuthorSession`, then commits one complete Source batch through C04.
The old profile/1 constructors continue to reject composition inputs. The local
source set is explicitly selected and captured; no remote publisher certification
or package downloader is implied.

The peer source entry is `crates/madb-cli/examples/arkts_composition_peer.rs`;
its host-selected installed image is `arkts-peer`. Both are independently bound.
The actual project bytes are included in the locked input closure because the
production metadata finalizer reads them. Original source, manifests and tool
images receive new identities; old C07 or sixth-batch evidence is not relabelled.

Build the three examples with the pinned Rust toolchain, then run:

```
python3.14 -B tests/integration/arkts_composition_session/check.py \
  --output /absolute/new/output \
  --bridge /absolute/composition_generation_probe \
  --probe /absolute/composition_session_probe \
  --peer /absolute/arkts_composition_peer \
  --observation /absolute/existing-formal-sdk-tool-observation.json
```

The observation selects existing reviewed formal tools. The production reader
rechecks each tool and its actual runtime/SDK trees; existence of a saved JSON
observation does not constitute current execution evidence. No simulator is used.

Checks cover the real successful generator session and Source commit, cancellation
before launch, during result acceptance and before commit, and payload alteration
after terminal capture. Failures/cancellation retain the old complete Source index.
`consume_source.py` runs separately with only the exported Source directory,
verifies the public inventory/bytes, and rejects a changed payload; it does not
import MadB modules or consume a helper's former writable tree.

This is one generator component plus fixed SDK assets. It does not prove multiple
component sessions, native/SDK compilation, Build/Candidate transactions, HAR
installation, target-runtime behavior, Lab certification or entire E01–E03
acceptance. The output retains those explicit missing items.

"""Finite C07 source/output observations; no registration, IR or commit authority.

Required roles come from the actual public AST, authored C07 policy and a fixed
SDK source recipe. No submitted responsibility list is accepted by this API.
The private values below are not a new public wire format or an OutputSet.
"""
from dataclasses import dataclass
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
BASE = Path(__file__).parent
TYPESCRIPT_SHA256 = "9475e93f3904bb0a6a83ec457cbb4926c28d02202bac7b1fb31dbc49af7d56a1"
NODE_SHA256 = "902b6a6984d5d825829ea9064ab73b734548df37bc0683990dca31c8dc2a9253"
MAX_BYTES = 1024 * 1024
SOURCE_PINS = {
    "sdk/arkts/facade/Boundary.ets.in": "dd3775ec075333a79efaeb236b6dc8da46622fea22b43a6e021cf4cfe19faa15",
    "sdk/arkts/facade/RuntimeFrame.ets": "a5eacf4b6b7aa7fdd886e1cc19ab3147517f96b60f19b4074df243de66e98492",
    "adapters/generators/arkts/profiles/snapshot-v1.json": "88729ffd4878a7aa991ce3a94f0edd3b3ec156751fbe5b12bca7cf050b2ced75",
    "sdk/arkts/native-runtime/src/protocol.cpp": "eb3978d2dda0e2ac1c67c485a4b74d66bf9b4a3155934ab895245fb23a0bcc25",
    "sdk/arkts/observation/StateObservation.ets": "1e26f847c8e37d7c65b236188d86c6ee3ad6dcc53c033d4304049a7ab7531534",
    "sdk/arkts/observation/ArkUIStateModel.ets": "7e62d7352ded16641e560f9230b65576d524caa8ac847dc65715e7e877ad77ef",
    "docs/adr/0013-arkts-runtime-and-completion.md": "8ef892436312636f7f2e368426c7f30c6949a170bf7a30e8e8b3ea59a1011a5a",
}
FACADE, FRAME, POLICY, NATIVE, STATE, UI, AUTHORITY = SOURCE_PINS
IMPLEMENTATION = tuple("adapters/generators/arkts/" + name for name in (
    "driver.py", "support.py", "lowering.py", "read_c_seam.py", "diagnostics.py",
    "declaration_diagnostics.py", "read_facade.cjs", "read_observation.cjs",
    "composition_recipe.py")) + (".node-version",)


@dataclass(frozen=True)
class FileObservation:
    path: str
    sha256: str
    size: int


@dataclass(frozen=True)
class RequiredRole:
    """Source-normalized private facts, not claims of a registered SDK package."""
    subject_kind: str
    subject: str
    stage: str
    scope_kind: str
    anchor_kind: str
    anchor: str
    layer: str
    source: FileObservation
    generated_path: str | None = None


@dataclass(frozen=True)
class RecipeObservation:
    recipe: str
    inputs: tuple[FileObservation, ...]
    implementation: tuple[FileObservation, ...]
    tools: tuple[FileObservation, ...]
    required: tuple[RequiredRole, ...]
    outputs: tuple[FileObservation, ...]
    # This native dependency is deliberately not fabricated as a generated .so.
    external_imports: tuple[str, ...] = ("libmadbc07sdk.so",)


def _sha(data):
    return hashlib.sha256(data).hexdigest()


def _ref(name, data):
    return FileObservation(name, "sha256:" + _sha(data), len(data))


def _absolute_unaliased(value):
    path = Path(value)
    if not path.is_absolute() or ".." in path.parts:
        raise ValueError("invalid domain path: absolute path without .. required")
    if any(part.is_symlink() for part in (path, *path.parents)):
        raise ValueError("symbolic domain path or ancestor")
    return path


def _directory(value, *, new=False):
    path = _absolute_unaliased(value)
    if new:
        if path.exists():
            raise ValueError("output domain must be new")
        if not path.parent.is_dir():
            raise ValueError("output parent must be an existing directory")
        return path.parent.resolve(strict=True) / path.name
    if not path.is_dir():
        raise ValueError("absent observation directory")
    return path.resolve(strict=True)


def _separate(source, output):
    # samefile also catches differently cased aliases on case-insensitive hosts.
    if output.is_relative_to(source) or any(parent.samefile(source) for parent in output.parents):
        raise ValueError("source and output domains must be separate")


def _read(root, name):
    if Path(name).is_absolute() or any(part in ("..", ".") for part in Path(name).parts):
        raise ValueError("invalid observation path")
    path = _directory(root)
    for part in Path(name).parts:
        path = path / part
        if path.is_symlink():
            raise ValueError("symbolic source/output path: " + name)
    if not path.is_file() or path.stat().st_nlink != 1 or path.stat().st_size > MAX_BYTES:
        raise ValueError("missing/nonregular/oversized source: " + name)
    with path.open("rb") as stream:
        data = stream.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        raise ValueError("source byte budget exceeded: " + name)
    return data


def _tool(path, expected):
    path = _absolute_unaliased(path)
    if not path.is_file() or path.stat().st_size > 128 * MAX_BYTES:
        raise ValueError("tool must be an explicit regular absolute path")
    data = path.read_bytes()
    if _sha(data) != expected:
        raise ValueError("tool bytes differ: " + str(path))
    return _ref(str(path), data)


def _load_driver():
    spec = importlib.util.spec_from_file_location("_composition_c07_driver", BASE / "driver.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _node(node, typescript, helper, source, name, mode):
    result = subprocess.run([str(node), str(BASE / helper), str(typescript), name, mode],
                            input=source, capture_output=True, timeout=20)
    if len(result.stdout) > MAX_BYTES or len(result.stderr) > MAX_BYTES:
        raise ValueError("reader output byte budget exceeded")
    if result.returncode:
        raise ValueError(result.stderr.decode("utf-8", errors="replace").strip())
    return json.loads(result.stdout)


def observe_recipe(source_root, output, *, mode, node, node_sha256, typescript,
                   typescript_sha256):
    """Exercise real production projection and return observations for this source slice.

    This recipe recognizes only the selected Node image, formal SDK parser and
    source versions, and has no execution fallback or caller repinning bypass.
    `disabled` means a fresh ordinary selection, never in-place runtime unload.
    """
    if mode not in ("ordinary", "enhanced", "disabled"):
        raise ValueError("unknown finite ArkTS recipe")
    if typescript_sha256 != TYPESCRIPT_SHA256:
        raise ValueError("unsupported SDK compiler bytes")
    if node_sha256 != NODE_SHA256:
        raise ValueError("unsupported Node image bytes")
    root, output = _directory(source_root), _directory(output, new=True)
    _separate(root, output)
    tools = (_tool(node, node_sha256), _tool(typescript, typescript_sha256))
    version = subprocess.run([str(node), "--version"], capture_output=True, timeout=5, check=True)
    if version.stdout.decode().strip() != "v" + (ROOT / ".node-version").read_text().strip():
        raise ValueError("unsupported Node version")
    selected = (FACADE, FRAME, POLICY, NATIVE, AUTHORITY) + ((STATE, UI) if mode == "enhanced" else ())
    sources = {name: _read(root, name) for name in selected}
    for name, data in sources.items():
        if _sha(data) != SOURCE_PINS[name]:
            raise ValueError("unsupported fixed SDK source bytes: " + name)
    inputs = tuple(_ref(name, data) for name, data in sorted(sources.items()))
    refs = {item.path: item for item in inputs}
    implementation = tuple(_ref(name, _read(ROOT, name)) for name in IMPLEMENTATION)
    driver = _load_driver()
    generated = driver.render_template(sources[FACADE], "libmadbc07sdk.so",
                                       "madb-declaration-projection-not-runtime")
    public = _node(node, typescript, "read_facade.cjs", generated,
                   "generated/facade/Boundary.ets", "source")
    policy = json.loads(sources[POLICY])
    if FileObservation(**policy["authority"]) != refs[AUTHORITY]:
        raise ValueError("authored policy authority differs from frozen source")
    if driver.lowering.projection(public) != policy["public_surface"]:
        raise ValueError("actual C07 public declarations differ from authored policy")
    calls = public["functions"] + [m for owner in public["resources"] for m in owner["methods"]]
    names = {".".join(filter(None, (call["owner"], call["name"]))) for call in calls}
    if names != set(policy["calls"]):
        raise ValueError("actual C07 call policy closure differs")
    roles = policy["calls"]
    for name, expected in (("Engine.roundtrip", "sync"), ("Engine.observeState", "stream"),
                           ("Subscription.requestStop", "sync"), ("Subscription.close", "async")):
        if name not in names or roles[name]["kind"] != expected:
            raise ValueError("C07 recipe role is absent or has wrong kind: " + name)
    public_id = lambda name: roles[name]["public_id"]
    roundtrip, observe, stop, close = map(public_id, ("Engine.roundtrip", "Engine.observeState",
                                                    "Subscription.requestStop", "Subscription.close"))
    facade_output = "generated/facade/Boundary.ets"
    required = [
        RequiredRole("public-symbol", roundtrip, "projection", "call", "public-symbol", roundtrip,
                     "facade", refs[FACADE], facade_output),
        RequiredRole("public-symbol", observe, "subscription", "subscription", "public-symbol", observe,
                     "source", refs[NATIVE]),
        RequiredRole("public-symbol", stop, "stop", "subscription", "public-symbol", observe,
                     "source", refs[NATIVE]),
        RequiredRole("public-symbol", close, "drain", "subscription", "public-symbol", observe,
                     "source", refs[NATIVE]),
        RequiredRole("public-symbol", close, "release", "subscription", "public-symbol", observe,
                     "facade", refs[FACADE], facade_output),
    ]
    assets = {facade_output: generated, "generated/facade/RuntimeFrame.ets": sources[FRAME]}
    if mode == "enhanced":
        for source, sdk_mode in ((STATE, "state"), (UI, "ui")):
            _node(node, typescript, "read_observation.cjs", sources[source], source, sdk_mode)
            assets["generated/facade/" + Path(source).name] = sources[source]
        for member, stage, source, layer in (
            ("StateObservation.requestStop", "stop", STATE, "receiver"),
            ("StateObservation.close", "async-bridge", STATE, "receiver"),
            ("ArkUIStateModel.attach", "projection", UI, "view-model"),
        ):
            required.append(RequiredRole("sdk-export", member, stage, "receiver", "sdk-export",
                                         member.split(".")[0] + ".attach", layer, refs[source]))
    # All local imports in the fixed source versions resolve within this exact
    # tree. The unchanged native module is an explicit external dependency.
    output.mkdir()
    for name, data in sorted(assets.items()):
        destination = output / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        with destination.open("xb") as stream:
            stream.write(data)
    actual = _node(node, typescript, "read_facade.cjs", _read(output, facade_output),
                   facade_output, "source")
    if driver.lowering.projection(actual) != driver.lowering.projection(public):
        raise ValueError("generated facade changed the real public declaration")
    for source, sdk_mode in ((STATE, "state"), (UI, "ui")) if mode == "enhanced" else ():
        name = "generated/facade/" + Path(source).name
        _node(node, typescript, "read_observation.cjs", _read(output, name), name, sdk_mode)
    result = RecipeObservation("madb.c07-c-arkts/" + ("arkui-v2@1" if mode == "enhanced" else "ordinary@1"),
                               inputs, implementation, tools, tuple(required),
                               tuple(_ref(name, data) for name, data in sorted(assets.items())))
    verify_observation(result, root, output)
    return result


def verify_observation(observation, source_root, output):
    """Recheck captured source/tool/output bytes; grants no artifact acceptance."""
    source_root, output = _directory(source_root), _directory(output)
    _separate(source_root, output)
    for item in observation.inputs:
        if _ref(item.path, _read(source_root, item.path)) != item:
            raise ValueError("source changed after observation: " + item.path)
    for item in observation.implementation:
        if _ref(item.path, _read(ROOT, item.path)) != item:
            raise ValueError("recipe implementation changed after observation")
    for item in observation.tools:
        if _tool(item.path, item.sha256.removeprefix("sha256:")) != item:
            raise ValueError("tool changed after observation")
    allowed = {item.path for item in observation.outputs}
    directories = {str(parent) for name in allowed for parent in Path(name).parents if str(parent) != "."}
    def check(folder, prefix=""):
        for child in folder.iterdir():
            name = prefix + child.name
            if child.is_symlink():
                raise ValueError("symbolic output path: " + name)
            if name in directories and child.is_dir():
                check(child, name + "/")
            elif name not in allowed or not child.is_file():
                raise ValueError("undeclared output: " + name)
    check(Path(output))
    for item in observation.outputs:
        if _ref(item.path, _read(output, item.path)) != item:
            raise ValueError("output changed after observation: " + item.path)

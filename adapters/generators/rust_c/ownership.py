"""Render the finite SDK source responsibilities; C04 checks the expected bytes."""
import tomllib

from lowering import DIAGNOSTIC_ROLES, ROLES, diagnostics_enabled
from support import NativeError, digest, input_path, load_json, semantic


READER = "org.madb.rust-c/reader-generator@0.1.0"
SDK = "madb-first-route-engine@0.1.0"
SOURCE_PREFIX = "sdk/rust/engine/"


def _escape(value):
    return str(value).replace("\\", "\\\\").replace("|", "\\|").replace("`", "\\`").replace("\r", " ").replace("\n", " ")


def _row(*cells):
    return "| " + " | ".join(_escape(cell) for cell in cells) + " |\n"


def _reference(reference):
    return {key: reference[key] for key in ("path", "sha256", "size")}


def _read(reference, input_root):
    path = input_path(input_root, reference["path"])
    data = path.read_bytes()
    if len(data) != reference["size"] or digest(data) != reference["sha256"]:
        raise NativeError("NATIVE006", "responsibility source reference changed: " + reference["path"])
    return path


def _origin(origin):
    location = origin["location"]
    return f"{origin['authority']}:{origin['id']}@{location['path']}:{location['line']}:{location['column']}"


def _value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (str, int)):
        return str(value)
    raise NativeError("NATIVE001", "unsupported finite responsibility policy value")


def _slot(policy):
    slot = policy["slot"]
    return "parameter:" + slot["name"] if slot["kind"] == "parameter" else slot["kind"]


def render_ownership(contract, request, config, input_root):
    """Use actual checked context, never a caller-supplied responsibility list."""
    if (config.get("provider"), config.get("semantics"), config.get("source")) != (
            "rust", "madb.first-route/1", SOURCE_PREFIX + "src/lib.rs"):
        return None
    if request.get("component", {}).get("id") != "org.madb.rust-c/reader-generator":
        return None
    diagnostics = diagnostics_enabled(config)
    lock = load_json(_read(request["lock"], input_root))
    profile = load_json(_read(request["profile"], input_root))
    selector = contract["selector"]
    if (profile.get("schema"), profile.get("route"), profile.get("provider"), profile.get("consumer"), profile.get("direction")) != (
            "madb.profile/1", "RUST-C-ABI", {"language": "rust", "runtime": "native"},
            {"language": "c", "runtime": "native"}, "consumer-to-provider") or selector != request["selector"] or (
            profile["id"], profile["target"], profile["direction"], profile["route"]) != (
            selector["profile_id"], selector["target"], selector["direction"], selector["route"]):
        raise NativeError("NATIVE006", "responsibility profile differs from its checked selector")
    component = request["component"]
    if (component["id"], component["version"]) != ("org.madb.rust-c/reader-generator", "0.1.0") or component not in lock["components"]:
        raise NativeError("NATIVE006", "responsibility reader component differs from the lock")
    inputs = {item["path"]: _reference(item) for item in request["inputs"]}
    locked = {item["path"]: _reference(item) for item in lock["inputs"]}
    if len(inputs) != len(request["inputs"]) or len(locked) != len(lock["inputs"]) or inputs != locked:
        raise NativeError("NATIVE006", "responsibility source inventory differs from the lock")
    minimum = {SOURCE_PREFIX + name for name in ("Cargo.toml", "src/lib.rs", "src/runtime.rs", "runtime-protocol.json", "cbindgen.toml")}
    if diagnostics:
        minimum |= {SOURCE_PREFIX + name for name in ("src/diagnostics.rs", "cbindgen-diagnostics.toml")}
    if not (minimum | {"backend.json"}) <= inputs.keys():
        raise NativeError("NATIVE006", "responsibility SDK source closure is incomplete")
    if load_json(_read(inputs["backend.json"], input_root)) != config:
        raise NativeError("NATIVE006", "responsibility backend differs from its frozen bytes")
    sdk = profile["sdk"]
    if inputs.get(sdk["path"]) != _reference(sdk):
        raise NativeError("NATIVE006", "responsibility SDK metadata is not a frozen source")
    toolchain = load_json(_read(sdk, input_root))
    if sdk.get("schema") != "madb.native-toolchain/1" or toolchain.get("schema") != "madb.native-toolchain/1":
        raise NativeError("NATIVE006", "responsibility SDK metadata has an unsupported schema")
    protocol = load_json(_read(inputs[SOURCE_PREFIX + "runtime-protocol.json"], input_root))
    if (protocol.get("schema"), protocol.get("component"), protocol.get("version"), protocol.get("provider"), protocol.get("direction")) != (
            "madb.native-runtime-protocol/1", "madb-first-route-engine", "0.1.0", "rust", "c-to-rust") or protocol != toolchain.get("runtime_protocol"):
        raise NativeError("NATIVE006", "responsibility SDK protocol differs from its metadata")
    native = [item for path, item in sorted(inputs.items()) if path.startswith(SOURCE_PREFIX)]
    for item in native:
        _read(item, input_root)
    package = tomllib.loads(_read(inputs[SOURCE_PREFIX + "Cargo.toml"], input_root).read_text())["package"]
    if (package["name"], package["version"]) != ("madb-first-route-engine", "0.1.0"):
        raise NativeError("NATIVE006", "responsibility SDK package identity differs")
    exports = {item["role"]: item for item in config["exports"]}
    if len(exports) != len(config["exports"]) or set(exports) != (ROLES | DIAGNOSTIC_ROLES if diagnostics else ROLES):
        raise NativeError("NATIVE001", "responsibility roles differ from the selected ABI recipe")
    symbols = {item["public_id"]: item for item in contract["symbols"]}
    if len(symbols) != len(contract["symbols"]) or len(symbols) != len(exports) or set(symbols) != {item["public_id"] for item in exports.values()}:
        raise NativeError("NATIVE006", "responsibility symbols differ from the checked ABI roles")
    rows = []
    for export in exports.values():
        public_id = export["public_id"]
        shape = symbols[public_id]["shape"]
        if (shape["kind"], shape["calling_convention"], shape["link_name"]) != ("sync", "c", export["name"]):
            raise NativeError("NATIVE006", "responsibility symbol differs from its ABI role")
        rows.extend([(public_id, "type-read", "c-declaration", READER),
                     (public_id, "abi-lowering", "c-to-logical", READER),
                     (public_id, "projection", "call-frame", SDK)])
    for role, stage, scope in (("observe", "subscription", "returned-subscription"), ("observe", "dispatch", "source-lease"),
            ("request_close", "stop", "argument-handle"), ("await_closed", "drain", "argument-handle"), ("release_handle", "release", "closed-token")):
        rows.append((exports[role]["public_id"], stage, scope, SDK))
    rows.append(("helper:madb_engine_create_checked", "projection", "identity-gate", "include/madb_rust_client.h via " + READER))
    output = "# C-to-Rust mechanism responsibilities\n\nFinite generated source evidence; not runtime certification.\n\n"
    output += f"- Boundary: `{_escape(contract['boundary_id'])}`\n"
    output += f"- Profile: `{_escape(selector['profile_id'])}` (`{_escape(selector['profile_digest'])}`)\n"
    for label, field in (("Source", "source_digest"), ("Structure", "structure_digest"), ("Policy", "policy_digest")):
        output += f"- {label} digest: `{_escape(contract[field])}`\n"
    output += f"- Reader/generator: `{READER}` (`{_escape(component['sha256'])}`)\n- Native SDK: `{SDK}`\n"
    output += f"- Native source digest: `{semantic('madb.source/1', native)}`\n"
    output += f"- SDK metadata: `{_escape(sdk['path'])}` (`{_escape(sdk['sha256'])}`, {sdk['size']} bytes)\n"
    output += "\n## Responsibilities\n\n| Subject | Stage | Scope | Owner |\n| --- | --- | --- | --- |\n"
    for row in sorted(rows, key=lambda item: tuple(value.encode() for value in item[:3])):
        output += _row(*row)
    output += "\n## Effective policy inputs\n\n| Symbol | Slot | Values | Origins |\n| --- | --- | --- | --- |\n"
    for policy in sorted(contract["policies"], key=lambda item: (item["symbol_id"].encode(), _slot(item).encode())):
        values = "; ".join(name + "=" + _value(value) for name, value in sorted(policy["values"].items()))
        origins = []
        for item in sorted(policy["provenance"], key=lambda item: item["property"]):
            text = item["property"] + "=" + _origin(item["origin"])
            if item.get("replaced_default") is not None:
                text += " (replaces " + _origin(item["replaced_default"]) + ")"
            origins.append(text)
        output += _row(policy["symbol_id"], _slot(policy), values, "; ".join(origins))
    output += "\n## Frozen native sources\n\n| Path | SHA-256 | Bytes |\n| --- | --- | --- |\n"
    for item in native:
        output += _row(item["path"], item["sha256"], item["size"])
    output += """
## Handoffs and limits

- Synchronous calls return one status and initialized outputs; no async bridge or cancellation owner is claimed.
- The SDK borrows input only through the call. Caller memory validity and callback/context lifetime remain caller preconditions.
- SourceLease owns callback registration and serialized source-caller delivery; no consumer queue, timer or fixed OS thread is implied.
- request_close stops admission; await_closed confirms drain; release_handle removes only a Closed token. Native cleanup precedes Closed.
- The checked helper owns identity comparison only and delegates bootstrap/create to the same native owners; it adds no subscription or diagnostic event.
- Diagnostic pull returns copied values and never owns the caller exporter, business resources or a second lifecycle state machine.
- Physical assembly, actual runtime behavior, independent Lab acceptance and certification require their separate existing evidence.
"""
    return output

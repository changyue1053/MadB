"""Reuse C06's exact Apple host linker/SDK observations for Rust proc macros."""

from __future__ import annotations
import importlib.util
from pathlib import Path

_ROOT = Path(__file__).resolve().parents[3]
_SPEC = importlib.util.spec_from_file_location(
    "_madb_rust_support", _ROOT / "adapters/generators/arkts/support.py"
)
support = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(support)
managed = support.load_module(_ROOT / "adapters/toolchains/kotlin_rust/tools.py")


def observe():
    recipe = _ROOT / "adapters/toolchains/kotlin_rust/toolchain.json"
    apple = support.read_json(recipe)["xcode"]
    if (
        managed.query("/usr/bin/xcodebuild", "-version")
        != "Xcode " + apple["version"] + "\nBuild version " + apple["build"]
    ):
        raise ValueError(
            "Rust host Xcode differs from the existing reviewed C06 toolchain"
        )
    if (
        managed.query("/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-version")
        != apple["sdk_version"]
        or managed.query(
            "/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-build-version"
        )
        != apple["sdk_build"]
    ):
        raise ValueError("Rust host Apple SDK differs from the reviewed toolchain")
    developer = Path(managed.query("/usr/bin/xcode-select", "-p")).resolve(strict=True)
    sdk = Path(
        managed.query("/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path")
    ).resolve(strict=True)
    result = dict(
        xcode=apple,
        tools={},
        trees={},
        developer=str(developer),
        sdk=str(sdk),
        recipe=dict(
            path=recipe.relative_to(_ROOT).as_posix(),
            sha256=support.sha(recipe),
            size=recipe.stat().st_size,
        ),
    )
    for key in ("clang", "ar", "ld"):
        image = Path(managed.query("/usr/bin/xcrun", "--find", key)).resolve(
            strict=True
        )
        if not image.is_relative_to(developer):
            raise ValueError(
                "Rust host executable escaped the reviewed Developer directory"
            )
        managed.register_image(
            result, key, image, apple["build"], "xcode://" + apple["build"]
        )
    managed.register_tree(result, "apple-sdk", sdk)
    managed.register_tree(
        result,
        "apple-toolchain-libraries",
        Path(result["tools"]["clang"]["path"]).parent.parent / "lib",
    )
    return result


def verify(config):
    host = config["rust"]["host"]
    recipe = _ROOT / host["recipe"]["path"]
    support.verify_refs(_ROOT, [host["recipe"]])
    if host["xcode"] != support.read_json(recipe)["xcode"] or set(host["tools"]) != {
        "clang",
        "ar",
        "ld",
    }:
        raise ValueError(
            "Rust host selection differs from the frozen existing Apple profile"
        )
    if set(host["trees"]) != {"apple-sdk", "apple-toolchain-libraries"}:
        raise ValueError("Rust host SDK/linker library closure is incomplete")
    for tool in host["tools"].values():
        support.verify_tool(tool)
    for tree in host["trees"].values():
        if managed.snapshot(tree["root"]) != tree:
            raise ValueError("Rust host Apple SDK/linker library tree changed")
    return host


def environment(config):
    # Verified by the operation's pre/post closure gate. Never consult current
    # xcode-select at execution time; target OHOS sysroot stays separately fixed.
    host = config["rust"]["host"]
    directory = str(Path(host["tools"]["clang"]["path"]).parent)
    return dict(
        SDKROOT=host["sdk"],
        DEVELOPER_DIR=host["developer"],
        MACOSX_DEPLOYMENT_TARGET="14.0",
        CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=host["tools"]["clang"]["path"],
        CC_aarch64_apple_darwin=host["tools"]["clang"]["path"],
        AR_aarch64_apple_darwin=host["tools"]["ar"]["path"],
        PATH=str(Path(config["tools"]["cargo"]["path"]).parent)
        + ":"
        + directory
        + ":/usr/bin:/bin:/usr/sbin:/sbin",
    )

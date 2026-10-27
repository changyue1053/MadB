# Bundled Rust/C values template

This is the first internal `org.madb.template/rust-c-sdk@0.1.0` package. The
compiled CLI catalog pins `package.tar` and `source/meta/template.json` separately.
`source/` owns editable declarations; `package.tar` and `identity.json` are
deterministic derivatives. Run `python3 scripts/build_init_template.py` after
editing, or `--check` to verify exact regeneration. Any subsequent shipped
package-byte change requires a new template version and catalog binding.

The current executable supports structure initialization and dry-run. Build,
run and IDE actions are rejected before creation. The profile/toolchain original
bytes come from the approved I01 values design input; they are a bound source
snapshot, not evidence that those executable images are installed or executed.
This template does not replace the C05 resource recipe or the I01 IDE exits.

"""Build providers from frozen source under a cold, input-addressed compiler root."""
from pathlib import Path
import hashlib

from .worker_support import canonical, copy_file, file_ref, input_path, load
from .tools import load_verifier
from .smoke_workspace import _CompilerCommands, _workspace_module

WORKSPACE = {
    'root': '/private/tmp/madb-compiler/c06-rust',
    'strategy': 'input-addressed-exclusive-cold-build',
}


def build_provider(config, inputs, commands, *, profile=None, test_support=False):
    """Cargo package identity includes its path even with source-path remapping.

    Compile from the same owned cold path for equal inputs. Retain Cargo's normal
    metadata, the linker's UUID, and every resulting byte; no compiler wrapper or
    post-link normalization is involved.
    """
    if config.get('rust_compiler_workspace') != WORKSPACE:
        raise ValueError('Rust providers require the declared stable compiler workspace')
    identity = load(input_path(inputs, 'identity.json'))
    selected_profile = profile or identity['profile_digest']
    names = ['identity.json']
    names.extend(path.relative_to(inputs).as_posix()
                 for path in sorted((inputs/'sdk/rust').rglob('*')) if path.is_file())
    references = [file_ref(input_path(inputs, name), inputs) for name in names]
    for directory in ('adapters/toolchains/kotlin_rust', 'adapters/toolchains/common'):
        references.extend(file_ref(path, inputs) for path in sorted((inputs/directory).rglob('*.py')))
    options = canonical(dict(variant=config['variant'], test_support=test_support,
                             deployment_target=config['deployment_target']))
    references.append(dict(path='@rust-provider-options', size=len(options),
                           sha256='sha256:' + hashlib.sha256(options).hexdigest()))
    workspace = _workspace_module()
    key = workspace.identity(references, config['toolchain'], selected_profile, identity['interface_digest'])
    with workspace.acquire(key, base=Path(WORKSPACE['root'])) as stage:
        project = stage/'project'
        frozen, output, scratch = project/'inputs', project/'output', project/'scratch'
        for directory in (frozen, output, scratch):
            directory.mkdir(parents=True)
        for name in names:
            copy_file(input_path(inputs, name), frozen/name)
        compiler = _CompilerCommands(config, frozen, output, scratch, stage, workspace)
        provider = _compile_provider(config, frozen, compiler, selected_profile, test_support)
        destination = commands.scratch/'providers'/key/provider.name
        copy_file(provider, destination)
        commands.lock_derivation = compiler.lock_derivation
        data = compiler.log.read_bytes()
        if commands.log.stat().st_size + len(data) > 4 * 1024 * 1024:
            raise ValueError('Rust compiler evidence exceeds 4 MiB')
        with commands.log.open('ab') as stream:
            stream.write(data)
        return destination


def _compile_provider(config, inputs, commands, profile, test_support):
    observation = config['toolchain']
    gobley = config['variant'] == 'kotlin-native-rust'
    source = commands.scratch/'source'
    for path in (inputs/'sdk/rust').rglob('*'):
        if path.is_file():
            name = path.relative_to(inputs).as_posix()
            copy_file(input_path(inputs, name), source/name)
    crate = 'gobley' if gobley else 'uniffi'
    library = 'libmadb_boundary_' + crate + '.dylib'
    commands.lock_derivation = load_verifier(crate).derive_lock(source/'sdk/rust'/crate/'Cargo.lock')
    target = commands.scratch/'cargo-target'
    cargo_home = commands.scratch/'cargo-home'
    cargo_home.mkdir()
    vendor = Path(observation['trees']['cargo-vendor']['root'])
    runtime = (Path(observation['trees']['gobley-runtime']['root']) if gobley else
               Path(observation['trees']['uniffi-managed-sources']['root'])/'uniffi_core-0.32.2')
    arguments = [
        'build', '--offline', '--locked', '--release', '--lib',
        '--manifest-path', source/'sdk/rust'/crate/'Cargo.toml',
        '--target', 'aarch64-apple-darwin', '--target-dir', target,
        '--config', 'source.crates-io.replace-with="madb-vendor"',
        '--config', f'source.madb-vendor.directory="{vendor}"',
        '--config', f'patch.crates-io.uniffi_core.path="{runtime}"',
    ]
    if not gobley:
        bindgen = Path(observation['trees']['uniffi-managed-sources']['root'])/'uniffi_bindgen-0.32.2'
        arguments += ['--config', f'patch.crates-io.uniffi_bindgen.path="{bindgen}"']
    if test_support:
        arguments += ['--features', 'test-support']
    flags = ['--remap-path-prefix=' + str(path) + '=/madb/' + ('source' if path == source else 'build')
             for path in (source, commands.scratch, target, runtime)]
    flags += ['-C', 'link-arg=-Wl,-install_name,@rpath/' + library,
              '-C', 'link-arg=-Wl,-reproducible']
    commands.run('cargo', arguments, extra={
        'CARGO_HOME': str(cargo_home), 'CARGO_NET_OFFLINE': 'true',
        'CARGO_ENCODED_RUSTFLAGS': '\x1f'.join(flags), 'RUSTFLAGS': '',
        'MADB_PROFILE_DIGEST': profile,
    })
    return target/'aarch64-apple-darwin/release'/library

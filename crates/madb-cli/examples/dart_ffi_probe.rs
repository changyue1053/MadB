//! Internal finite E04 composition root. Public ports do all admission; this
//! executable is not a product command, package registry or acceptance token.
#[path = "dart_ffi_probe/build_chain.rs"]
mod build_chain;
#[path = "dart_ffi_probe/chain.rs"]
mod chain;
#[path = "dart_ffi_probe/inputs.rs"]
mod inputs;
#[path = "dart_ffi_probe/package_chain.rs"]
mod package_chain;
#[path = "dart_ffi_probe/recipe.rs"]
mod recipe;

use madb_contracts::{DocumentKind, SchemaRegistry, composition, native};
use std::{
    io::{Read, Write},
    path::Path,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("package") {
        return package_chain::run(&args[1..]);
    }
    if args.get(1).map(String::as_str) == Some("build") {
        return build_chain::run(&args[1..]);
    }
    if args.get(1).map(String::as_str) == Some("chain") {
        return chain::run(&args[1..]);
    }
    if args.len() != 3 {
        return Err("expected metadata|composition-digest|profile-digest INPUT_ROOT".into());
    }
    let root = Path::new(&args[2]);
    let registry = SchemaRegistry::new()?;
    match args[1].as_str() {
        "verify" => {
            let input = inputs::Inputs::load(root)?;
            input.bind(&recipe::requirements())?;
            input.verify()?;
            println!("verified finite input observation");
        }
        "composition-digest" | "profile-digest" => {
            let (name, kind) = if args[1] == "composition-digest" {
                ("composition.json", DocumentKind::Composition)
            } else {
                ("profile.json", DocumentKind::ProfileV2)
            };
            let workspace = madb_host::LocalWorkspace::open(root)?;
            let bytes = inputs::read(&workspace, name, inputs::LIMIT)?;
            let document = registry.decode(kind, &bytes)?;
            println!(
                "{}",
                if kind == DocumentKind::Composition {
                    composition::composition_digest(&document)?
                } else {
                    composition::profile_v2_digest(&document)?
                }
            );
        }
        "metadata" => {
            let input = inputs::Inputs::load(root)?;
            let mut seed = Vec::new();
            std::io::stdin()
                .take(inputs::LIMIT as u64 + 1)
                .read_to_end(&mut seed)?;
            if seed.len() > inputs::LIMIT {
                return Err("metadata exceeds public document limit".into());
            }
            let requirements = recipe::requirements();
            let bound = input.bind(&requirements)?;
            let complete =
                native::finalize_composition_metadata(&seed, input.context(), &requirements)?;
            let ir = input.registry.decode(DocumentKind::ContractIr, &complete)?;
            recipe::check(&input, &bound, &ir)?;
            input.verify()?;
            std::io::stdout().write_all(&complete)?;
        }
        _ => return Err("unsupported probe operation".into()),
    }
    Ok(())
}

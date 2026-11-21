//! Derives the guest's plant target filter from the native predictor's supported catalog.
use std::{env, fs, path::PathBuf};
use syn::{Expr, Item, Lit};

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let source = manifest.join("../../../crates/gameplay/src/placement_plants.rs");
    println!("cargo:rerun-if-changed={}", source.display());
    let parsed = syn::parse_file(&fs::read_to_string(&source).unwrap()).unwrap();
    let mut names = Vec::new();
    let mut groups = 0;
    for item in parsed.items {
        let Item::Const(item) = item else { continue };
        if !["SINGLE", "DOUBLE", "SAPLINGS"]
            .iter()
            .any(|name| item.ident == *name)
        {
            continue;
        }
        groups += 1;
        let Expr::Reference(reference) = item.expr.as_ref() else {
            panic!("plant catalog must remain an explicit checked array");
        };
        let Expr::Array(array) = reference.expr.as_ref() else {
            panic!("plant catalog must remain an explicit checked array");
        };
        for entry in &array.elems {
            let Expr::Lit(value) = entry else {
                panic!("nonliteral plant identifier")
            };
            let Lit::Str(value) = &value.lit else {
                panic!("non-string plant identifier")
            };
            names.push(format!("minecraft:{}", value.value()));
        }
    }
    assert_eq!(
        groups, 3,
        "native plant catalog changed; update the guest generator"
    );
    names.sort();
    names.dedup();
    let generated = format!("const SOIL_PLANT_IDENTIFIERS: &[&str] = &{names:?};\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("soil_plants.rs"),
        generated,
    )
    .unwrap();
    world_states(&manifest);
}

/// Copies the native placement's world-controlled state names, so the guest ignores the same
/// properties when it offers a placement and checks the built guide.
fn world_states(manifest: &std::path::Path) {
    let source = manifest.join("../../../crates/gameplay/src/placement_world_states.rs");
    println!("cargo:rerun-if-changed={}", source.display());
    let parsed = syn::parse_file(&fs::read_to_string(&source).unwrap()).unwrap();
    let names: Vec<String> = parsed
        .items
        .iter()
        .find_map(|item| match item {
            Item::Const(item) if item.ident == "WORLD_CONTROLLED_NAMES" => Some(item),
            _ => None,
        })
        .map(|item| {
            let Expr::Reference(reference) = item.expr.as_ref() else {
                panic!("world-controlled names must remain an explicit array");
            };
            let Expr::Array(array) = reference.expr.as_ref() else {
                panic!("world-controlled names must remain an explicit array");
            };
            array
                .elems
                .iter()
                .map(|entry| match entry {
                    Expr::Lit(value) => match &value.lit {
                        Lit::Str(value) => value.value(),
                        _ => panic!("non-string world-controlled name"),
                    },
                    _ => panic!("nonliteral world-controlled name"),
                })
                .collect()
        })
        .expect("native world-controlled state names");
    assert!(!names.is_empty());
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("world_states.rs"),
        format!("pub(crate) const WORLD_CONTROLLED_STATES: &[&str] = &{names:?};\n"),
    )
    .unwrap();
}

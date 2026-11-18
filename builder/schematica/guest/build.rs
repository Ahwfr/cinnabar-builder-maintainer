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
}

//! Finds the `similar` source that Cargo fetched, for `main.rs` to
//! fingerprint: `src/` ports it by hand.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let mut sources = String::new();
    let dir = upstream_dir("similar");
    println!("cargo:rustc-env=UPSTREAM_DIR={}", dir.display());
    for file in [
        "src/common.rs",
        "src/types.rs",
        "src/iter.rs",
        "src/udiff.rs",
        "src/utils.rs",
        "src/algorithms/mod.rs",
        "src/algorithms/myers.rs",
        "src/algorithms/lcs.rs",
        "src/algorithms/patience.rs",
        "src/algorithms/compact.rs",
        "src/algorithms/replace.rs",
        "src/algorithms/capture.rs",
        "src/algorithms/utils.rs",
        "src/text/mod.rs",
        "src/text/abstraction.rs",
        "src/text/inline.rs",
        "src/text/utils.rs",
    ] {
        let path = dir.join(file);
        sources.push_str(
            &std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display())),
        );
        println!("cargo:rerun-if-changed={}", path.display());
    }
    std::fs::write(out.join("sources.rs.txt"), sources).unwrap();
    println!("cargo:rerun-if-changed=Cargo.toml");
}

/// Where Cargo put the package `name` this build depends on.
fn upstream_dir(name: &str) -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let manifest = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("Cargo.toml");
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(&manifest)
        .output()
        .expect("could not run `cargo metadata`");
    assert!(out.status.success(), "`cargo metadata` failed");
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let pkg = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("{name} is not among the dependencies"));
    Path::new(pkg["manifest_path"].as_str().unwrap())
        .parent()
        .unwrap()
        .to_path_buf()
}

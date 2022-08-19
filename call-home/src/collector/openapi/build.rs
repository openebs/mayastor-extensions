use std::{env, process::Command};

fn main() {
    let current_dir = env::current_dir().unwrap();
    let dir = current_dir.to_str().unwrap();
    let command = format!("../../../mayastor-control-plane/scripts/rust/generate-openapi-bindings.sh --skip-md5-same --skip-git-diff --target-dir={}",dir);
    let output = Command::new("bash")
        .args(&["-c", command.trim()])
        .output()
        .expect("failed to execute bash command");

    if !output.status.success() {
        panic!("openapi update failed: {:?}", output);
    }

    println!("cargo:rerun-if-changed=../nix/pkgs/openapi-generator");
    println!("cargo:rerun-if-changed=../mayastor-control-plane/control-plane/rest/openapi-specs");
    // seems the internal timestamp is taken before build.rs runs, so we can't set this
    // directive against files created during the build of build.rs??
    // https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
    // println!("cargo:rerun-if-changed=.");
}

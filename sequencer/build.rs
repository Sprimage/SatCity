use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../circuits/src");

    let status = Command::new("scarb")
        .arg("build")
        .current_dir("../circuits")
        .status()
        .expect("Failed to execute scarb build");

    if !status.success() {
        panic!("scarb build failed");
    }
}
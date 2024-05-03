use std::process::Command;

fn main() {
    if let Ok(hash) = git_hash() {
        println!("cargo:rustc-env=GIT_HASH={}", hash);
    }

    println!("cargo:rerun-if-changed=../boulder/src");
    println!("cargo:rerun-if-changed=../Cargo.lock");
}

fn git_hash() -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output()?;
    Ok(String::from_utf8(output.stdout)?)
}

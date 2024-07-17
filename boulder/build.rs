use etc_os_release::OsRelease;
use std::error::Error;

/// Set cargo::rustc-cfg=os_release_id="whatever" when /etc/os-release warrants it.
/// The intent is to enable trivial conditional compilation via [target.'cfg(...)']
/// stanzas.
fn main() -> Result<(), Box<dyn Error>> {
    // only recompile when necessary
    println!("cargo::rerun-if-changed=./build.rs");
    // if /etc/os-release doesn't exist, we have a problem big enough that it's OK to crash
    let os_release = OsRelease::open()?;
    println!("cargo::rustc-cfg=os_release_id=\"{}\"", os_release.id());

    Ok(())
}

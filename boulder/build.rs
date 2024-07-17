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
    // this is only here for visibility during compilation
    let os_release_id = os_release.id();
    println!("cargo::rustc-cfg=os_release_id=\"{}\"", os_release_id);

    match os_release_id {
        "solus" => advanced_link_args(),
        "fedora" | "serpentos" => default_link_args(),
        _ => conservative_link_args(),
    };

    Ok(())
}

fn advanced_link_args() {
    println!("-fuse-ld=lld");
    println!("-Wl,--compress-debug-sections=zstd");
    println!("-Csymbol-mangling-version=v0");
}

fn default_link_args() {
    println!("-fuse-ld=lld");
    //println!("-Wl,--compress-debug-sections=zstd");
    println!("-Csymbol-mangling-version=v0");
}

fn conservative_link_args() {
    println!("-fuse-ld=lld");
    //println!("-Wl,--compress-debug-sections=zstd");
    //println!("-Csymbol-mangling-version=v0");
}

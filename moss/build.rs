extern crate varlink_generator;

fn main() {
    println!("cargo:rerun-if-changed=src/varlink/com.serpentos.moss.varlink");
    varlink_generator::cargo_build_tosource("src/varlink/com.serpentos.moss.varlink", true);
}

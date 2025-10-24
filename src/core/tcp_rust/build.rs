use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    // Get the lwIP source directory
    let lwip_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("../..")
        .join("..");

    let include_dir = lwip_dir.join("src/include");
    let unix_port_include = lwip_dir.join("contrib/ports/unix/port/include");
    let unix_lib_include = lwip_dir.join("contrib/ports/unix/lib");

    println!("cargo:rustc-link-search={}/build", lwip_dir.display());

    // Generate bindings for lwIP C headers
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_dir.display()))
        .clang_arg(format!("-I{}", unix_port_include.display()))
        .clang_arg(format!("-I{}", unix_lib_include.display()))
        // Allowlist only what we need
        .allowlist_type("pbuf")
        .allowlist_type("pbuf_layer")
        .allowlist_type("pbuf_type")
        .allowlist_type("netif")
        .allowlist_type("ip_addr")
        .allowlist_type("ip_addr_t")
        .allowlist_type("ip4_addr")
        .allowlist_type("ip4_addr_t")
        .allowlist_type("ip6_addr")
        .allowlist_type("ip6_addr_t")
        .allowlist_function("pbuf_alloc")
        .allowlist_function("pbuf_free")
        .allowlist_function("pbuf_header")
        .allowlist_function("pbuf_remove_header")
        .allowlist_function("pbuf_realloc")
        .allowlist_function("mem_malloc")
        .allowlist_function("mem_free")
        .allowlist_function("ip_output_if")
        .allowlist_function("ip4_output_if")
        .allowlist_function("ip6_output_if")
        .allowlist_function("ip_chksum_pseudo")
        .allowlist_var("PBUF_.*")
        .allowlist_var("IP_PROTO_TCP")
        // Generate with useful derivations
        .derive_debug(true)
        .derive_default(true)
        // Use core instead of std for no_std compatibility
        .use_core()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

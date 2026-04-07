use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/jax/bridge.cc");
    println!("cargo:rerun-if-changed=include/jax_bridge.h");
    
    // Configure JAX/XLA bridge
    let jax_home = env::var("JAX_HOME").unwrap_or_else(|_| "/opt/jax".to_string());
    println!("cargo:rustc-link-search={}/lib", jax_home);
    println!("cargo:rustc-link-lib=jax_xla");
    
    // Optimize for NUMA if available
    if let Ok(numa_nodes) = env::var("NUMA_NODES") {
        println!("cargo:rustc-cfg=numa_aware");
        println!("cargo:rustc-env=NUMA_NODES={}", numa_nodes);
    }
    
    // Build configuration
    let profile = env::var("PROFILE").unwrap();
    if profile == "release" {
        println!("cargo:rustc-cfg=release_build");
    }
    
    // Generate bindings for C++ bridge
    let bindings = bindgen::Builder::default()
        .header("include/jax_bridge.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");
    
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("jax_bindings.rs"))
        .expect("Couldn't write bindings");
    
    // CPU feature detection
    println!("cargo:rustc-cfg=has_avx2");
    println!("cargo:rustc-cfg=has_avx512");
    
    // Enable deadlock detection in debug builds
    if profile == "debug" {
        println!("cargo:rustc-cfg=deadlock_detection");
    }
}

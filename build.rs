fn main() {
    #[cfg(feature = "chafa-static")]
    {
        // For static linking, we handle linking ourselves
        let lib = pkg_config::Config::new()
            .statik(true)
            .probe("chafa")
            .expect(
                "Failed to find chafa via pkg-config. Install libchafa-dev or set PKG_CONFIG_PATH.",
            );

        // Find the static library and emit proper linking
        for path in &lib.link_paths {
            let static_lib = path.join("libchafa.a");
            if static_lib.exists() {
                // Use this path for static linking
                println!("cargo:rustc-link-search=native={}", path.display());
                println!("cargo:rustc-link-lib=static=chafa");

                // Also link dependencies that pkg-config found
                for link_path in &lib.link_paths {
                    if link_path != path {
                        println!("cargo:rustc-link-search=native={}", link_path.display());
                    }
                }

                // Link glib and its dependencies statically
                println!("cargo:rustc-link-lib=static=glib-2.0");
                println!("cargo:rustc-link-lib=static=sysprof-capture-4");
                println!("cargo:rustc-link-lib=pcre2-8");
                println!("cargo:rustc-link-lib=m");

                return;
            }
        }

        // If no static lib found, fall back to dynamic linking via pkg-config
        println!(
            "cargo:warning=libchafa.a not found in {:?}, using dynamic linking",
            lib.link_paths
        );
        // pkg_config already handled the linking when probe() succeeded
    }
}

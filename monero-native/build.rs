use bindgen::EnumVariation;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs as unix_fs;

#[cfg(target_os = "windows")]
use std::fs::copy;

fn main() {
    let header_path = "./monero_c/monero_libwallet2_api_c/src/main/cpp/wallet2_api_c.h";
    let lib_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("./libs");

    // Set library names based on target OS.
    //
    // This rigamarole is currently required because the "monero_libwallet2_api_c" library is also
    // required under the names "wallet2_api_c" and "libmonero_libwallet2_api_c" by various parts of
    // the stack.  This is a temporary workaround until the library is refactored to use a single
    // name consistently.
    let original_lib = if cfg!(target_os = "windows") {
        lib_path.join("libwallet2_api_c.dll")
    } else if cfg!(target_os = "macos") {
        lib_path.join("libwallet2_api_c.dylib")
    } else {
        lib_path.join("libwallet2_api_c.so")
    };

    let symlink_1 = if cfg!(target_os = "windows") {
        lib_path.join("wallet2_api_c.dll")
    } else if cfg!(target_os = "macos") {
        lib_path.join("libwallet2_api_c.dylib")
    } else {
        lib_path.join("libwallet2_api_c.so")
    };

    let symlink_2 = if cfg!(target_os = "windows") {
        lib_path.join("monero_wallet2_api_c.dll")
    } else if cfg!(target_os = "macos") {
        lib_path.join("libmonero_libwallet2_api_c.dylib")
    } else {
        lib_path.join("libmonero_libwallet2_api_c.so")
    };

    // On Unix-like systems, create symlinks.
    #[cfg(unix)]
    {
        if original_lib.exists() && !symlink_1.exists() {
            unix_fs::symlink(&original_lib, &symlink_1)
                .expect("Failed to create symbolic link for libwallet2_api_c.so");
        }

        if original_lib.exists() && !symlink_2.exists() {
            unix_fs::symlink(&original_lib, &symlink_2)
                .expect("Failed to create symbolic link for libmonero_libwallet2_api_c.so");
        }
    }

    // On Windows, copy the files instead of symlinking.
    #[cfg(target_os = "windows")]
    {
        if original_lib.exists() && !symlink_1.exists() {
            copy(&original_lib, &symlink_1).expect("Failed to copy DLL file to wallet2_api_c.dll");
        }

        if original_lib.exists() && !symlink_2.exists() {
            copy(&original_lib, &symlink_2)
                .expect("Failed to copy DLL file to monero_wallet2_api_c.dll");
        }
    }

    println!("cargo:rerun-if-changed={}", header_path);
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rustc-link-search=native={}", lib_path.display());
    println!("cargo:rustc-link-lib=dylib=monero_libwallet2_api_c");

    // Use c++ instead of stdc++ on macOS
    if cfg!(target_os = "macos") {
        println!("cargo:rustc-link-lib=c++");
    } else {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }

    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_path.display());

    // Generate bindings using bindgen.
    let bindings = bindgen::Builder::default()
        .header(header_path)
        .allowlist_function("MONERO_.*")
        .allowlist_var("MONERO_.*")
        .allowlist_var("NetworkType_.*")
        .allowlist_var("PendingTransactionStatus_.*")
        .allowlist_var("Priority_.*")
        .allowlist_var("UnsignedTransactionStatus_.*")
        .allowlist_var("TransactionInfoDirection_.*")
        .allowlist_var("AddressBookErrorCode.*")
        .allowlist_var("WalletDevice_.*")
        .allowlist_var("WalletStatus_.*")
        .allowlist_var("WalletConnectionStatus_.*")
        .allowlist_var("WalletBackgroundSync_.*")
        .allowlist_var("BackgroundSync_.*")
        .allowlist_var("LogLevel_.*")
        .blocklist_type("__.*")
        .blocklist_type("_.*")
        .blocklist_function("__.*")
        .layout_tests(false)
        .default_enum_style(EnumVariation::Rust {
            non_exhaustive: false,
        })
        .derive_default(false)
        .conservative_inline_namespaces()
        .generate_comments(false)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("src")
        .join("bindings.rs");
    bindings
        .write_to_file(out_path.clone())
        .expect("Couldn't write bindings!");

    // Annotate the generated bindings to ignore certain warnings.
    if out_path.exists() {
        let contents = fs::read_to_string(out_path.clone()).expect("Failed to read bindings.rs");

        let prepend_content = "#![allow(non_upper_case_globals)]\n#![allow(dead_code)]\n";

        if !contents.contains("#![allow(non_upper_case_globals)]") {
            let new_contents = format!("{}{}", prepend_content, contents);

            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(out_path.clone())
                .expect("Failed to open bindings.rs");

            file.write_all(new_contents.as_bytes())
                .expect("Failed to write to bindings.rs");
        }
    }
}

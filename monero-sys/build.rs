use cmake::Config;

fn main() {
    // Only rerun this when the bridge.rs or static_bridge.h file changes.
    println!("cargo:rerun-if-changed=src/bridge.rs");
    println!("cargo:rerun-if-changed=src/bridge.h");

    // Build with the monero library all dependencies required
    let mut config = Config::new("monero");
    let output_directory = config
        .build_target("wallet_api")
        .define("CMAKE_RELEASE_TYPE", "Release")
        .define("STATIC", "ON")
        .build_arg("-j")
        .build();

    let monero_build_dir = output_directory.join("build");

    println!(
        "cargo:debug=Build directory: {}",
        output_directory.display()
    );

    // Add output directories to the link search path
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("lib").display()
    );

    // Add additional link search paths for libraries in different directories
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("contrib/epee/src").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("external/easylogging++").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir
            .join("external/db_drivers/liblmdb")
            .display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("external/randomx").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/crypto").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/net").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/ringct").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/checkpoints").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/multisig").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/cryptonote_basic").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/common").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/cryptonote_core").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/hardforks").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/blockchain_db").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/device").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/device_trezor").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/mnemonics").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        monero_build_dir.join("src/rpc").display()
    );

    #[cfg(target_os = "macos")]
    {
        // add homebrew search paths/
        println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
    }

    // Link libwallet and libwallet_api statically
    println!("cargo:rustc-link-lib=static=wallet");
    println!("cargo:rustc-link-lib=static=wallet_api");

    // Link additional required libraries
    println!("cargo:rustc-link-lib=static=epee");
    println!("cargo:rustc-link-lib=static=easylogging");
    println!("cargo:rustc-link-lib=static=lmdb");
    println!("cargo:rustc-link-lib=static=randomx");
    println!("cargo:rustc-link-lib=static=cncrypto");
    println!("cargo:rustc-link-lib=static=net");
    println!("cargo:rustc-link-lib=static=ringct");
    println!("cargo:rustc-link-lib=static=ringct_basic");
    println!("cargo:rustc-link-lib=static=checkpoints");
    println!("cargo:rustc-link-lib=static=multisig");
    println!("cargo:rustc-link-lib=static=version");
    println!("cargo:rustc-link-lib=static=cryptonote_basic");
    println!("cargo:rustc-link-lib=static=cryptonote_format_utils_basic");
    println!("cargo:rustc-link-lib=static=common");
    println!("cargo:rustc-link-lib=static=cryptonote_core");
    println!("cargo:rustc-link-lib=static=hardforks");
    println!("cargo:rustc-link-lib=static=blockchain_db");
    println!("cargo:rustc-link-lib=static=device");
    println!("cargo:rustc-link-lib=static=device_trezor");
    println!("cargo:rustc-link-lib=static=mnemonics");
    println!("cargo:rustc-link-lib=static=rpc_base");

    // Link required system libraries dynamically
    println!("cargo:rustc-link-lib=dylib=hidapi");
    println!("cargo:rustc-link-lib=dylib=usb-1.0");
    println!("cargo:rustc-link-lib=dylib=unbound");
    println!("cargo:rustc-link-lib=dylib=boost_serialization");
    println!("cargo:rustc-link-lib=dylib=protobuf");
    println!("cargo:rustc-link-lib=dylib=sodium");
    println!("cargo:rustc-link-lib=dylib=boost_filesystem");
    println!("cargo:rustc-link-lib=dylib=boost_thread");
    println!("cargo:rustc-link-lib=dylib=boost_chrono");
    println!("cargo:rustc-link-lib=dylib=absl_base");
    println!("cargo:rustc-link-lib=dylib=absl_log_sink");
    println!("cargo:rustc-link-lib=dylib=absl_strings");
    println!("cargo:rustc-link-lib=dylib=absl_log_entry");
    println!("cargo:rustc-link-lib=dylib=absl_log_severity");
    println!("cargo:rustc-link-lib=dylib=absl_log_internal_message");
    println!("cargo:rustc-link-lib=dylib=absl_raw_logging_internal");
    println!("cargo:rustc-link-lib=dylib=absl_log_internal_check_op");
    println!("cargo:rustc-link-lib=dylib=absl_log_internal_nullguard");
    println!("cargo:rustc-link-lib=dylib=ssl");
    println!("cargo:rustc-link-lib=dylib=crypto");

    #[cfg(target_os = "macos")]
    {
        // Locate the Clang built-ins directory that contains libclang_rt.osx.*
        let clang = std::process::Command::new("xcrun")
            .args(["--find", "clang"])
            .output()
            .expect("failed to run xcrun --find clang");
        let clang_bin = std::path::PathBuf::from(String::from_utf8(clang.stdout).unwrap().trim());

        // <toolchain>/usr/bin/clang -> strip /bin/clang -> <toolchain>/usr
        let mut clang_dir = clang_bin;
        clang_dir.pop(); // bin
        clang_dir.pop(); // usr

        // lib/clang/<version>/lib/darwin
        let builtins_dir = clang_dir.join("lib").join("clang");
        // Highest version sub-directory
        let version_dir = std::fs::read_dir(&builtins_dir)
            .expect("read clang directory")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().ok().map(|t| t.is_dir()).unwrap_or(false))
            .max_by_key(|e| e.file_name()) // pick latest version
            .expect("no clang version dirs found")
            .path();
        let darwin_dir = version_dir.join("lib").join("darwin");

        println!("cargo:rustc-link-search=native={}", darwin_dir.display());

        // Static archive is always present, dylib only on some versions.
        println!("cargo:rustc-link-lib=static=clang_rt.osx");

        // Minimum OS version you already add:
        println!("cargo:rustc-link-arg=-mmacosx-version-min=11.0");
    }
    // Build the CXX bridge
    let mut build = cxx_build::bridge("src/bridge.rs");
    build
        .flag("-mmacosx-version-min=11.0")
        .flag_if_supported("-std=c++17")
        .include("src") // Include the bridge.h file
        .include("monero/src") // Includes the monero headers
        .include("monero/external/easylogging++") // Includes the easylogging++ headers
        .include("monero/contrib/epee/include") // Includes the epee headers for net/http_client.h
        .include("/opt/homebrew/include") // Homebrew include path for Boost
        .compile("monero-sys");
}

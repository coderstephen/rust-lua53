use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The command to build lua, with switches for different OSes.
fn build_lua_native(dir: &Path) -> io::Result<()> {
    let platform = if cfg!(target_os = "windows") {
        "mingw"
    } else if cfg!(target_os = "macos") {
        "macosx"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "freebsd") {
        "freebsd"
    } else if cfg!(target_os = "dragonfly") {
        "bsd"
    } else {
        panic!("Unsupported target OS")
    };

    if cfg!(any(target_os = "linux", target_os = "freebsd", target_os = "bsd")) {
        run_command(&["make", platform, "MYCFLAGS=-fPIC"], Some(dir))
    } else {
        run_command(&["make", platform], Some(dir))
    }
}

fn build_lua_target(dir: &Path) -> io::Result<()> {
    let cc = env::var("CC").unwrap_or("gcc".to_string());

    let target = if let Some(target) = env::var("TARGET").ok().and_then(|var| var.split('-').nth(2).map(|s| s.to_string())) {
        target
    } else {
        panic!("Unknown target OS")
    };

    let platform = match &target as &str {
        "windows" => { "mingw" }
        "darwin" => { "macosx" }
        "linux" => { "linux" }
        "freebsd" => { "freebsd" }
        "dragonfly" => { "bsd" }
        _ =>  {
            panic!("Unsupported target OS")
        }
    };

    if platform == "linux" || platform == "freebsd" || platform == "bsd" {
        run_command(&["make", platform, "MYCFLAGS=-fPIC"], Some(dir))
    } else {
        run_command(&["make", platform, &format!("CC={}", &cc)], Some(dir))
    }
}

/// The command to fetch a URL (e.g. with wget) specialized for different
/// OSes.
#[cfg(not(any(target_os = "freebsd", target_os = "dragonfly", target_os = "macos")))]
fn fetch_in_dir(url: &str, cwd: Option<&Path>) -> io::Result<()> {
    run_command(&["wget", url], cwd)
}

#[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
fn fetch_in_dir(url: &str, cwd: Option<&Path>) -> io::Result<()> {
    run_command(&["fetch", url], cwd)
}

#[cfg(target_os = "macos")]
fn fetch_in_dir(url: &str, cwd: Option<&Path>) -> io::Result<()> {
    run_command(&["curl", "-O", url], cwd)
}

/// Runs the command 'all_args[0]' with the arguments 'all_args[1..]' in the
/// directory 'cwd' or the current directory.
fn run_command(all_args: &[&str], cwd: Option<&Path>) -> io::Result<()> {
    let mut command = Command::new(all_args[0]);
    command.args(&all_args[1..]);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let status = try!(command.status());
    if !status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, format!("The command\n\
        \t{}\n\
        did not run successfully.", all_args.join(" "))));
    }
    Ok(())
}

/// If a static Lua is not yet available from a prior run of this script, this
/// will download Lua and build it. The cargo configuration text to link
/// statically against lua.a is then printed to stdout.
fn prebuild() -> io::Result<()> {
    let out_dir = env::var("OUT_DIR").unwrap();
    let build_dir = PathBuf::from(&out_dir);
    let lua_dir = PathBuf::from(&format!("{}/lua-5.3.0", &out_dir));

    // Ensure the presence of liblua.a
    if !fs::metadata(&format!("{}/lua-5.3.0/src/liblua.a", out_dir)).is_ok() {
        try!(fs::create_dir_all(build_dir.as_path()));

        // Download lua if it hasn't been already
        if !fs::metadata(&format!("{}/lua-5.3.0.tar.gz", &out_dir)).is_ok() {
            println!("{:?}", out_dir);
            try!(fetch_in_dir("http://www.lua.org/ftp/lua-5.3.0.tar.gz", Some(build_dir.as_path())));
            try!(run_command(&["tar", "xzf", "lua-5.3.0.tar.gz"], Some(build_dir.as_path())));
        }
        // Compile lua
        try!(run_command(&["make", "clean"], Some(lua_dir.as_path())));
        try!(build_lua_native(lua_dir.as_path()));
    }

    // Ensure the presence of glue.rs
    if !fs::metadata(&format!("{}/glue.rs", out_dir)).is_ok() {
        // Compile glue.c
        let glue = format!("{}/glue", out_dir);
        try!(run_command(&["gcc",
                         "-I", &format!("{}/lua-5.3.0/src", &out_dir),
                         "src/glue/glue.c",
                         "-o", &glue], None));
        // Run glue to generate glue.rs
        try!(run_command(&[&glue, &format!("{}/glue.rs", out_dir)], None));
    }

    // Build lua for the specified target
    try!(run_command(&["make", "clean"], Some(lua_dir.as_path())));
    // Compile lua
    try!(build_lua_target(lua_dir.as_path()));

    // Output build information
    println!("cargo:rustc-link-lib=static=lua");
    println!("cargo:rustc-link-search=native={}/lua-5.3.0/src", &out_dir);

    Ok(())
}

fn main() {
    match prebuild() {
        Err(e) => panic!("Error: {}", e),
        Ok(()) => (),
    }
}

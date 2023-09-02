use std::{path::PathBuf, process::Command};

use ignore::{types::TypesBuilder, WalkBuilder};

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let csharp_project = "des-lib";

    // 运行 dotnet publish 命令。
    let output = Command::new("dotnet")
        .args(["publish", "-v", "d", "-r", "win-x64", "-c", "Release"])
        .current_dir(PathBuf::from(manifest_dir).join(csharp_project))
        .output()
        .unwrap();
    let out: String = String::from_utf8_lossy(&output.stdout).into();
    if !output.status.success() {
        let err: String = String::from_utf8_lossy(&output.stderr).into();
        panic!("Error: \n{}\n{}\n", out, err);
    }

    // 在构建的日志中查找 ilcompiler 的安装位置。
    let pattern = "sdk\\System.Private.CoreLib.dll";
    let core_lib_path: PathBuf = out
        .find(pattern)
        .and_then(|pos| {
            let matched = &out[..pos + pattern.len() + 1];
            matched.rfind(':').map(|begin| &matched[begin - 1..])
        })
        .unwrap_or_else(|| {
            std::fs::write("dotnet-output.txt", &out).unwrap();
            panic!("ILCompiler sdk path not found in the dotnet command output: dotnet-output.txt")
        })
        .into();
    let sdk_path = core_lib_path.parent().unwrap().to_str().unwrap();

    // 找到 C# 项目中的 *.cs 和 *.csproj 文件，如果这些文件发生变化，则应重新构建。
    let mut types = TypesBuilder::new();
    types.add("cs", "*.cs").unwrap();
    types.add("csproj", "*.csproj").unwrap();
    types.select("cs").select("csproj");
    let walker = WalkBuilder::new(csharp_project)
        .types(types.build().unwrap())
        .build();
    for file in walker {
        let file = file.unwrap();
        if file.file_type().map(|t| t.is_file()).unwrap_or_default() {
            println!("cargo:rerun-if-changed={}", file.path().display());
        }
    }

    println!("cargo:rustc-link-arg=/INCLUDE:NativeAOT_StaticInitialization");
    println!("cargo:rustc-link-search={sdk_path}");
    println!(
        "cargo:rustc-link-search={manifest_dir}\\{csharp_project}\\bin\\Release\\net7.0\\win-x64\\publish"
    );
    println!("cargo:rustc-link-lib=static=bootstrapperdll");
    println!("cargo:rustc-link-lib=static=Runtime.WorkstationGC");
    println!("cargo:rustc-link-lib=static=System.Globalization.Native.Aot");
    println!("cargo:rustc-link-lib=static={csharp_project}");
}

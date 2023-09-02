# 静态链接 C# 到 Rust

最近 `.Net 7` 发布之后，因为带了 `AOT` 编译器，又爆发了一波热度，正好我最近有需求需要使用到这个功能，本文就记录下如何实现将 `.Net 7` 库编译成静态库，然后用 `Rust` 链接。

本文实现的是将一个非标准的 `DES` 算法编译成静态库，供 `Rust` 调用。该 `DES` 算法的 `C#` 实现在这里可以找到：<https://github.com/fygroup/Security/blob/master/DES.cs>。

本文项目的目录结构为：

```plaintext
./call-net-from-rust-statically
  ├── des-lib
  │   ├── des-lib.csproj
  │   └── DES.cs
  ├── Cargo.toml
  ├── build.rs
  └── src
      └── main.rs
```

先创建好 `call-net-from-rust-statically` 目录：

```powershell
mkdir call-net-from-rust-statically
```

## C# 项目部分

首先创建项目：

```powershell
cd call-net-from-rust-statically
dotnet new classlib -n des-lib
```

将 `Class1.cs` 重命名为 `DES.cs`，然后把上面链接中的 `DES` 类复制到 `DES.cs` 中，改下命名空间，再加上导出函数的代码，如下：

```csharp
namespace des_lib;

using System.Runtime.InteropServices;

public class DES
{
  [UnmanagedCallersOnly(EntryPoint = "wtf_des_encrypt")]
  public static nint FFI_Encrypt(nint message, nint key)
  {
    var managedMessage = Marshal.PtrToStringUTF8(message);
    var managedKey = Marshal.PtrToStringUTF8(key);
    if (managedKey == null || managedMessage == null)
    {
      return nint.Zero;
    }
    var cipherText = EncryptDES(managedMessage, managedKey);
    return Marshal.StringToHGlobalAnsi(cipherText);
  }

  [UnmanagedCallersOnly(EntryPoint = "wtf_des_decrypt")]
  public static nint FFI_Decrypt(nint cipherMessage, nint key)
  {
    var managedCipherMessage = Marshal.PtrToStringUTF8(cipherMessage);
    var managedKey = Marshal.PtrToStringUTF8(key);
    if (managedKey == null || managedCipherMessage == null)
    {
      return nint.Zero;
    }
    var plainText = DecryptDES(managedCipherMessage, managedKey);
    return Marshal.StringToHGlobalAnsi(plainText);
  }

  [UnmanagedCallersOnly(EntryPoint = "wtf_des_free")]
  public static void FFI_FreeMemory(nint buffer)
  {
    Marshal.FreeHGlobal(buffer);
  }

  // 将原有 DES 类的内容放在这里。
}
```

其中 `wtf_des_encrypt`、`wtf_des_decrypt` 和 `wtf_des_free` 就是导出的加密、解密以及释放内存的方法。

配置项目的属性：

```xml
<Project Sdk="Microsoft.NET.Sdk">

  <PropertyGroup>
    <TargetFramework>net7.0</TargetFramework>
    <NativeLib>Static</NativeLib>
    <PublishAot>true</PublishAot>
    <StripSymbols>true</StripSymbols>
    <SelfContained>true</SelfContained>
  </PropertyGroup>

</Project>
```

然后就可以用如下命令编译一下试试看：

```powershell
cd des-lib
dotnet publish -r win-x64 -c Release
```

在构建完毕之后，会在 `bin\Release\net7.0\win-x64\publish` 目录下生成 `des-lib.lib` 文件。

## Rust 项目部分

在上面的项目构建成功后，将会把 `ilcompiler` 包缓存，并可以在该目录 `%USERPROFILE%/.nuget/packages/runtime.win-x64.microsoft.dotnet.ilcompiler/7.0.1/sdk` 找到链接依赖的一些静态库（注意，版本号可能会变更）。

在 `call-net-from-rust-statically` 目录中创建 `Rust` 项目：

```powershell
cd call-net-from-rust-statically
cargo init
```

先添加 `windows` 依赖，这是因为在链接的时候，`.Net` 运行时会依赖 `Win32 API`：

```powershell
cargo add windows
```

添加 `build.rs`，一定要注意修改 `sdk_path` 中的 `ilcompiler` 版本号（本文讲的是实现步骤，最终的代码我会把 `des-lib` 的构建也放在 `build.rs` 中，并从构建的输出中寻找这个版本号，而不需要写死）：

```rust
use std::path::PathBuf;

fn main() {
    let user_profile: PathBuf = std::env::var("USERPROFILE").unwrap().into();
    let sdk_path: PathBuf = (user_profile)
        .join(".nuget\\packages\\runtime.win-x64.microsoft.dotnet.ilcompiler\\7.0.1\\sdk");
    let manifest_dir: PathBuf = std::env::var("CARGO_MANIFEST_DIR").unwrap().into();
    let des_lib_path = manifest_dir.join("des-lib");

    println!("cargo:rustc-link-arg=/INCLUDE:NativeAOT_StaticInitialization");
    println!("cargo:rustc-link-search={}", sdk_path.display());
    println!(
        "cargo:rustc-link-search={}\\bin\\Release\\net7.0\\win-x64\\publish",
        des_lib_path.display()
    );

    // 新版本的 windows crate 已不再提供 windows.lib，此处不再需要。
    // println!("cargo:rustc-link-lib=static=windows");
    println!("cargo:rustc-link-lib=static=bootstrapperdll");
    println!("cargo:rustc-link-lib=static=Runtime.WorkstationGC");
    println!("cargo:rustc-link-lib=static=System.Globalization.Native.Aot");
    println!("cargo:rustc-link-lib=static=des-lib");
}
```

接下来就是调用了，在 `main.rs` 中添加：

```rust
// 链接 windows crate。
extern crate windows;

extern "C" {
    fn wtf_des_encrypt(message: *const u8, key: *const u8) -> *const u8;
    fn wtf_des_decrypt(cipher_text: *const u8, key: *const u8) -> *const u8;
    fn wtf_des_free(ptr: *const u8);
}

fn main() {
    let key = b"key\0";
    let cipher_text = unsafe { wtf_des_encrypt(b"message\0".as_ptr(), key.as_ptr()) };
    let cipher_text = unsafe { std::ffi::CStr::from_ptr(cipher_text as *const i8) };
    let plain_text = unsafe { wtf_des_decrypt(cipher_text.as_ptr() as _, key.as_ptr()) };
    let plain_text = unsafe { std::ffi::CStr::from_ptr(plain_text as *const i8) };
    println!("cipher_text: {}", cipher_text.to_str().unwrap());
    println!("plain_text: {}", plain_text.to_str().unwrap());

    unsafe {
        wtf_des_free(cipher_text.as_ptr() as _);
        wtf_des_free(plain_text.as_ptr() as _);
    }
}
```

## Linux 兼容

Linux 系统上，需要链接的库有些许不一样，具体参见：<https://github.com/hamflx/call-net-from-rust-statically/commit/6cb7e9adb0a8faa48afc27e95267163131ca0717> 和 <https://github.com/hamflx/call-net-from-rust-statically/commit/95d155a309ff5d47b5800fbf8551e3343f3302b0>。

如果你加了额外的功能导致构建失败，可以自行查找所依赖的库，并链接。

例如，构建时报错，`undefined reference to ``RhRegisterOSModule'`，我们可以运行如下的代码找到所需的依赖：

```bash
# 这个路径按你的需要更改。
cd /home/hamflx/.nuget/packages/runtime.linux-x64.microsoft.dotnet.ilcompiler/7.0.10

find . -name '*.a' | xargs -I{} nm -o '{}' | grep RhRegisterOSModule
```

此时，输出结果大致如下：

```plaintext
./framework/libSystem.Native.a:pal_threading.c.o:0000000000000100 T SystemNative_LowLevelMonitor_Release
```

因此，我们可以链接 `System.Native` 库。

**注意，有时输出结果可能有多个，我们需要的是具有 `T` 标志的库。**

**注意，Linux 系统下，库的顺序会影响符号的解析，有时报错 `undefined reference` 可能调整下顺序即可。**

## 最终版本

仓库地址：<https://github.com/hamflx/call-net-from-rust-statically>，在本文的基础增加了自动构建 `C#` 项目，自动查找 `ilcompiler` 的路径并链接。

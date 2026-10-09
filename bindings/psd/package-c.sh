#!/bin/sh
# shellcheck disable=SC2016
# pkg-config expands its own path placeholders.
set -eu
cd "$(dirname "$0")"
mkdir -p target
set --
if [ "$(uname -s)" = Darwin ]; then
    set -- -C link-arg=-Wl,-install_name,@rpath/libphotocraft_psd_bindings.dylib
fi
cargo rustc --release --features c --locked --target-dir target/c --color never -- "$@" --print native-static-libs 2>target/c-build.log || { cat target/c-build.log >&2; exit 1; }
rm -rf target/native-sdk
mkdir -p target/native-sdk/include target/native-sdk/lib/cmake/PhotoCraftPsd target/native-sdk/lib/pkgconfig
cp include/photocraft_psd.h include/photocraft_psd.hpp include/module.modulemap target/native-sdk/include/
cp LICENSE-* README.md target/native-sdk/
flags=$(sed -n 's/^note: native-static-libs: //p' target/c-build.log)
{
    printf 'set(_psd_native_libs "%s")\n' "$flags"
    cat cmake/PhotoCraftPsdConfig.cmake
} > target/native-sdk/lib/cmake/PhotoCraftPsd/PhotoCraftPsdConfig.cmake
case "$(uname -s)" in
MINGW*|MSYS*|CYGWIN*)
    mkdir -p target/native-sdk/bin
    cp target/c/release/photocraft_psd_bindings.dll target/native-sdk/bin/
    cp target/c/release/photocraft_psd_bindings.dll.lib target/c/release/photocraft_psd_bindings.lib target/native-sdk/lib/
    ;;
Darwin) cp target/c/release/libphotocraft_psd_bindings.dylib target/c/release/libphotocraft_psd_bindings.a target/native-sdk/lib/ ;;
Linux) cp target/c/release/libphotocraft_psd_bindings.so target/c/release/libphotocraft_psd_bindings.a target/native-sdk/lib/ ;;
*) echo 'Unsupported native SDK platform.' >&2; exit 1 ;;
esac
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)
cat > target/native-sdk/lib/pkgconfig/photocraft-psd.pc <<'PC'
prefix=${pcfiledir}/../..
libdir=${prefix}/lib
includedir=${prefix}/include

Name: PhotoCraft PSD
Description: PSD/PSB reader and writer
PC
printf 'Version: %s\n' "$version" >> target/native-sdk/lib/pkgconfig/photocraft-psd.pc
case "$(uname -s)" in
MINGW*|MSYS*|CYGWIN*) printf 'Libs: -L${prefix}/bin -lphotocraft_psd_bindings\n' ;;
*) printf 'Libs: -L${libdir} -lphotocraft_psd_bindings\n' ;;
esac >> target/native-sdk/lib/pkgconfig/photocraft-psd.pc
case "$(uname -s)" in
MINGW*|MSYS*|CYGWIN*) ;;
*)
    printf 'Libs.private: %s\n' "$flags" >> target/native-sdk/lib/pkgconfig/photocraft-psd.pc
    ;;
esac
printf 'Cflags: -I${includedir}\n' >> target/native-sdk/lib/pkgconfig/photocraft-psd.pc

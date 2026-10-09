#!/bin/sh
set -eu
cd "$(dirname "$0")"
bin=target/sdk-consumer
exe=
case "$(uname -s)" in
MINGW*|MSYS*|CYGWIN*)
    case "${VSCMD_ARG_TGT_ARCH:-x64}" in arm64) arch=ARM64 ;; x64) arch=x64 ;; *) exit 1 ;; esac
    cmake -S tests/sdk -B target/sdk-consumer -A "$arch" -DCMAKE_PREFIX_PATH="$(cygpath -m "$PWD/target/native-sdk")"
    bin="$bin/Release"
    exe=.exe
    PATH="$PWD/target/native-sdk/bin:$PATH"; export PATH
    ;;
*) cmake -S tests/sdk -B target/sdk-consumer -DCMAKE_PREFIX_PATH="$PWD/target/native-sdk" -DCMAKE_BUILD_TYPE=Release ;;
esac
cmake --build target/sdk-consumer --config Release --parallel
for lang in c cpp; do
    for fixture in target/fixture.psd target/fixtures/*.psd; do
        "$bin/check-$lang-shared$exe" "$fixture"
    done
    "$bin/check-$lang-static$exe" target/fixture.psd
    "$bin/inspect-$lang$exe" target/normalized-input.psd "target/$lang.psd"
    cmp target/fixture.psd "target/$lang.psd"
    "$bin/create-$lang$exe" "target/created-$lang.psd"
done
cmp target/created-c.psd target/created-cpp.psd
if [ -z "$exe" ]; then
    PKG_CONFIG_PATH="$PWD/target/native-sdk/lib/pkgconfig"; export PKG_CONFIG_PATH
    # shellcheck disable=SC2046
    cc -std=c11 -Wall -Wextra -Werror examples/inspect.c $(pkg-config --cflags --libs photocraft-psd) \
        -Wl,-rpath,"$PWD/target/native-sdk/lib" -o target/inspect-pkgconfig
    target/inspect-pkgconfig target/normalized-input.psd target/pkgconfig.psd
    cmp target/fixture.psd target/pkgconfig.psd
fi
if [ "$(uname -s)" = Darwin ]; then
    # Linked consumers must load the SDK independently of its build directory.
    [ "$(otool -D target/native-sdk/lib/libphotocraft_psd_bindings.dylib | sed -n '2p')" = '@rpath/libphotocraft_psd_bindings.dylib' ]
    swiftc -O -module-cache-path target/swift-cache -I target/native-sdk/include -L target/native-sdk/lib \
        -Xlinker -rpath -Xlinker "$PWD/target/native-sdk/lib" tests/sdk/swift.swift -o target/check-swift
    target/check-swift target/fixture.psd
fi

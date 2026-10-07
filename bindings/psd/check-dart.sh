#!/bin/sh
set -eu
cd "$(dirname "$0")"
root="$PWD"
suffix=
case "$(uname -s)" in
    Darwin) library="$root/target/native-sdk/lib/libphotocraft_psd_bindings.dylib" ;;
    MINGW*|MSYS*|CYGWIN*)
        root=$(cygpath -m "$PWD")
        library="$root/target/native-sdk/bin/photocraft_psd_bindings.dll"
        suffix=.exe ;;
    *) library="$root/target/native-sdk/lib/libphotocraft_psd_bindings.so" ;;
esac
export PUB_CACHE="$root/target/dart-pub-cache"
export DASH__SUPPRESS_ANALYTICS=true
cd dart
dart pub get
dart format --output=none --set-exit-if-changed lib example test
dart analyze --fatal-infos
set -- "$root/target/fixture.psd" "$root"/target/fixtures/*.psd
dart run test/smoke.dart "$library" "$@"
dart compile exe test/smoke.dart -o "$root/target/check-dart$suffix"
"$root/target/check-dart$suffix" "$library" "$@"
dart run example/inspect.dart "$library" "$root/target/normalized-input.psd" "$root/target/dart.psd"
cmp "$root/target/fixture.psd" "$root/target/dart.psd"
dart run example/create.dart "$library" "$root/target/created-dart.psd"
cmp "$root/target/created-c.psd" "$root/target/created-dart.psd"
status=0
dart run example/create.dart "$library" "$root/target/fixtures" >"$root/target/dart-error.log" 2>&1 || status=$?
test "$status" -eq 1

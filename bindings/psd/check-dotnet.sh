#!/bin/sh
set -eu
cd "$(dirname "$0")"
rid="${1:?usage: check-dotnet.sh <RID>}"
case "$rid" in
    linux-x64|linux-arm64) library=libphotocraft_psd_bindings.so ;;
    osx-arm64) library=libphotocraft_psd_bindings.dylib ;;
    win-x64|win-arm64) library=photocraft_psd_bindings.dll ;;
    *) echo "unsupported RID: $rid" >&2; exit 1 ;;
esac
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml)
root="$PWD"
case "$rid" in win-*) root=$(cygpath -m "$PWD") ;; esac
native_root="$root/target/dotnet-native-check/$rid"
export DOTNET_CLI_HOME="$root/target/dotnet-home"
export NUGET_PACKAGES="$root/target/nuget-cache"
export DOTNET_GENERATE_ASPNET_CERTIFICATE=false
export DOTNET_CLI_TELEMETRY_OPTOUT=1
export DOTNET_NOLOGO=1
mkdir -p "$native_root/$rid/native"
cp "target/c/release/$library" "$native_root/$rid/native/"
dotnet pack csharp/PhotoCraft.Psd.csproj -c Release --artifacts-path "$root/target/dotnet" \
    -p:PsdVersion="$version" -p:NativeRoot="$native_root" -o target/nuget
"${PSD_BINDINGS_PYTHON:-python3}" tests/nuget.py "target/nuget/PhotoCraft.Psd.$version.nupkg" "$rid"
# Recheck the current package even when its version has not changed.
rm -rf "$NUGET_PACKAGES/photocraft.psd"
for project in tests/csharp/Smoke.csproj examples/csharp/Inspect.csproj examples/csharp/create/Create.csproj; do
    dotnet restore "$project" --force --source "$root/target/nuget" \
        -p:PsdVersion="$version" -p:ArtifactsPath="$root/target/dotnet"
done
dotnet run --project tests/csharp/Smoke.csproj -c Release --no-restore \
    -p:PsdVersion="$version" -p:ArtifactsPath="$root/target/dotnet" -- target/fixture.psd target/fixtures/*.psd
dotnet run --project examples/csharp/Inspect.csproj -c Release --no-restore \
    -p:PsdVersion="$version" -p:ArtifactsPath="$root/target/dotnet" -- target/normalized-input.psd target/csharp.psd
cmp target/fixture.psd target/csharp.psd
status=0
dotnet run --project examples/csharp/Inspect.csproj -c Release --no-build --no-restore \
    -p:PsdVersion="$version" -p:ArtifactsPath="$root/target/dotnet" -- target/fixture.psd target \
    >target/csharp-error.log 2>&1 || status=$?
test "$status" -eq 1
dotnet run --project examples/csharp/create/Create.csproj -c Release --no-restore \
    -p:PsdVersion="$version" -p:ArtifactsPath="$root/target/dotnet" -- target/created-csharp.psd
cmp target/created-c.psd target/created-csharp.psd

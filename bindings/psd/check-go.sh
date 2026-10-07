#!/bin/sh
set -eu
cd "$(dirname "$0")"
CGO_ENABLED=1
GOCACHE="$PWD/target/go-cache"
export CGO_ENABLED GOCACHE
case "$(uname -s)" in
MINGW*|MSYS*|CYGWIN*)
    CC=clang
    CGO_LDFLAGS="-L$(cygpath -m "$PWD/target/native-sdk/bin")"
    PATH="$PWD/target/native-sdk/bin:$PATH"
    export CC PATH
    ;;
*) CGO_LDFLAGS="-L$PWD/target/native-sdk/lib -Wl,-rpath,$PWD/target/native-sdk/lib" ;;
esac
export CGO_LDFLAGS
[ -z "$(gofmt -l go examples/go)" ] || { echo 'Run gofmt on the Go binding.' >&2; exit 1; }
go vet ./go ./examples/go/...
# Go's race detector supports Windows x64 but not Windows ARM64.
case "$(go env GOOS)/$(go env GOARCH)" in
windows/arm64) GOEXPERIMENT=cgocheck2 go test -count=1 ./go ;;
*) GOEXPERIMENT=cgocheck2 go test -race -count=1 ./go ;;
esac
go run ./examples/go/inspect target/normalized-input.psd target/go.psd target/go-preview.png
cmp target/fixture.psd target/go.psd
go run ./examples/go/create target/created-go.psd
cmp target/created-c.psd target/created-go.psd
if go run ./examples/go/create target/fixtures; then echo 'Expected output-path error.' >&2; exit 1; fi

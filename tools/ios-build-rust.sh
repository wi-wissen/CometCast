#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Builds the Rust static libraries of the iOS app. Called by an Xcode build phase of each
# target (apps/flutter/ios/project.yml) and by hand / CI with --xcframework.
#
#   ios-build-rust.sh cc-ffi             core of the Flutter app  -> libcc_ffi.a
#   ios-build-rust.sh cc-ios             broadcast extension      -> libcc_ios.a
#   ios-build-rust.sh cc-ios --xcframework   all three targets -> build/rust/CcIos.xcframework
#
# Inside Xcode the current platform decides what is built (PLATFORM_NAME, ARCHS):
#   iphoneos          aarch64-apple-ios
#   iphonesimulator   aarch64-apple-ios-sim and/or x86_64-apple-ios (joined with lipo)
# Output: apps/flutter/ios/build/rust/<platform>/lib<crate>.a plus native-static-libs.txt, the
# system libraries to link (compare with OTHER_LDFLAGS in project.yml).
#
# Rules:
#  * cc-ffi is built WITHOUT default features: the default would compile OpenH264 (C++), and a
#    downloaded decoder library is not allowed on iOS. Received video goes through VideoToolbox.
#  * cc-ios is always built in release mode: a debug build is far too big for the ~50 MB limit
#    of a broadcast extension. cc-ffi follows the Xcode configuration (Debug = cargo debug).
#
# UNCERTAIN: never run on a Mac (this file was written on Linux and only syntax-checked with
# `bash -n`). Needs rustup targets: aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios.
set -euo pipefail

crate="${1:?usage: ios-build-rust.sh <cc-ffi|cc-ios> [--xcframework]}"
mode="${2:-}"
case "$crate" in cc-ffi | cc-ios) ;; *) echo "unknown crate: $crate" >&2; exit 2 ;; esac

root="$(cd "$(dirname "$0")/.." && pwd)"
ios_dir="$root/apps/flutter/ios"
out_root="$ios_dir/build/rust"
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TARGET_DIR="${COMETCAST_CARGO_TARGET_DIR:-$ios_dir/build/cargo-target}"
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-15.0}"
# Xcode exports compiler and linker settings of the Xcode target; they would leak into the
# C compilers of the Rust build scripts (ring) and break them.
unset CC CXX LD LDFLAGS CFLAGS CXXFLAGS CPPFLAGS LIBRARY_PATH CPATH OTHER_LDFLAGS || true

profile="release"
if [ "$crate" = "cc-ffi" ] && [ "${CONFIGURATION:-Release}" = "Debug" ]; then profile="debug"; fi
profile_flag=(); [ "$profile" = "release" ] && profile_flag=(--release)

build_one() { # <rust target> <platform dir>
  local target="$1" dir="$2" libname="lib${crate//-/_}.a"
  rustup target add "$target" >/dev/null 2>&1 || true
  mkdir -p "$out_root/$dir"
  if [ "$crate" = "cc-ffi" ]; then
    cargo rustc -p cc-ffi --lib --crate-type staticlib --no-default-features \
      --target "$target" "${profile_flag[@]}" --manifest-path "$root/Cargo.toml" \
      -- --print native-static-libs 2> "$out_root/$dir/native-static-libs.$target.txt" \
      || { cat "$out_root/$dir/native-static-libs.$target.txt" >&2; return 1; }
  else
    cargo rustc -p cc-ios --lib --crate-type staticlib \
      --target "$target" "${profile_flag[@]}" --manifest-path "$root/Cargo.toml" \
      -- --print native-static-libs 2> "$out_root/$dir/native-static-libs.$target.txt" \
      || { cat "$out_root/$dir/native-static-libs.$target.txt" >&2; return 1; }
  fi
  echo "$CARGO_TARGET_DIR/$target/$profile/$libname"
}

join_sim() { # builds the simulator slices that ARCHS asks for, prints the joined library path
  local archs="${1:-arm64}" libs=() libname="lib${crate//-/_}.a"
  case " $archs " in *" arm64 "*) libs+=("$(build_one aarch64-apple-ios-sim iphonesimulator)") ;; esac
  case " $archs " in *" x86_64 "*) libs+=("$(build_one x86_64-apple-ios iphonesimulator)") ;; esac
  [ "${#libs[@]}" -gt 0 ] || { echo "no simulator architecture in ARCHS=$archs" >&2; exit 2; }
  local joined="$out_root/iphonesimulator/$libname"
  if [ "${#libs[@]}" -eq 1 ]; then cp "${libs[0]}" "$joined"; else lipo -create "${libs[@]}" -output "$joined"; fi
  echo "$joined"
}

libname="lib${crate//-/_}.a"

if [ "$mode" = "--xcframework" ]; then
  device="$(build_one aarch64-apple-ios iphoneos)"
  cp "$device" "$out_root/iphoneos/$libname"
  sim="$(join_sim "arm64 x86_64")"
  headers=()
  [ "$crate" = "cc-ios" ] && headers=(-headers "$root/crates/cc-ios/include")
  name="CcIos"; [ "$crate" = "cc-ffi" ] && name="CcFfi"
  rm -rf "$out_root/$name.xcframework"
  xcodebuild -create-xcframework \
    -library "$out_root/iphoneos/$libname" "${headers[@]}" \
    -library "$sim" "${headers[@]}" \
    -output "$out_root/$name.xcframework"
  echo "created $out_root/$name.xcframework"
  exit 0
fi

platform="${PLATFORM_NAME:-iphoneos}"
case "$platform" in
  iphoneos)
    lib="$(build_one aarch64-apple-ios iphoneos)"
    cp "$lib" "$out_root/iphoneos/$libname"
    ;;
  iphonesimulator)
    join_sim "${ARCHS:-arm64}" >/dev/null
    ;;
  *) echo "unsupported PLATFORM_NAME: $platform" >&2; exit 2 ;;
esac
echo "built $libname for $platform ($profile) in $out_root/$platform"

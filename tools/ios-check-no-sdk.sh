#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Type-checks the iOS Rust crates on a machine WITHOUT Xcode (for example Linux CI), so the
# Rust side of the iOS port can be checked without a Mac.
#
#   tools/ios-check-no-sdk.sh            # cc-ios and cc-ffi, three iOS targets
#
# How: `cargo check` runs the build scripts of C dependencies (ring, oslog, dart-sys) which need
# an Apple SDK. This script builds a fake SDK root in a temporary directory that holds a few
# stub headers (assert.h, string.h, TargetConditionals.h, os/log.h, ...) and points cc-rs at
# clang with it. The C code is only compiled to objects that `cargo check` never links.
# So this proves that the Rust code type-checks for the iOS targets; it proves NOTHING about
# linking, the C code of ring, or running. The real build needs a Mac (tools/ios-build-rust.sh).
#
# UNCERTAIN: the stubs cover exactly what ring 0.17.14, oslog 0.2.0 and dart-sys 4.1.5 include;
# another version of those crates may need more stubs. Needs `clang` and `llvm-ar` (or `ar`).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
for t in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  rustup target add "$t" >/dev/null
done

sdk="$(mktemp -d)"
trap 'rm -rf "$sdk"' EXIT
inc="$sdk/usr/include"
mkdir -p "$inc/os"
cat > "$inc/TargetConditionals.h" <<'H'
#define TARGET_OS_IPHONE 1
#define TARGET_OS_IOS 1
#define TARGET_OS_MAC 1
#define TARGET_OS_OSX 0
#define TARGET_OS_SIMULATOR 0
#define TARGET_CPU_ARM64 1
H
printf '#define assert(x) ((void)0)\n' > "$inc/assert.h"
cat > "$inc/string.h" <<'H'
#include <stddef.h>
void *memcpy(void*,const void*,size_t);void *memset(void*,int,size_t);
int memcmp(const void*,const void*,size_t);void *memmove(void*,const void*,size_t);
int strcmp(const char*,const char*);size_t strlen(const char*);
H
printf '#include <stddef.h>\nvoid *malloc(size_t);void free(void*);\n' > "$inc/stdlib.h"
printf '#include <stddef.h>\ntypedef struct FILE FILE;\nint printf(const char*,...);int fprintf(FILE*,const char*,...);int snprintf(char*,size_t,const char*,...);\n' > "$inc/stdio.h"
printf '#include <stdint.h>\n#define PRId64 "lld"\n#define PRIu64 "llu"\n#define PRIx64 "llx"\n#define PRIdPTR "ld"\n#define PRIuPTR "lu"\n#define PRIxPTR "lx"\n#define PRId32 "d"\n#define PRIu32 "u"\n' > "$inc/inttypes.h"
cat > "$inc/os/log.h" <<'H'
#include <stdint.h>
#include <stdbool.h>
typedef struct os_log_s *os_log_t;
typedef uint8_t os_log_type_t;
os_log_t os_log_create(const char *subsystem, const char *category);
bool os_log_type_enabled(os_log_t log, os_log_type_t type);
#define OS_LOG_DEFAULT ((os_log_t)0)
#define os_log_with_type(log, type, format, ...) ((void)0)
#define os_log_debug(log, format, ...) ((void)0)
#define os_log_info(log, format, ...) ((void)0)
#define os_log(log, format, ...) ((void)0)
#define os_log_error(log, format, ...) ((void)0)
#define os_log_fault(log, format, ...) ((void)0)
H

ar_tool="$(command -v llvm-ar || command -v ar)"
export SDKROOT="$sdk"
for t in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  u="${t//-/_}"
  export "CC_$u=clang" "AR_$u=$ar_tool" "CFLAGS_$u=-isystem $inc"
  echo "== cc-ios, $t"
  cargo check -p cc-ios --lib --target "$t"
  echo "== cc-ffi (no default features: no OpenH264), $t"
  cargo check -p cc-ffi --no-default-features --lib --target "$t"
done
echo "OK: the Rust code type-checks for all three iOS targets (no linking was done)."

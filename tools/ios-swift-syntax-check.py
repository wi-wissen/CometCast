#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Syntax-only check of the Swift files of the iOS project on a machine without a Swift
toolchain. Uses the tree-sitter Swift grammar:

    python3 -m venv /tmp/swiftcheck && /tmp/swiftcheck/bin/pip install tree-sitter tree-sitter-swift
    /tmp/swiftcheck/bin/python tools/ios-swift-syntax-check.py apps/flutter/ios

It finds parse errors (unbalanced braces, bad syntax). It does NOT type-check, resolve imports or
know the Apple SDK: a file that passes here can still fail to compile. `swiftc` on a Mac is the
real check.
"""
import pathlib
import sys

import tree_sitter
import tree_sitter_swift

parser = tree_sitter.Parser(tree_sitter.Language(tree_sitter_swift.language()))


def errors(node, src, out):
    if node.type == "ERROR" or node.is_missing:
        out.append((node.start_point[0] + 1, node.start_point[1] + 1, src[node.start_byte:node.start_byte + 40]))
    for child in node.children:
        errors(child, src, out)


bad = 0
root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else ".")
for path in sorted(root.rglob("*.swift")):
    if any(p in path.parts for p in ("Pods", "ephemeral", ".symlinks", "build")):
        continue
    src = path.read_bytes()
    found = []
    errors(parser.parse(src).root_node, src, found)
    print(path, "OK" if not found else found[:5])
    bad += bool(found)
sys.exit(1 if bad else 0)

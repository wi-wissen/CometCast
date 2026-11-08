#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Writes tests/fixtures/join-proof.json (presenter device proof of `join_room`).

The vectors are made with an implementation that is independent of the Rust and TypeScript code
(Python `cryptography`, Ed25519 as in RFC 8032; hashlib for SHA-256), from the rules of
docs/design/presenter-identity.md. The Rust test (crates/cc-transport/tests/join_proof_vectors.rs),
the web test (apps/web/tests/joinProof.test.ts) and the Swift test (apps/apple-tv/CometCastCore)
read the file; they never regenerate it.

    python3 tools/gen-join-proof-vectors.py > tests/fixtures/join-proof.json
"""
import base64
import hashlib
import json

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

DOMAIN = b"cometcast-join-v1"
WINDOW_MS = 120000
# Order of the Ed25519 base point group (RFC 8032).
L = 2**252 + 27742317777372353535851937790883648493


def b64(b):
    return base64.urlsafe_b64encode(b).rstrip(b"=").decode()


class Key:
    def __init__(self, name, seed):
        self.name = name
        self.seed = seed
        self.sk = Ed25519PrivateKey.from_private_bytes(seed)
        self.pk = self.sk.public_key().public_bytes(
            serialization.Encoding.Raw, serialization.PublicFormat.Raw
        )
        self.id = hashlib.sha256(self.pk).digest()[:16].hex()

    def info(self):
        return {
            "name": self.name,
            "seed_hex": self.seed.hex(),
            "public_key_hex": self.pk.hex(),
            "pk": b64(self.pk),
            "id": self.id,
        }


def signed_bytes(code, ts):
    return DOMAIN + b"\0" + code.upper().encode() + b"\0" + str(ts).encode()


A = Key("A", bytes(range(32)))
B = Key("B", bytes([0x42] * 32))
TS = 1767225600000

valid = []
refused = []


def proof(key, code, ts):
    msg = signed_bytes(code, ts)
    return msg, {"pk": b64(key.pk), "ts": ts, "sig": b64(key.sk.sign(msg))}


def add_valid(name, key, code, ts, now, verify_code=None):
    msg, dev = proof(key, code, ts)
    valid.append(
        {
            "name": name,
            "key": key.name,
            "code": verify_code or code,
            "now_ms": now,
            "signed_for_code": code,
            "signed_bytes_hex": msg.hex(),
            "device": dev,
            "device_id": key.id,
        }
    )
    return dev


def add_refused(name, reason, device, code, now):
    refused.append({"name": name, "reason": reason, "code": code, "now_ms": now, "device": device})


add_valid("key A, now equals ts", A, "K7QM2W", TS, TS)
add_valid("key B, other code", B, "P4X9TR", TS + 5, TS + 5000)
add_valid("lower-case code at the display is the same room", A, "K7QM2W", TS, TS, "k7qm2w")
add_valid("ts exactly 120 s behind the clock", A, "K7QM2W", TS, TS + WINDOW_MS)
add_valid("ts exactly 120 s ahead of the clock", A, "K7QM2W", TS, TS - WINDOW_MS)
add_valid("ts zero", B, "K7QM2W", 0, 60000)
# JavaScript keeps integers exactly only up to 2^53 - 1; larger values are not realistic times anyway.
add_valid("ts at the largest exactly representable integer", B, "K7QM2W", 2**53 - 1, 2**53 - 1)

_, base = proof(A, "K7QM2W", TS)


def mut(d, **kw):
    out = dict(d)
    out.update(kw)
    return out


add_refused("wrong room code", "signature", base, "K7QM2X", TS)
add_refused("ts changed after signing", "signature", mut(base, ts=TS + 1), "K7QM2W", TS)
add_refused("ts 121 s behind the clock", "window", base, "K7QM2W", TS + WINDOW_MS + 1)
add_refused("ts 121 s ahead of the clock", "window", base, "K7QM2W", TS - WINDOW_MS - 1)
bad = bytearray(base["sig"].encode())
bad[0] = ord("A") if bad[0] != ord("A") else ord("B")
add_refused("signature with a changed first character", "signature", mut(base, sig=bytes(bad).decode()), "K7QM2W", TS)
_, other = proof(B, "K7QM2W", TS)
add_refused("signature of another key", "signature", mut(base, sig=other["sig"]), "K7QM2W", TS)
add_refused("public key of another key", "signature", mut(base, pk=b64(B.pk)), "K7QM2W", TS)

# Signature over the same data with the name inside: the name is not signed, so a proof that
# signs more or less than the documented bytes does not verify.
sig_name = b64(A.sk.sign(signed_bytes("K7QM2W", TS) + b"\0Ana"))
add_refused("signed bytes with a name", "signature", mut(base, sig=sig_name), "K7QM2W", TS)
sig_nodomain = b64(A.sk.sign(b"K7QM2W\0" + str(TS).encode()))
add_refused("signed bytes without the domain", "signature", mut(base, sig=sig_nodomain), "K7QM2W", TS)
sig_lower = b64(A.sk.sign(DOMAIN + b"\0k7qm2w\0" + str(TS).encode()))
add_refused("signed with the code in lower case", "signature", mut(base, sig=sig_lower), "K7QM2W", TS)

# Strict verification: a small-order public key (the identity point) with the signature
# R = identity, S = 0 verifies for every message without strict checks.
identity = bytes([1]) + bytes(31)
weak_sig = identity + bytes(32)
add_refused("small-order public key (identity point)", "signature", {"pk": b64(identity), "ts": TS, "sig": b64(weak_sig)}, "K7QM2W", TS)
# A signature with S + L is the same signature without strict (reduced S) checks.
raw = base64.urlsafe_b64decode(base["sig"] + "==")
s_plus_l = (int.from_bytes(raw[32:], "little") + L).to_bytes(32, "little")
add_refused("signature with S not reduced (S + L)", "signature", mut(base, sig=b64(raw[:32] + s_plus_l)), "K7QM2W", TS)

# Shapes the server (and the display) refuse.
add_refused("pk 42 characters", "malformed", mut(base, pk=base["pk"][:42]), "K7QM2W", TS)
add_refused("pk 44 characters", "malformed", mut(base, pk=base["pk"] + "A"), "K7QM2W", TS)
add_refused("sig 85 characters", "malformed", mut(base, sig=base["sig"][:85]), "K7QM2W", TS)
add_refused("pk standard base64 alphabet", "malformed", mut(base, pk=base["pk"].replace("-", "+").replace("_", "/") + "") if ("-" in base["pk"] or "_" in base["pk"]) else mut(base, pk=base["pk"][:42] + "+"), "K7QM2W", TS)
add_refused("pk with padding", "malformed", mut(base, pk=base["pk"][:42] + "="), "K7QM2W", TS)
# Non-canonical spelling: the last character has bits that the 32 bytes do not use.
alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
last = alphabet.index(base["pk"][-1])
add_refused("pk non-canonical last character", "malformed", mut(base, pk=base["pk"][:-1] + alphabet[last | 1]), "K7QM2W", TS)
last = alphabet.index(base["sig"][-1])
add_refused("sig non-canonical last character", "malformed", mut(base, sig=base["sig"][:-1] + alphabet[last | 1]), "K7QM2W", TS)
add_refused("ts as a string", "malformed", mut(base, ts=str(TS)), "K7QM2W", TS)
add_refused("ts negative", "malformed", mut(base, ts=-1), "K7QM2W", TS)
add_refused("ts fractional", "malformed", mut(base, ts=TS + 0.5), "K7QM2W", TS)
add_refused("extra key", "malformed", mut(base, name="Ana"), "K7QM2W", TS)
add_refused("missing sig", "malformed", {"pk": base["pk"], "ts": TS}, "K7QM2W", TS)
add_refused("not an object", "malformed", "device", "K7QM2W", TS)
add_refused("null", "malformed", None, "K7QM2W", TS)

print(
    json.dumps(
        {
            "about": "Shared vectors for the presenter device proof of join_room. Made by tools/gen-join-proof-vectors.py with an independent implementation. Signed bytes: 'cometcast-join-v1', 0x00, room code in upper case, 0x00, ts in decimal. The name is not signed. The proof is accepted when |ts - now_ms| <= window_ms and the signature verifies strictly. device_id = lower-case hex of the first 16 bytes of SHA-256(public key).",
            "domain": DOMAIN.decode(),
            "window_ms": WINDOW_MS,
            "keys": [A.info(), B.info()],
            "valid": valid,
            "refused": refused,
        },
        indent=2,
        ensure_ascii=False,
    )
)

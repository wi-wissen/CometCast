#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Writes tests/fixtures/announce-v2.json (signed display announcements, layout v=2).

The vectors are made with an implementation that is independent of the Rust code (Python
`cryptography`, Ed25519 as in RFC 8032; hashlib for SHA-256), from the rules of
docs/design/signed-announcements.md. The Rust test (crates/cc-discovery/tests/announce_v2_vectors.rs)
and the Swift test (apps/apple-tv/CometCastCore) read the file; they never regenerate it.

    python3 tools/gen-announce-v2-vectors.py > tests/fixtures/announce-v2.json
"""
import base64
import hashlib
import json

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

SIGNED_KEYS = ["v", "id", "pk", "seq", "state", "srv", "tls", "mode", "code", "name"]
DOMAIN = b"cometcast-announce-v2"


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
            "tag": self.id[:4].upper(),
        }


def signed_bytes(values, order=SIGNED_KEYS):
    out = DOMAIN + b"\0"
    for k in order:
        out += k.encode() + b"=" + values.get(k, "").encode() + b"\0"
    return out


def record(key, seq, state, fields, order=SIGNED_KEYS):
    values = {"v": "2", "id": key.id, "pk": b64(key.pk), "seq": str(seq), "state": state}
    values.update(fields)
    msg = signed_bytes(values, order)
    sig = b64(key.sk.sign(msg))
    txt = [[k, values[k]] for k in SIGNED_KEYS if values.get(k, "") != ""]
    txt.append(["sig", sig])
    return msg, sig, txt


A = Key("A", bytes(range(32)))
B = Key("B", bytes([0x42] * 32))

OPEN = {"srv": "10.0.0.5:8443", "mode": "open", "code": "P4X9TR", "name": "Room 204"}
APPROVAL_TLS = {
    "srv": "relay.example.org:443",
    "tls": "1",
    "mode": "approval",
    "code": "K7Q2MW",
    "name": "Raum Süd 3",
}
LOCKED = {"srv": "192.168.1.20:8443", "mode": "locked"}

valid = []


def add(name, key, seq, state, fields):
    msg, sig, txt = record(key, seq, state, fields)
    valid.append(
        {
            "name": name,
            "key": key.name,
            "seq": seq,
            "state": state,
            "fields": fields,
            "signed_bytes_hex": msg.hex(),
            "sig": sig,
            "txt": txt,
        }
    )
    return txt


add("open_seq100", A, 100, "live", OPEN)
add("open_seq200", A, 200, "live", OPEN)
add("open_seq250", A, 250, "live", OPEN)
add("gone_seq300", A, 300, "gone", OPEN)
add("approval_tls_unicode", B, 1700000000000, "live", APPROVAL_TLS)
add("locked_no_code_no_name", A, 1700000000001, "live", LOCKED)
add("gone_locked", B, 1700000000002, "gone", LOCKED)

refused = []


def refuse(name, reason, txt):
    refused.append({"name": name, "reason": reason, "txt": txt})


def txt_of(name):
    return next(v["txt"] for v in valid if v["name"] == name)


def mutate(txt, key, value=None, remove=False):
    out = []
    for k, v in txt:
        if k == key:
            if remove:
                continue
            v = value
        out.append([k, v])
    return out


base = txt_of("open_seq100")
refuse("changed code", "signature", mutate(base, "code", "K7Q2MW"))
refuse("changed server", "signature", mutate(base, "srv", "10.66.0.1:1"))
refuse("changed seq", "signature", mutate(base, "seq", "101"))
refuse("changed state", "signature", mutate(base, "state", "gone"))
refuse("removed code", "signature", mutate(base, "code", remove=True))
refuse("removed name", "signature", mutate(base, "name", remove=True))
refuse("added tls", "signature", base[:5] + [["tls", "1"]] + base[5:])
refuse("id of another key", "id_mismatch", mutate(base, "id", B.id))
# Signed by key B but claims the id of key A (the forged-id case).
_, _, forged = record(B, 100, "live", OPEN)
refuse("victim id, attacker key", "id_mismatch", mutate(forged, "id", A.id))
# Signed with the keys in another order (srv and mode swapped).
order = list(SIGNED_KEYS)
order[5], order[7] = order[7], order[5]
_, _, swapped = record(A, 100, "live", OPEN, order)
refuse("wrong key order", "signature", swapped)
refuse("layout v1", "version", [["v", "1"], ["id", "0123456789abcdef"], ["srv", "10.0.0.5:8443"], ["mode", "open"], ["code", "P4X9TR"]])
refuse("no sig", "malformed", mutate(base, "sig", remove=True))
refuse("short sig", "malformed", mutate(base, "sig", base[-1][1][:80]))
refuse("short pk", "malformed", mutate(base, "pk", b64(A.pk)[:40]))
refuse("seq with leading zero", "malformed", mutate(base, "seq", "0100"))
refuse("uppercase id", "malformed", mutate(base, "id", A.id.upper()))
refuse("unknown state", "malformed", mutate(base, "state", "dead"))

# A sequence at the presenter: (record, accepted?). The highest accepted seq decides.
seq_cases = [
    {
        "name": "lower and equal seq are ignored, gone applies, replay after gone is ignored",
        "steps": [
            {"record": "open_seq200", "accepted": True},
            {"record": "open_seq100", "accepted": False},
            {"record": "open_seq200", "accepted": False},
            {"record": "gone_seq300", "accepted": True},
            {"record": "open_seq250", "accepted": False},
            {"record": "gone_seq300", "accepted": False},
        ],
    }
]

# seq = max(clock ms, last + 1), starting after `floor`.
seq_counter = [
    {"floor": 0, "clock": [1000, 1000, 5, 5000], "expect": [1000, 1001, 1002, 5000]},
    {"floor": 9000, "clock": [10, 10, 20000], "expect": [9001, 9002, 20000]},
]

print(
    json.dumps(
        {
            "about": "Shared vectors for signed display announcements (layout v=2). Made by tools/gen-announce-v2-vectors.py with an independent implementation. Signed bytes: 'cometcast-announce-v2', 0x00, then key=value, 0x00 for v,id,pk,seq,state,srv,tls,mode,code,name (absent optional keys with an empty value).",
            "keys": [A.info(), B.info()],
            "valid": valid,
            "refused": refused,
            "seq_cases": seq_cases,
            "seq_counter": seq_counter,
        },
        indent=2,
        ensure_ascii=False,
    )
)

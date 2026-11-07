/* SPDX-License-Identifier: GPL-3.0-or-later */
/*
 * C ABI of libcc_ios.a, the Comet Cast sender core for the iOS broadcast extension.
 * Written by hand from crates/cc-ios/src/ffi.rs; keep both in sync (a Rust test checks the
 * function names, the ABI version and the size of CcIosStatus).
 *
 * Threading: cc_ios_start returns at once. The status callback runs on a background thread;
 * the status struct and its message are valid only during the call. Push functions can be
 * called from any thread and never block. cc_ios_stop must not be called from the callback.
 *
 * UNCERTAIN: never linked into an extension, never run on a device (docs/ios.md).
 */
#ifndef CC_IOS_H
#define CC_IOS_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define CC_IOS_ABI_VERSION 1

/* Broadcast state (CcIosStatus.state). */
enum {
    CC_IOS_STATE_IDLE = 0,
    CC_IOS_STATE_CONNECTING = 1,   /* joining the room on the relay */
    CC_IOS_STATE_WAITING = 2,      /* in the room, negotiating with the receiver */
    CC_IOS_STATE_STREAMING = 3,    /* media can flow */
    CC_IOS_STATE_RECONNECTING = 4, /* temporarily lost, may recover */
    CC_IOS_STATE_ENDED = 5,        /* the receiver left */
    CC_IOS_STATE_FAILED = 6        /* see error */
};

/* Error numbers (CcIosStatus.error, and the return value of cc_ios_start). */
enum {
    CC_IOS_ERR_NONE = 0,
    CC_IOS_ERR_INVALID_ROOM_CODE = 1,
    CC_IOS_ERR_INVALID_RELAY = 2,
    CC_IOS_ERR_ROOM_NOT_FOUND = 3,
    CC_IOS_ERR_SERVER_UNREACHABLE = 4,
    CC_IOS_ERR_CONNECTION_LOST = 5,
    CC_IOS_ERR_PEER_LEFT = 6,
    CC_IOS_ERR_INTERNAL = 7,
    CC_IOS_ERR_ALREADY_RUNNING = 8
};

/* Return values of cc_ios_push_frame. */
enum {
    CC_IOS_PUSH_QUEUED = 0,
    CC_IOS_PUSH_NOT_STREAMING = 1,
    CC_IOS_PUSH_QUEUE_FULL = 2,
    CC_IOS_PUSH_WAITING_FOR_KEY_FRAME = 3,
    CC_IOS_PUSH_MISSING_PARAMETER_SETS = 4,
    CC_IOS_PUSH_INVALID = 5,
    CC_IOS_PUSH_NO_SENDER = 6
};

typedef struct CcIosStatus {
    int32_t state;
    int32_t error;
    uint64_t pushed;            /* access units handed to the sender */
    uint64_t sent;              /* written to the transport */
    uint64_t dropped;           /* dropped (not streaming, queue full, waiting, invalid) */
    uint64_t bytes_sent;
    uint64_t keyframes_sent;
    uint64_t keyframe_requests;
    const char *message;        /* technical English, for logs; valid during the call */
} CcIosStatus;

typedef void (*CcIosStatusCallback)(const CcIosStatus *status, void *user);

/*
 * Join the room `room_code` on `relay_url` ("host:port" or ws:// / wss:// URL) and send the
 * access units given to cc_ios_push_frame. `app_group_path` is the directory for cc-ios.log
 * (the App Group container); NULL = no log. All strings are NUL-terminated UTF-8 and copied.
 * Returns 0 when started, else an error number (1, 2, 7 or 8). `user` must stay valid until
 * cc_ios_stop returned.
 */
int32_t cc_ios_start(const char *room_code, const char *relay_url, const char *app_group_path,
                     CcIosStatusCallback callback, void *user);

/* Codec configuration: Annex B with SPS/PPS, or an avcC record. Returns 1 if accepted. */
int32_t cc_ios_set_config(const uint8_t *data, size_t len);

/*
 * One H.264 access unit, Annex B (start codes) preferred; AVCC with 4-byte lengths is accepted
 * too. Copied. `pts_us` in microseconds, negative = unknown. Returns a CC_IOS_PUSH_* value.
 */
int32_t cc_ios_push_frame(const uint8_t *data, size_t len, bool is_key, int64_t pts_us);

/* True once per key frame request; the encoder should then produce a sync frame. */
bool cc_ios_keyframe_requested(void);

/* Stop and release everything. Safe when nothing runs. May block for a few seconds. */
void cc_ios_stop(void);

int32_t cc_ios_abi_version(void);

#ifdef __cplusplus
}
#endif

#endif /* CC_IOS_H */

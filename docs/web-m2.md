<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Web client, milestone M2: toolbar, images and links

Status: implemented in `apps/web` (v0.1 line). Written for developers of other clients.

## What is in it

- **Receiver toolbar** over the received screen (floating, draggable, collapsible to a dot): pen
  (5 colours, 3 sizes), highlighter, eraser, undo, clear, laser pointer, freeze, black screen, full
  screen. Pointer events (mouse, touch, pen); the toolbar is a keyboard-operable `role="toolbar"`
  (Tab enters, Left/Right/Home/End move, Escape collapses, arrow keys on the grip move it).
- **Annotations are local.** They are drawn on a canvas on the receiver and stored only in that tab.
  Nothing is sent back to the sender or to the server. They are lost on reload.
- **Freeze** paints the current video frame to a canvas shown above the video. The `<video>` element
  keeps playing the live stream underneath, so unfreezing continues at the live position.
- **Send file or link** view on the sender: image (picker or drag and drop) and http(s) link, both over
  a WebRTC DataChannel. The signalling server only mediates the connection setup (SDP and ICE); it never
  sees the payloads.
- PDF and Office documents, and playing media links directly, are **not implemented**; the sender view
  says "coming later". They are planned via the Rust core.

## DataChannel `control`

The sender creates a data channel with the label `control` (`ordered: true`, reliable, no
`maxRetransmits`/`maxPacketLifeTime`) before creating the offer; the receiver accepts channels with
this label and ignores others. The sender may run in two modes: with a screen stream (media plus
channel) or content-only (channel only, no media section).

UNCERTAIN: this message set is defined by the web client for M2 only. It should move to `cc-proto`
(with the other protocol types) and be implemented by the native clients once it has been exercised.
Until then native clients must not assume it, and it may change.

### Text messages (JSON, one object per DataChannel text message, at most 8192 characters)

| `kind` | Fields | Meaning |
|--------|--------|---------|
| `cmd` | `cmd`: a `ControlCmd` from cc-proto in serde's default form: `"next_page"`, `"prev_page"`, `"stop"`, `{"go_to_page":{"page":3}}` | Remote-control command. The web receiver acts only on `stop` (closes the shown image or link); page commands are accepted and ignored until documents exist. |
| `image` | `name` (string, 1..255), `mime` (`image/png`, `image/jpeg`, `image/webp`, `image/gif`), `size` (integer bytes, 1..10 MiB) | Starts an image transfer. Binary chunks follow. |
| `image_end` | none | Ends the transfer; the receiver checks that the received byte count equals `size`. |
| `link` | `url` (string, at most 2048 characters) | Asks the receiver to open a link. See below. |

Example sequence for an image: `{"kind":"image","name":"a.png","mime":"image/png","size":40000}`,
then three binary messages (16384, 16384, 7232 bytes), then `{"kind":"image_end"}`.

### Binary messages

An `ArrayBuffer` of at most 16 KiB (16384 bytes) carrying the next bytes of the image announced by
the last `image` message, in order. The channel is ordered and reliable, so there are no sequence
numbers. A binary message without a preceding `image`, or one that would exceed `size`, is rejected
and the transfer is dropped. A new `image` message replaces a transfer in progress.

The sender stops writing while the channel's `bufferedAmount` is above 1 MiB and resumes at the
256 KiB low-water mark (`bufferedamountlow`).

UNCERTAIN: the 10 MiB limit, the 16 KiB chunk size and the buffer thresholds are untested guesses; no
measurements behind them. 16 KiB is the size commonly considered interoperable across browsers
(not verified here against other browsers).

### Receiver behaviour and safety

- Images: only the four MIME types above are accepted (no SVG, which can carry script). The blob gets
  the declared type from this allow list and is shown with `object-fit: contain`; a close button and
  the `stop` command remove it, and the object URL is revoked.
- Links: the receiver validates again (sender checks are not trusted). Only absolute `http:` and
  `https:` URLs without user name or password, without whitespace or control characters. A valid link
  is shown as a card "`<host>` wants to open a link" with **Open** and **Dismiss**. **Nothing is opened
  automatically**; Open is an anchor with `target="_blank"` and `rel="noopener noreferrer"`, so the
  browser opens a tab only after the receiver's person clicks it. The host is shown as the browser
  parses it (internationalised names appear in their punycode form).
- Malformed or unknown messages are ignored; the receiver shows a translated notice for transfer
  problems.

UNCERTAIN: whether a link opened by someone else's request is acceptable for every deployment (for
example kiosks) is not decided; a policy or allow list may be needed later.

## Tests

- Unit (vitest): `tests/control.test.ts` (chunking, reassembly, validation, back-pressure),
  `tests/toolbar.test.ts` (toolbar state, annotation model), `tests/contentSession.test.ts`.
- End to end: `npm run build && npm run e2e:m2` (real server and two Chromium pages): image arrives
  (`naturalWidth > 0`), link card appears and nothing opens until the click, pen changes canvas pixels,
  freeze keeps the frame while the video's `currentTime` keeps advancing.

What the e2e does not prove: other browsers, touch input on real devices, behaviour across networks
(no STUN/TURN yet), very large files, and real fullscreen (the button is not exercised).

## Chunk size rule

Every binary chunk of an image must be exactly 16 KiB, except the last one. The receiver rejects
smaller chunks (`bad_chunk`) to bound the number of messages it must hold in memory. Other senders
must follow this rule. UNCERTAIN: the value is unmeasured; see the other limits above.

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
- PDF and Office documents, and playing media links directly, are **not implemented in the web client**; the
  sender view says "coming later". Native clients can send and show PDFs (`document` kinds below).

## DataChannel `control`

The sender creates a data channel with the label `control` (`ordered: true`, reliable, no
`maxRetransmits`/`maxPacketLifeTime`) before creating the offer; the receiver accepts channels with
this label and ignores others. The sender may run in two modes: with a screen stream (media plus
channel) or content-only (channel only, no media section).

UNCERTAIN: this message set was defined by the web client for M2 and is now also implemented by the
native clients (`crates/cc-files`, see [transport.md](transport.md), "Content over the control data
channel"), which add the `document` kinds below. It should move to `cc-proto` (with the other
protocol types) once it has been exercised, and it may change.

### Text messages (JSON, one object per DataChannel text message, at most 8192 characters)

| `kind` | Fields | Meaning |
|--------|--------|---------|
| `cmd` | `cmd`: a `ControlCmd` from cc-proto in serde's default form: `"next_page"`, `"prev_page"`, `"stop"`, `{"go_to_page":{"page":3}}` | Remote-control command. The web receiver acts only on `stop` (closes the shown image or link); page commands are accepted and ignored until documents exist. |
| `image` | `name` (string, 1..255 characters), `mime` (`image/png`, `image/jpeg`, `image/webp`, `image/gif`), `size` (integer bytes, 1..10 MiB) | Starts an image transfer. Binary chunks follow. |
| `image_end` | none | Ends the transfer; the receiver checks that the received byte count equals `size`. |
| `link` | `url` (string, at most 2048 characters, **and the normalised URL at most 2048 characters too**) | Asks the receiver to open a link. See below. |
| `document` | `name` (string, 1..255 characters), `mime` (`application/pdf` only), `size` (integer bytes, 1..64 MiB) | Starts a PDF transfer, **sent by native clients; the web client does not send it**. Binary chunks follow, exactly as for an image. UNCERTAIN: new kind, defined by the native clients, not yet agreed anywhere else; the 64 MiB limit is a guess. |
| `document_end` | none | Ends the PDF transfer; the receiver checks that the received byte count equals `size`, then opens the document. |

Example sequence for a document: `{"kind":"document","name":"a.pdf","mime":"application/pdf","size":24832}`,
then two binary messages (16384 and 8448 bytes), then `{"kind":"document_end"}`. A receiver that can show it opens
the first page; `cmd` messages `next_page`, `prev_page` and `{"go_to_page":{"page":N}}` (N from 1) turn pages and
`stop` closes the document (native receivers; N is clamped to the page count, page commands are ignored while no
document is shown).

Example sequence for an image: `{"kind":"image","name":"a.png","mime":"image/png","size":40000}`,
then three binary messages (16384, 16384, 7232 bytes), then `{"kind":"image_end"}`.

### Names and link length

- A `name` is 1 to 255 **Unicode characters** (code points), counted the same way on every client; the web
  client counts code points, not UTF-16 units (an emoji is one character). The receiver removes control
  characters, bidirectional controls and other invisible characters before showing a name: the Unicode
  format characters (category Cf, e.g. U+200B, U+FEFF, U+00AD, tag characters U+E0000..U+E007F), U+2028 and
  U+2029, and a few blank-looking letters (U+2800, U+3164, U+115F, U+1160, U+17B4, U+17B5, U+FFA0). A name
  that is empty after cleaning is refused. UNCERTAIN: the list is hand-written on both sides (Rust
  `sanitize_name`, web `sanitizeName`), not generated from the Unicode data.
- A link counts against the 2048 characters **twice**: as written and as normalised (percent-encoding, punycode).
  1500 times "e-acute" is 1500 characters but 9000 after normalising, so it is refused by the sender and by the
  receiver. The whole `link` message must stay within 8192 characters. Validation is idempotent: a link that
  passes becomes a URL that passes again, unchanged.

### Binary messages

An `ArrayBuffer` of at most 16 KiB (16384 bytes) carrying the next bytes of the image announced by
the last `image` message, in order. The channel is ordered and reliable, so there are no sequence
numbers. A binary message without a preceding `image` (or `document`), or one that would exceed `size`, is
rejected and the transfer is dropped. A new `image` message replaces a transfer in progress (a native
receiver also reports that as a protocol violation). A native receiver that **refuses** a start message
(too large, type not allowed) counts that as one violation and silently swallows the chunks and the end
message that follow, up to the announced size; the web client does not do that.

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
- Documents: the web client **cannot show PDFs yet** (no web renderer). It skips the data of a `document`
  transfer without keeping it (same chunk rule, counting only) and, at `document_end`, shows the translated
  notice "The sender sent a PDF document. This app cannot show documents yet."
  (`receive.transferError.document_unsupported`). Nothing crashes and nothing is stored. Native receivers
  render PDFs through `cc-docs` in a child process (the file is treated as hostile, see
  [documents.md](documents.md)).
- Native receivers apply the same rules as the web client (MIME allow list, link validation, chunk rule, size
  limits) and a few more: a hard size cap while receiving, a message-count cap, a stalled-transfer timeout
  (60 s), merging of page commands, and blocking a peer after 64 protocol violations (which decay, one per 2 s).
  Details in [transport.md](transport.md).

UNCERTAIN: whether a link opened by someone else's request is acceptable for every deployment (for
example kiosks) is not decided; a policy or allow list may be needed later.

## Tests

- Unit (vitest): `tests/control.test.ts` (chunking, reassembly, validation, back-pressure),
  `tests/toolbar.test.ts` (toolbar state, annotation model), `tests/contentSession.test.ts`.
- Interop with the native clients: `npm run build && npm run e2e:content` (web sender to `cometcast-cli receive`,
  `cometcast-cli send-file` / `send-link` to the web receiver, the PDF notice, native PDF with page commands).
- End to end: `npm run build && npm run e2e:m2` (real server and two Chromium pages): image arrives
  (`naturalWidth > 0`), link card appears and nothing opens until the click, pen changes canvas pixels,
  freeze keeps the frame while the video's `currentTime` keeps advancing.

What the e2e does not prove: other browsers, touch input on real devices, behaviour across networks
(no STUN/TURN yet), very large files, and real fullscreen (the button is not exercised).

## Chunk size rule

Every binary chunk of an image must be exactly 16 KiB, except the last one. The receiver rejects
smaller chunks (`bad_chunk`) to bound the number of messages it must hold in memory. Other senders
must follow this rule. UNCERTAIN: the value is unmeasured; see the other limits above.

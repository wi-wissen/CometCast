<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Web client, milestone M2: toolbar, images, links and PDF documents

Status: implemented in `apps/web` (v0.1 line). Written for developers of other clients.

## What is in it

- **Receiver toolbar** over the received screen (floating, draggable, collapsible to a dot): pen
  (5 colours, 3 sizes), highlighter, eraser, undo, clear, laser pointer, freeze, black screen, full
  screen. Pointer events (mouse, touch, pen); the toolbar is a keyboard-operable `role="toolbar"`
  (Tab enters, Left/Right/Home/End move, Escape collapses, arrow keys on the grip move it).
- **Annotations are local.** They are drawn on a canvas on the receiver and stored only in that tab.
  Nothing is sent back to the sender or to the server. They are lost on reload. There is no annotation message
  in the `control` channel below; the Flutter receiver has the same toolbar and the same local behaviour
  ([flutter-bridge.md](flutter-bridge.md#toolbar-over-the-received-screen)).
- **Freeze** paints the current video frame to a canvas shown above the video. The `<video>` element
  keeps playing the live stream underneath, so unfreezing continues at the live position.
- **Send file or link** view on the sender: image and PDF (picker or drag and drop) and http(s) link, all over
  a WebRTC DataChannel; two buttons (previous and next page) send the page commands. The signalling server only mediates the connection setup (SDP and ICE); it never
  sees the payloads.
- **PDF documents** (receiver and sender) are implemented, see "Receiver behaviour and safety" and
  [documents.md](documents.md#in-the-web-client). The receiver renders pages with hayro compiled to WebAssembly,
  one page at a time on request, in a Web Worker. This needs the renderer files in `dist/pdf/`, which come from
  `npm run build:wasm` (a separate step that needs Rust, see `apps/web/README.md`); **without them the app still
  works, but a received PDF only gives the notice "cannot show documents yet"**.
- **Media links** are played by the web receiver (see "Media links" below). Office documents are **not
  implemented in the web client**; the sender view lists them as "coming later".

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
| `cmd` | `cmd`: a `ControlCmd` from cc-proto in serde's default form: `"next_page"`, `"prev_page"`, `"stop"`, `{"go_to_page":{"page":3}}` | Remote-control command. The web receiver acts on `stop` (closes the shown image, link or document) and, while a document is shown, on the page commands (N counts from 1 and is clamped to the page count). Page commands are ignored while no document is shown. The web sender sends `next_page`, `prev_page` and `stop`; it has no control for `go_to_page`. |
| `image` | `name` (string, 1..255 characters), `mime` (`image/png`, `image/jpeg`, `image/webp`, `image/gif`), `size` (integer bytes, 1..10 MiB) | Starts an image transfer. Binary chunks follow. |
| `image_end` | none | Ends the transfer; the receiver checks that the received byte count equals `size`. |
| `link` | `url` (string, at most 2048 characters, **and the normalised URL at most 2048 characters too**) | Asks the receiver to open a link. See below. |
| `document` | `name` (string, 1..255 characters), `mime` (`application/pdf` only), `size` (integer bytes, 1..64 MiB) | Starts a PDF transfer, sent by the native clients and by the web client. Binary chunks follow, exactly as for an image. UNCERTAIN: new kind, defined by the native clients, not yet agreed anywhere else; the 64 MiB limit is a guess. |
| `document_end` | none | Ends the PDF transfer; the receiver checks that the received byte count equals `size`, then opens the document. |

Example sequence for a document: `{"kind":"document","name":"a.pdf","mime":"application/pdf","size":24832}`,
then two binary messages (16384 and 8448 bytes), then `{"kind":"document_end"}`. A receiver that can show it opens
the first page; `cmd` messages `next_page`, `prev_page` and `{"go_to_page":{"page":N}}` (N from 1) turn pages and
`stop` closes the document (N is clamped to the page count, page commands are ignored while no document is
shown; same in the native and the web receiver).

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
- Documents: the receiver collects a `document` transfer exactly like an image (one buffer of the announced size,
  at most 64 MiB, every chunk exactly 16 KiB except the last, size must match at `document_end`) and then hands
  the bytes to the PDF renderer, which treats the file as hostile (see [documents.md](documents.md#in-the-web-client)):
  `cc-docs` as WebAssembly in a dedicated Web Worker, wall-clock limit of 20 s for open and for each page,
  worker terminated on a timeout or failure, memory of the module capped at 512 MiB, 1000 pages at most. Pages
  are rendered **only when shown** (first page after opening, then on page commands or the receiver's own
  previous/next buttons) and drawn as pixels on a canvas. **Nothing from the PDF is run or opened**: no script,
  no action, no link (links in the PDF are not exposed and the document view contains no anchor). A file
  that cannot be shown gives a translated notice (`receive.transferError.document_*`: damaged or unsupported,
  password protected, too large, too many pages, refused by the safety check, memory, timeout, failed); when the
  renderer files are not installed it is `document_unsupported`. Annotations are cleared on every page turn.
  Native receivers render PDFs through `cc-docs` in a child process (see [documents.md](documents.md)).
- UNCERTAIN: a document that fails while a page is rendered (timeout, crash, memory trap) is closed and not
  reopened by itself; the sender has to send it again. Opening a 64 MiB PDF allocates a 64 MiB buffer up front
  and then roughly the same again inside the module; not measured. A transfer that gets no chunk for 30 s (UNCERTAIN value)
  is dropped with a notice (`receive.transferError.stalled`; the native one has 60 s).
- Native receivers apply the same rules as the web client (MIME allow list, link validation, chunk rule, size
  limits) and a few more: a hard size cap while receiving, a message-count cap, a stalled-transfer timeout
  (60 s), merging of page commands, and blocking a peer after 64 protocol violations (which decay, one per 2 s).
  Details in [transport.md](transport.md).

UNCERTAIN: whether a link opened by someone else's request is acceptable for every deployment (for
example kiosks) is not decided; a policy or allow list may be needed later.

## Media links

A sender plays a video or audio file on the receiver by sending an ordinary `link`. **There is no new wire
message and no new field**: the receiver classifies the validated URL by the extension of its path (query and
fragment ignored, case-insensitive; `apps/web/src/core/media.ts`): `.mp4 .webm .ogv` video, `.mp3 .ogg .wav .m4a`
audio, `.m3u8` HLS. Other receivers (native clients, older web builds) see the same link card as before.

- The link card of a media link gets a **Play from `<host>`** button next to Open and Dismiss. **Nothing is
  loaded, no request is made and no media element exists until the receiver's person clicks Play** (a click is
  also the user activation that the browser's autoplay policy wants; the policy is not overridden). Dismiss
  still has the initial focus.
- The player is one `<video>` or `<audio>` element (native controls: play/pause, seek, volume, fullscreen for
  video) plus our **Stop** button and Escape. Attributes: `crossorigin="anonymous"` (no cookies or credentials),
  `referrerpolicy="no-referrer"`, `controlslist="nodownload noremoteplayback"`, `disablepictureinpicture`;
  the page also sets `<meta name="referrer" content="same-origin">`, so cross-origin requests carry no referrer
  (the e2e checks that the media host saw no cookie and no `Referer`). No download button is offered by us;
  a browser's own context menu is not removed.
- **One player at a time.** Stop, Dismiss, a `cmd` `stop` from the sender, new content (image, document, another
  link) or the sender leaving closes the player: the element is paused, its `src` removed, `load()` called and it
  is taken out of the page, which closes the connection.
- Errors map `MediaError.code` to translated notices (aborted, network, decode, unsupported) that name only the
  host, never the path or query. Because of `crossorigin="anonymous"` **a media server that sends no CORS
  header (`Access-Control-Allow-Origin`) cannot be played** and gives the "network" notice; the link can still be
  opened with Open. This is a deliberate price of not sending cookies.
- **HLS (`.m3u8`)**: only where the browser plays it natively (Safari; recent Chrome on some platforms). The
  Play button is offered only if `canPlayType` says so; otherwise the card says that the stream cannot be played
  here and Open stays. **hls.js is not bundled** (Apache-2.0 would be compatible with GPL-3.0-or-later, but it was
  not added in this step: a new runtime dependency, MSE code path, fetching segments from third-party hosts).
  UNCERTAIN: HLS playback itself was not exercised in the e2e (no segments are served; only the Play/no-Play
  decision is checked, which on the test machine's Chrome offered Play).
- **Sender-side remote control is not done.** The `cmd` set (`next_page`, `prev_page`, `stop`, `go_to_page`)
  has no play, pause or seek, and inventing one would be a protocol change; only `stop` works. UNCERTAIN as a
  product question: whether a presenter needs it.
- **No third-party players.** YouTube, Vimeo and similar page URLs are not recognised and no iframe is embedded:
  their terms forbid it or make it unreliable, and they load trackers on the receiver. Such a link is an ordinary
  link card. (Desktop and mini PC extraction with yt-dlp stays a native-core item of the architecture.)
- Not done: Flutter. The Flutter link card is unchanged (Open and Dismiss only); a player there needs a media
  package such as `media_kit` or `video_player` with native libraries, a heavy dependency (UNCERTAIN, not
  evaluated further).

## Tests

- Unit (vitest): `tests/control.test.ts` (chunking, reassembly, validation, back-pressure),
  `tests/toolbar.test.ts` (toolbar state, annotation model), `tests/contentSession.test.ts`.
- Unit (vitest) for documents: `tests/control.test.ts` (document reassembly, chunk rule, `sendDocument`),
  `tests/pdfRenderer.test.ts` (worker protocol, time limits, termination, with a fake worker),
  `tests/documentSession.test.ts` (page commands, clamping, error notices, renderer disposal). The renderer
  crate has native unit tests (`cargo test -p cc-docs-wasm`).
- Interop with the native clients: `npm run build && npm run e2e:content` (web sender to `cometcast-cli receive`
  including a PDF and a page turn, `cometcast-cli send-file` / `send-link` to the web receiver, a native PDF
  rendered in the web receiver when the WASM renderer was built, otherwise the notice, native PDF with page commands).
- PDF in the browser: `npm run build:wasm && npm run build && npm run e2e:pdf` (web sender to web receiver in
  English and German: a generated 3-page PDF renders to a non-blank canvas with a black box on page 1 and a red
  box on page 2, next/prev from the sender and on the receiver with clamping, a link annotation is not followed,
  a damaged PDF gives the notice, a non-PDF is refused by the sender, stop closes it). Set `E2E_CHROME=/path/to/chrome`
  to use an installed Chrome instead of Playwright's Chromium (also read by `e2e:m2` and `e2e:content`).
- Media links: `tests/media.test.ts` (classification, HLS probe, error mapping, player state) and
  `npm run build && npm run e2e:media`: sender sends a link to a generated `.wav` and `.webm` served by a local
  server; nothing is requested before the click, playing after it with no cookie and no referrer, Stop and the
  sender's `stop` release the element, a new link replaces a playing one, a broken file and a server without CORS
  give a notice with the host only, HLS and plain links. Needs `ffmpeg` for the `.webm`; Chrome (Google Chrome
  on Linux) only.
- End to end: `npm run build && npm run e2e:m2` (real server and two Chromium pages): image arrives
  (`naturalWidth > 0`), link card appears and nothing opens until the click, pen changes canvas pixels,
  freeze keeps the frame while the video's `currentTime` keeps advancing.

What the e2e does not prove: other browsers (PDFs: only Chrome/Chromium on Linux was run), touch input on real
devices, behaviour across networks (no STUN/TURN yet), very large files, hostile PDFs in the browser, PDF
speed and memory on weak devices, and real fullscreen (the button is not exercised).

## Chunk size rule

Every binary chunk of an image or a PDF must be exactly 16 KiB, except the last one. The receiver rejects
smaller chunks (`bad_chunk`) to bound the number of messages it must hold in memory. Other senders
must follow this rule. UNCERTAIN: the value is unmeasured; see the other limits above.

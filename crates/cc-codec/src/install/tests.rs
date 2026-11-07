// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests of the download path against a local HTTP server. The plain-HTTP endpoint exists
//! only here (`#[cfg(test)]`): `Endpoint::cisco()` is the only constructor in release code and
//! it fixes the host to Cisco's and `https_only` to `true`.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::store::sha256_hex;

const FAKE_LIB_BZ2: &[u8] = &[
    66, 90, 104, 57, 49, 65, 89, 38, 83, 89, 145, 140, 45, 74, 0, 3, 249, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 240, 80, 3, 62, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 26, 104, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 161, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 105, 160, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 132,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 166, 128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 26, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 154, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 104, 10, 165, 84, 76, 70, 155, 66, 104, 200, 192, 35, 4, 105, 132, 211, 0,
    153, 52, 194, 96, 19, 210, 105, 180, 154, 12, 67, 76, 0, 0, 152, 76, 0, 76, 0, 9, 136, 96, 17,
    129, 12, 152, 140, 32, 192, 8, 98, 12, 9, 163, 0, 8, 192, 52, 208, 73, 56, 136, 173, 246, 194,
    80, 59, 129, 221, 14, 240, 119, 195, 194, 9, 120, 120, 97, 48, 9, 0, 152, 132, 200, 38, 97,
    226, 4, 208, 60, 80, 241, 130, 106, 19, 96, 241, 195, 200, 9, 184, 78, 2, 114, 19, 160, 157,
    132, 240, 39, 161, 62, 15, 36, 60, 160, 159, 133, 0, 40, 33, 66, 10, 24, 121, 97, 230, 5, 16,
    60, 208, 162, 133, 24, 40, 225, 72, 15, 56, 41, 33, 231, 133, 40, 41, 97, 76, 10, 104, 83, 130,
    158, 21, 0, 168, 135, 160, 21, 32, 169, 135, 162, 21, 64, 244, 130, 170, 21, 96, 171, 133, 96,
    61, 48, 172, 134, 42, 182, 18, 85, 208, 175, 5, 124, 44, 1, 234, 4, 136, 88, 96, 72, 135, 170,
    22, 32, 245, 130, 198, 18, 65, 49, 11, 32, 122, 225, 101, 15, 96, 61, 144, 246, 131, 219, 11,
    48, 89, 194, 208, 30, 224, 123, 161, 239, 5, 164, 61, 240, 181, 5, 172, 45, 129, 109, 11, 112,
    91, 194, 224, 23, 16, 185, 5, 204, 46, 129, 117, 11, 176, 93, 194, 240, 23, 144, 189, 5, 236,
    62, 0, 190, 5, 244, 47, 193, 127, 15, 132, 48, 1, 129, 12, 16, 96, 195, 29, 133, 12, 48, 97,
    195, 16, 24, 144, 197, 7, 196, 21, 112, 197, 134, 48, 49, 161, 142, 9, 8, 136, 199, 134, 64,
    50, 33, 146, 12, 152, 101, 3, 43, 17, 25, 96, 203, 134, 96, 62, 48, 204, 134, 104, 51, 113, 17,
    242, 7, 202, 31, 48, 103, 3, 58, 25, 224, 207, 196, 70, 128, 52, 33, 162, 13, 24, 124, 225,
    244, 6, 146, 34, 52, 161, 166, 13, 56, 106, 3, 82, 31, 72, 125, 81, 17, 245, 134, 168, 53, 97,
    246, 6, 176, 53, 161, 174, 136, 143, 180, 62, 224, 215, 134, 192, 54, 33, 178, 15, 190, 34, 63,
    0, 252, 67, 242, 13, 152, 109, 3, 106, 27, 104, 136, 219, 134, 224, 55, 33, 186, 13, 216, 111,
    3, 243, 136, 143, 208, 63, 80, 222, 134, 248, 55, 225, 251, 7, 239, 17, 31, 192, 127, 33, 192,
    14, 8, 112, 131, 134, 28, 72, 136, 226, 135, 244, 28, 96, 254, 195, 252, 14, 56, 114, 34, 35,
    146, 28, 160, 229, 135, 48, 57, 161, 206, 14, 124, 68, 116, 3, 162, 29, 32, 255, 67, 254, 14,
    152, 117, 34, 35, 170, 29, 96, 235, 135, 96, 59, 33, 218, 9, 56, 136, 237, 132, 160, 119, 3,
    186, 29, 224, 239, 135, 254, 46, 228, 138, 112, 161, 33, 35, 24, 90, 148,
];
const BOMB_BZ2: &[u8] = &[
    66, 90, 104, 57, 49, 65, 89, 38, 83, 89, 75, 228, 120, 2, 1, 33, 35, 196, 192, 192, 0, 3, 4, 0,
    0, 128, 4, 0, 8, 32, 0, 80, 134, 0, 41, 80, 104, 240, 68, 17, 205, 209, 74, 20, 179, 51, 221,
    132, 65, 31, 23, 114, 69, 56, 80, 144, 75, 228, 120, 2,
];

fn fake_lib() -> Vec<u8> {
    let mut v = b"\x7fELF".to_vec();
    v.extend((0..4000u32).map(|i| (i * 7 % 251) as u8));
    v
}

fn artifact_for_bytes(bytes: &[u8]) -> Artifact {
    Artifact {
        name: "libfake.so",
        sha256: Box::leak(sha256_hex(bytes).into_boxed_str()),
        compressed_bytes: 695,
    }
}

/// A raw HTTP response.
struct Raw(Vec<u8>);

fn ok(body: &[u8]) -> Raw {
    let mut r = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    r.extend_from_slice(body);
    Raw(r)
}

fn status(line: &str, extra: &str) -> Raw {
    Raw(
        format!("HTTP/1.1 {line}\r\n{extra}Content-Length: 0\r\nConnection: close\r\n\r\n")
            .into_bytes(),
    )
}

/// Serve one response per connection, in order, then stop. Returns the base URL and the
/// number of requests seen.
fn serve(responses: Vec<Raw>) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&count);
    std::thread::spawn(move || {
        for r in responses {
            let Ok((mut s, _)) = listener.accept() else {
                return;
            };
            let mut buf = [0u8; 4096];
            let mut got = Vec::new();
            while !got.windows(4).any(|w| w == b"\r\n\r\n") {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => got.extend_from_slice(&buf[..n]),
                }
            }
            seen.fetch_add(1, Ordering::SeqCst);
            let _ = s.write_all(&r.0);
        }
    });
    (url, count)
}

fn test_endpoint(base_url: String) -> Endpoint {
    Endpoint {
        base_url,
        https_only: false,
        no_proxy: true,
        retries: 2,
        backoff: Duration::from_millis(1),
        total_timeout: Duration::from_secs(10),
        stall_timeout: Duration::from_secs(5),
    }
}

fn bz2_of_fake() -> &'static [u8] {
    FAKE_LIB_BZ2
}

#[test]
fn downloads_decompresses_verifies_and_installs() {
    let lib = fake_lib();
    let art = artifact_for_bytes(&lib);
    let (url, count) = serve(vec![ok(bz2_of_fake())]);
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("codecs");
    let mut last = (0, 0);
    let p =
        install_from_network(&test_endpoint(url), &art, &dir, &mut |d, t| last = (d, t)).unwrap();
    assert_eq!(fs::read(&p).unwrap(), lib);
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(last.0, bz2_of_fake().len() as u64);
    assert_eq!(last.1, last.0);
}

#[test]
fn accepts_an_uncompressed_library_too() {
    let lib = fake_lib();
    let art = artifact_for_bytes(&lib);
    let (url, _) = serve(vec![ok(&lib)]);
    let t = tempfile::tempdir().unwrap();
    let p = install_from_network(&test_endpoint(url), &art, t.path(), &mut |_, _| {}).unwrap();
    assert_eq!(fs::read(p).unwrap(), lib);
}

#[test]
fn tampered_download_is_rejected_and_nothing_is_written() {
    let lib = fake_lib();
    let art = artifact_for_bytes(&lib);
    let mut evil = lib.clone();
    evil[100] ^= 1;
    let (url, _) = serve(vec![ok(&evil)]);
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("codecs");
    let err = install_from_network(&test_endpoint(url), &art, &dir, &mut |_, _| {}).unwrap_err();
    assert!(matches!(err, CodecError::HashMismatch { .. }), "{err}");
    assert!(!dir.join(art.name).exists());
}

#[test]
fn html_error_page_is_a_format_error() {
    let art = artifact_for_bytes(&fake_lib());
    let (url, _) = serve(vec![ok(b"<html>captive portal</html>")]);
    let t = tempfile::tempdir().unwrap();
    let err =
        install_from_network(&test_endpoint(url), &art, t.path(), &mut |_, _| {}).unwrap_err();
    assert!(matches!(err, CodecError::BadFormat(_)), "{err}");
}

#[test]
fn announced_size_over_the_cap_is_refused() {
    let art = artifact_for_bytes(&fake_lib());
    let big = MAX_COMPRESSED_BYTES + 1;
    let (url, _) = serve(vec![Raw(format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {big}\r\nConnection: close\r\n\r\n"
    )
    .into_bytes())]);
    let t = tempfile::tempdir().unwrap();
    let err =
        install_from_network(&test_endpoint(url), &art, t.path(), &mut |_, _| {}).unwrap_err();
    assert!(matches!(err, CodecError::TooLarge), "{err}");
}

#[test]
fn body_over_the_cap_without_content_length_is_refused() {
    let art = artifact_for_bytes(&fake_lib());
    let mut body = b"BZh9".to_vec();
    body.resize(MAX_COMPRESSED_BYTES as usize + 10, 0);
    // No Content-Length: the body ends with the connection.
    let mut raw = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
    raw.extend_from_slice(&body);
    let (url, _) = serve(vec![Raw(raw)]);
    let t = tempfile::tempdir().unwrap();
    let err =
        install_from_network(&test_endpoint(url), &art, t.path(), &mut |_, _| {}).unwrap_err();
    assert!(matches!(err, CodecError::TooLarge), "{err}");
}

#[test]
fn decompression_bomb_is_capped() {
    let err = decode(BOMB_BZ2).unwrap_err();
    assert!(matches!(err, CodecError::TooLarge), "{err}");
}

#[test]
fn decode_sniffs_formats() {
    assert_eq!(decode(bz2_of_fake()).unwrap(), fake_lib());
    assert_eq!(decode(&fake_lib()).unwrap(), fake_lib());
    assert!(matches!(decode(b"hello"), Err(CodecError::BadFormat(_))));
    assert!(matches!(decode(b""), Err(CodecError::BadFormat(_))));
    let truncated = &bz2_of_fake()[..bz2_of_fake().len() / 2];
    assert!(matches!(decode(truncated), Err(CodecError::BadFormat(_))));
}

#[test]
fn redirects_are_not_followed() {
    let art = artifact_for_bytes(&fake_lib());
    let (url, count) = serve(vec![
        status("302 Found", "Location: http://127.0.0.1:1/evil\r\n"),
        ok(bz2_of_fake()),
    ]);
    let t = tempfile::tempdir().unwrap();
    let mut ep = test_endpoint(url);
    ep.retries = 0;
    let err = install_from_network(&ep, &art, t.path(), &mut |_, _| {}).unwrap_err();
    assert!(matches!(err, CodecError::Http(302)), "{err}");
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn server_errors_are_retried_not_found_is_not() {
    let lib = fake_lib();
    let art = artifact_for_bytes(&lib);
    let (url, count) = serve(vec![
        status("503 Service Unavailable", ""),
        ok(bz2_of_fake()),
    ]);
    let t = tempfile::tempdir().unwrap();
    install_from_network(&test_endpoint(url), &art, t.path(), &mut |_, _| {}).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2);

    let (url, count) = serve(vec![status("404 Not Found", ""), ok(bz2_of_fake())]);
    let err =
        install_from_network(&test_endpoint(url), &art, t.path(), &mut |_, _| {}).unwrap_err();
    assert!(matches!(err, CodecError::Http(404)));
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

#[test]
fn gives_up_after_the_retries() {
    let art = artifact_for_bytes(&fake_lib());
    let (url, count) = serve(vec![
        status("500 Internal Server Error", ""),
        status("500 Internal Server Error", ""),
        status("500 Internal Server Error", ""),
    ]);
    let t = tempfile::tempdir().unwrap();
    let err =
        install_from_network(&test_endpoint(url), &art, t.path(), &mut |_, _| {}).unwrap_err();
    assert!(matches!(err, CodecError::Http(500)));
    assert_eq!(count.load(Ordering::SeqCst), 3);
}

#[test]
fn plain_http_is_refused_when_https_only() {
    let art = artifact_for_bytes(&fake_lib());
    let (url, count) = serve(vec![ok(bz2_of_fake())]);
    let t = tempfile::tempdir().unwrap();
    let mut ep = test_endpoint(url);
    ep.https_only = true;
    ep.retries = 0;
    let err = install_from_network(&ep, &art, t.path(), &mut |_, _| {}).unwrap_err();
    // Refused before any connection; not a retryable error.
    assert!(
        matches!(&err, CodecError::Network(m) if m.to_lowercase().contains("https")),
        "{err}"
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
    // The real endpoint is HTTPS only.
    let c = Endpoint::cisco();
    assert!(c.https_only && c.base_url.starts_with("https://ciscobinary.openh264.org"));
}

/// A directory that passes the checks for an administrator folder.
fn admin_dir_in(t: &tempfile::TempDir) -> PathBuf {
    let admin = t.path().join("admin");
    fs::create_dir_all(&admin).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&admin, fs::Permissions::from_mode(0o755)).unwrap();
    }
    admin
}

#[test]
fn imports_the_administrators_bz2_file_as_a_copy() {
    let lib = fake_lib();
    let art = artifact_for_bytes(&lib);
    let t = tempfile::tempdir().unwrap();
    let admin = admin_dir_in(&t);
    let user = t.path().join("user");
    assert!(matches!(
        import_from_dir(&art, &admin, &user).unwrap(),
        Imported::Nothing
    ));
    fs::write(admin.join(format!("{}.bz2", art.name)), bz2_of_fake()).unwrap();
    let Imported::Done(p) = import_from_dir(&art, &admin, &user).unwrap() else {
        panic!("not imported");
    };
    // The library is loaded from the private copy, never from the administrator folder.
    assert_eq!(p, user.join(art.name));
    assert!(!p.starts_with(&admin));
    assert_eq!(fs::read(p).unwrap(), lib);
}

#[test]
fn import_rejects_a_wrong_file_without_failing_the_folder() {
    let art = artifact_for_bytes(&fake_lib());
    let t = tempfile::tempdir().unwrap();
    let admin = admin_dir_in(&t);
    let mut other = fake_lib();
    other[10] ^= 0xff;
    fs::write(admin.join(art.name), &other).unwrap();
    let user = t.path().join("user");
    let Imported::Rejected(e) = import_from_dir(&art, &admin, &user).unwrap() else {
        panic!("not rejected");
    };
    assert!(matches!(e, CodecError::HashMismatch { .. }), "{e}");
    assert!(!user.join(art.name).exists());
}

#[test]
fn import_rejects_an_oversized_bz2() {
    let art = artifact_for_bytes(&fake_lib());
    let t = tempfile::tempdir().unwrap();
    let admin = admin_dir_in(&t);
    let big = fs::File::create(admin.join(format!("{}.bz2", art.name))).unwrap();
    big.set_len(MAX_COMPRESSED_BYTES + 1).unwrap();
    let Imported::Rejected(e) = import_from_dir(&art, &admin, &t.path().join("u")).unwrap() else {
        panic!("not rejected");
    };
    assert!(matches!(e, CodecError::TooLarge), "{e}");
}

#[test]
fn a_bad_admin_file_does_not_stop_the_download() {
    let lib = fake_lib();
    let art = artifact_for_bytes(&lib);
    let t = tempfile::tempdir().unwrap();
    let admin = admin_dir_in(&t);
    fs::write(admin.join(format!("{}.bz2", art.name)), b"not bzip2").unwrap();
    let user = t.path().join("user");
    let (url, count) = serve(vec![ok(bz2_of_fake())]);
    let p = run_in(&test_endpoint(url), &art, Some(admin.clone()), &user).unwrap();
    assert_eq!(fs::read(p).unwrap(), lib);
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // If the download fails too, the message tells about both.
    let t2 = tempfile::tempdir().unwrap();
    let (url, _) = serve(vec![status("404 Not Found", "")]);
    let err = run_in(
        &test_endpoint(url),
        &art,
        Some(admin),
        &t2.path().join("user"),
    )
    .unwrap_err();
    let CodecError::AfterAdminError { source, admin } = &err else {
        panic!("{err}");
    };
    assert!(matches!(**source, CodecError::Http(404)));
    assert!(admin.contains("unexpected format"), "{admin}");
    assert!(err.to_string().contains("administrator folder"));
}

#[test]
fn a_relative_admin_dir_is_a_clear_error_and_nothing_is_downloaded() {
    let art = artifact_for_bytes(&fake_lib());
    let t = tempfile::tempdir().unwrap();
    let (url, count) = serve(vec![ok(bz2_of_fake())]);
    let err = run_in(
        &test_endpoint(url),
        &art,
        Some(PathBuf::from("relative/codecs")),
        &t.path().join("user"),
    )
    .unwrap_err();
    assert!(
        matches!(&err, CodecError::BadConfig(m) if m.contains("absolute")),
        "{err}"
    );
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

#[cfg(unix)]
#[test]
fn an_admin_dir_that_others_can_write_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let art = artifact_for_bytes(&fake_lib());
    let t = tempfile::tempdir().unwrap();
    let admin = admin_dir_in(&t);
    fs::write(admin.join(art.name), fake_lib()).unwrap();
    for mode in [0o775, 0o777] {
        fs::set_permissions(&admin, fs::Permissions::from_mode(mode)).unwrap();
        let err = import_from_dir(&art, &admin, &t.path().join("user")).unwrap_err();
        assert!(matches!(err, CodecError::Unsafe(_)), "{mode:o}: {err}");
    }
    // A world-writable file in a good folder is a bad file: rejected, not a folder error.
    fs::set_permissions(&admin, fs::Permissions::from_mode(0o755)).unwrap();
    fs::set_permissions(admin.join(art.name), fs::Permissions::from_mode(0o666)).unwrap();
    let Imported::Rejected(e) = import_from_dir(&art, &admin, &t.path().join("user")).unwrap()
    else {
        panic!("not rejected");
    };
    assert!(matches!(e, CodecError::Unsafe(_)), "{e}");
}

#[test]
fn a_stalled_download_fails_after_the_stall_timeout() {
    let art = artifact_for_bytes(&fake_lib());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let _ = s.read(&mut buf);
        let head = "HTTP/1.1 200 OK\r\nContent-Length: 695\r\nConnection: close\r\n\r\n";
        let _ = s.write_all(head.as_bytes());
        let _ = s.write_all(&bz2_of_fake()[..100]);
        // Then nothing, but the connection stays open.
        std::thread::sleep(Duration::from_secs(3));
    });
    let mut ep = test_endpoint(url);
    ep.retries = 0;
    ep.stall_timeout = Duration::from_millis(300);
    let t = tempfile::tempdir().unwrap();
    let start = std::time::Instant::now();
    let err = install_from_network(&ep, &art, t.path(), &mut |_, _| {}).unwrap_err();
    assert!(
        matches!(&err, CodecError::Network(m) if m.contains("stalled")),
        "{err}"
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn a_slow_download_that_makes_progress_finishes() {
    let lib = fake_lib();
    let art = artifact_for_bytes(&lib);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let _ = s.read(&mut buf);
        let body = bz2_of_fake();
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = s.write_all(head.as_bytes());
        // Six pieces, 150 ms apart: 750 ms in all, longer than the stall timeout of 400 ms.
        for piece in body.chunks(body.len() / 6 + 1) {
            let _ = s.write_all(piece);
            let _ = s.flush();
            std::thread::sleep(Duration::from_millis(150));
        }
    });
    let mut ep = test_endpoint(url);
    ep.retries = 0;
    ep.stall_timeout = Duration::from_millis(400);
    let t = tempfile::tempdir().unwrap();
    let p = install_from_network(&ep, &art, t.path(), &mut |_, _| {}).unwrap();
    assert_eq!(fs::read(p).unwrap(), lib);
}

#[test]
fn the_real_endpoint_has_the_documented_limits() {
    let c = Endpoint::cisco();
    assert_eq!(c.stall_timeout, Duration::from_secs(30));
    assert_eq!(c.total_timeout, Duration::from_secs(600));
}

#[test]
fn messages_name_files_not_folders() {
    let e = CodecError::SymlinkRefused(PathBuf::from("/home/someone/private/codecs/lib.so"));
    let m = e.to_string();
    assert!(m.contains("lib.so") && !m.contains("someone"), "{m}");
}

/// Real download from Cisco into a temporary directory (needs Internet and a TLS path that
/// webpki roots accept: not an intercepting proxy). Run with `--ignored`.
#[test]
#[ignore = "needs access to ciscobinary.openh264.org"]
fn real_cisco_download_matches_the_pinned_hash() {
    let art = current_artifact().expect("platform has a library");
    let t = tempfile::tempdir().unwrap();
    let p = install_from_network(&Endpoint::cisco(), &art, t.path(), &mut |_, _| {}).unwrap();
    assert!(p.is_file());
}

/// The whole public path against Cisco: `ensure_codec` stores the library below the data
/// directory, and `codec_status` and `find_library` then report it. Run with `--ignored`.
#[test]
#[ignore = "needs access to ciscobinary.openh264.org"]
fn real_ensure_codec_end_to_end() {
    let t = tempfile::tempdir().unwrap();
    crate::set_data_dir(Some(t.path().to_path_buf()));
    assert_eq!(codec_status(), CodecStatus::Missing);
    let status = ensure_codec();
    let CodecStatus::Ready { path } = status else {
        panic!("not ready: {status:?}");
    };
    assert!(path.starts_with(t.path().to_str().unwrap()));
    assert_eq!(find_library().unwrap().origin, crate::Origin::UserDir);
    crate::set_data_dir(None);
}

//! The canonical tar (docs/spec/package-format.md, "Container"): a POSIX ustar stream fully
//! determined by the package's files.
//!
//! Canonical form, as written by [`emit`]:
//!
//! - one regular-file entry (`typeflag '0'`) per file, in the order given (the caller sorts them
//!   bytewise by path); no directory entries;
//! - every header: ustar magic `ustar\0` version `00`, mode `0000644`, uid/gid `0000000`, mtime
//!   `00000000000`, size as 11 octal digits, empty link name and user/group names, devmajor/devminor
//!   `0000000`, empty `prefix`, checksum as 6 octal digits followed by NUL and space;
//! - a path of at most 100 bytes goes in the `name` field. A longer path is preceded by a PAX
//!   extended header (`typeflag 'x'`, name `././@PaxHeader`, same fields otherwise) holding exactly
//!   one `path` record, and the file's own `name` field holds the path truncated to its longest
//!   prefix of at most 100 bytes that ends on a character boundary;
//! - entry data padded with zeros to a 512-byte boundary;
//! - exactly two zero blocks at the end, and nothing after them.
//!
//! [`read`] parses any ustar stream of regular files and PAX `path` records (rejecting everything
//! else) without panicking on malformed input; callers then require that re-emitting the parsed
//! files reproduces the input byte for byte, which rejects every non-canonical detail.

use crate::PackageFile;

/// tar block size.
const BLOCK: usize = 512;
/// Size of the ustar `name` field.
const NAME_LEN: usize = 100;
/// Name of the PAX extended header entry.
const PAX_NAME: &[u8] = b"././@PaxHeader";
/// Largest PAX extended header data accepted when reading (paths are at most 1 KiB).
const MAX_PAX_BYTES: u64 = 8 * 1024;
/// Zero padding source.
const ZEROS: [u8; 2 * BLOCK] = [0; 2 * BLOCK];

/// Feed the canonical tar of `files` (already sorted) to `sink`, chunk by chunk. Streaming lets
/// callers hash or compare the tar without materialising it.
pub(crate) fn emit(files: &[PackageFile], mut sink: impl FnMut(&[u8])) {
    for file in files {
        let path = file.path.as_bytes();
        let name = if path.len() > NAME_LEN {
            let record = pax_record("path", &file.path);
            sink(&header(PAX_NAME, record.len() as u64, b'x'));
            sink(&record);
            sink(&ZEROS[..padding(record.len())]);
            truncate_name(&file.path)
        } else {
            path
        };
        sink(&header(name, file.bytes.len() as u64, b'0'));
        sink(&file.bytes);
        sink(&ZEROS[..padding(file.bytes.len())]);
    }
    sink(&ZEROS);
}

/// The exact length of the canonical tar of `files`.
pub(crate) fn len(files: &[PackageFile]) -> usize {
    let entries: usize = files
        .iter()
        .map(|file| {
            let pax = if file.path.len() > NAME_LEN {
                BLOCK + padded(pax_record("path", &file.path).len())
            } else {
                0
            };
            pax + BLOCK + padded(file.bytes.len())
        })
        .sum();
    entries + 2 * BLOCK
}

/// The canonical tar of `files` as one buffer.
pub(crate) fn to_vec(files: &[PackageFile]) -> Vec<u8> {
    let mut out = Vec::with_capacity(len(files));
    emit(files, |chunk| out.extend_from_slice(chunk));
    out
}

/// Whether `bytes` is exactly the canonical tar of `files`, compared while streaming.
pub(crate) fn is_canonical(files: &[PackageFile], bytes: &[u8]) -> bool {
    let mut offset = 0usize;
    let mut equal = true;
    emit(files, |chunk| {
        if equal {
            equal = bytes.get(offset..offset + chunk.len()) == Some(chunk);
        }
        offset += chunk.len();
    });
    equal && offset == bytes.len()
}

/// Zero bytes after `len` data bytes up to the next block boundary.
fn padding(len: usize) -> usize {
    (BLOCK - len % BLOCK) % BLOCK
}

/// `len` rounded up to a whole number of blocks.
fn padded(len: usize) -> usize {
    len + padding(len)
}

/// The longest prefix of `path` of at most [`NAME_LEN`] bytes ending on a character boundary.
fn truncate_name(path: &str) -> &[u8] {
    let mut end = NAME_LEN.min(path.len());
    while !path.is_char_boundary(end) {
        end -= 1;
    }
    &path.as_bytes()[..end]
}

/// One PAX record: `"<len> <key>=<value>\n"`, where `<len>` counts the whole record including
/// its own digits.
fn pax_record(key: &str, value: &str) -> Vec<u8> {
    let rest = 1 + key.len() + 1 + value.len() + 1;
    let mut len = rest + 1;
    while len != rest + decimal_digits(len) {
        len = rest + decimal_digits(len);
    }
    format!("{len} {key}={value}\n").into_bytes()
}

fn decimal_digits(mut n: usize) -> usize {
    let mut digits = 1;
    while n >= 10 {
        n /= 10;
        digits += 1;
    }
    digits
}

/// A canonical header block.
fn header(name: &[u8], size: u64, typeflag: u8) -> [u8; BLOCK] {
    debug_assert!(name.len() <= NAME_LEN);
    debug_assert!(size < 8u64.pow(11));
    let mut h = [0u8; BLOCK];
    h[..name.len()].copy_from_slice(name);
    h[100..108].copy_from_slice(b"0000644\0");
    h[108..116].copy_from_slice(b"0000000\0");
    h[116..124].copy_from_slice(b"0000000\0");
    write_octal(&mut h[124..135], size);
    h[136..148].copy_from_slice(b"00000000000\0");
    h[148..156].fill(b' ');
    h[156] = typeflag;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    h[329..337].copy_from_slice(b"0000000\0");
    h[337..345].copy_from_slice(b"0000000\0");
    let sum = checksum(&h);
    write_octal(&mut h[148..154], u64::from(sum));
    h[154] = 0;
    h[155] = b' ';
    h
}

/// Zero-padded octal digits filling `field` exactly.
fn write_octal(field: &mut [u8], mut value: u64) {
    for byte in field.iter_mut().rev() {
        *byte = b'0' + (value & 7) as u8;
        value >>= 3;
    }
}

/// The header checksum: the byte sum with the checksum field counted as spaces.
fn checksum(h: &[u8]) -> u32 {
    let sum: u32 = h.iter().map(|&b| u32::from(b)).sum();
    let field: u32 = h[148..156].iter().map(|&b| u32::from(b)).sum();
    sum - field + 8 * u32::from(b' ')
}

/// Why [`read`] stopped.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ReadError {
    /// More regular-file entries than allowed.
    TooManyFiles(usize),
    /// Malformed or unsupported.
    Malformed(String),
}

impl From<String> for ReadError {
    fn from(message: String) -> Self {
        ReadError::Malformed(message)
    }
}

impl From<&str> for ReadError {
    fn from(message: &str) -> Self {
        ReadError::Malformed(message.to_string())
    }
}

/// Parse a tar stream of regular files (with optional PAX `path` headers). Stops with an error
/// at the first malformed or unsupported header, before reading more than `max_files` entries.
/// Never panics, whatever the input.
pub(crate) fn read(tar: &[u8], max_files: usize) -> Result<Vec<PackageFile>, ReadError> {
    let mut files = Vec::new();
    let mut pos = 0usize;
    let mut pax_path: Option<String> = None;
    loop {
        let block = tar
            .get(pos..pos + BLOCK)
            .ok_or("truncated tar stream (missing the end-of-archive blocks)")?;
        if block.iter().all(|&b| b == 0) {
            let second = tar
                .get(pos + BLOCK..pos + 2 * BLOCK)
                .ok_or("missing the second end-of-archive block")?;
            if second.iter().any(|&b| b != 0) {
                return Err("missing the second end-of-archive block".into());
            }
            if pos + 2 * BLOCK != tar.len() {
                return Err("data after the end-of-archive blocks".into());
            }
            if pax_path.is_some() {
                return Err("a PAX header is not followed by a file".into());
            }
            return Ok(files);
        }
        let entry = files.len() + 1;
        let stored = parse_octal(&block[148..156])
            .ok_or_else(|| format!("entry {entry}: malformed header checksum"))?;
        if stored != u64::from(checksum(block)) {
            return Err(format!("entry {entry}: header checksum mismatch").into());
        }
        if &block[257..263] != b"ustar\0" || &block[263..265] != b"00" {
            return Err(format!("entry {entry}: not a POSIX ustar header").into());
        }
        for (range, field, expected) in [
            (100..108, "mode", 0o644),
            (108..116, "uid", 0),
            (116..124, "gid", 0),
            (136..148, "mtime", 0),
        ] {
            if parse_octal(&block[range]) != Some(expected) {
                return Err(format!("entry {entry}: {field} must be {expected:o}").into());
            }
        }
        let size = parse_octal(&block[124..136])
            .ok_or_else(|| format!("entry {entry}: malformed size"))?;
        let data_start = pos + BLOCK;
        let data_end = usize::try_from(size)
            .ok()
            .and_then(|size| data_start.checked_add(size))
            .filter(|&end| end <= tar.len())
            .ok_or_else(|| format!("entry {entry}: truncated data"))?;
        let next = data_end
            .checked_add(padding(data_end))
            .filter(|&next| next <= tar.len())
            .ok_or_else(|| format!("entry {entry}: truncated padding"))?;
        let data = &tar[data_start..data_end];
        match block[156] {
            b'x' => {
                if pax_path.is_some() {
                    return Err(format!("entry {entry}: two PAX headers in a row").into());
                }
                if size > MAX_PAX_BYTES {
                    return Err(format!("entry {entry}: PAX header too large").into());
                }
                pax_path = Some(parse_pax(data).map_err(|e| format!("entry {entry}: {e}"))?);
            }
            b'0' => {
                if files.len() == max_files {
                    return Err(ReadError::TooManyFiles(max_files));
                }
                let path = match pax_path.take() {
                    Some(path) => path,
                    None => header_path(block)
                        .ok_or_else(|| format!("entry {entry}: path is not valid UTF-8"))?,
                };
                files.push(PackageFile {
                    path,
                    bytes: data.to_vec(),
                });
            }
            other => {
                return Err(format!(
                    "entry {entry}: not a regular file (type {:?})",
                    char::from(other)
                )
                .into());
            }
        }
        pos = next;
    }
}

/// `prefix/name` (or `name` when the prefix is empty) from a ustar header.
fn header_path(block: &[u8]) -> Option<String> {
    let field = |bytes: &[u8]| -> Option<String> {
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        String::from_utf8(bytes[..end].to_vec()).ok()
    };
    let name = field(&block[..NAME_LEN])?;
    let prefix = field(&block[345..500])?;
    Some(if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    })
}

/// An octal header field: digits, then NUL/space padding. `None` for anything else (including
/// the base-256 extension and overflow).
fn parse_octal(field: &[u8]) -> Option<u64> {
    let end = field
        .iter()
        .position(|&b| b == 0 || b == b' ')
        .unwrap_or(field.len());
    let (digits, rest) = field.split_at(end);
    if digits.is_empty() || rest.iter().any(|&b| b != 0 && b != b' ') {
        return None;
    }
    digits.iter().try_fold(0u64, |acc, &b| match b {
        b'0'..=b'7' => acc.checked_mul(8)?.checked_add(u64::from(b - b'0')),
        _ => None,
    })
}

/// The `path` of a PAX extended header holding exactly one `path` record.
fn parse_pax(data: &[u8]) -> Result<String, String> {
    let mut path = None;
    let mut rest = data;
    while !rest.is_empty() {
        let space = rest
            .iter()
            .position(|&b| b == b' ')
            .ok_or("malformed PAX record")?;
        let len: usize = std::str::from_utf8(&rest[..space])
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or("malformed PAX record length")?;
        if len <= space + 1 || len > rest.len() || rest[len - 1] != b'\n' {
            return Err("malformed PAX record".into());
        }
        let record = &rest[space + 1..len - 1];
        let eq = record
            .iter()
            .position(|&b| b == b'=')
            .ok_or("malformed PAX record")?;
        let (key, value) = (&record[..eq], &record[eq + 1..]);
        if key != b"path" {
            return Err(format!(
                "unsupported PAX record `{}`",
                String::from_utf8_lossy(key)
            ));
        }
        if path.is_some() {
            return Err("duplicate PAX path record".into());
        }
        path = Some(String::from_utf8(value.to_vec()).map_err(|_| "PAX path is not valid UTF-8")?);
        rest = &rest[len..];
    }
    path.ok_or_else(|| "PAX header without a path record".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, bytes: &[u8]) -> PackageFile {
        PackageFile {
            path: path.to_string(),
            bytes: bytes.to_vec(),
        }
    }

    #[test]
    fn header_layout() {
        let tar = to_vec(&[file("mod.toml", b"format = 1\n")]);
        assert_eq!(tar.len(), 512 + 512 + 1024);
        let h = &tar[..512];
        assert_eq!(&h[..8], b"mod.toml");
        assert!(h[8..100].iter().all(|&b| b == 0));
        assert_eq!(&h[100..108], b"0000644\0");
        assert_eq!(&h[108..116], b"0000000\0");
        assert_eq!(&h[116..124], b"0000000\0");
        assert_eq!(&h[124..136], b"00000000013\0");
        assert_eq!(&h[136..148], b"00000000000\0");
        assert_eq!(h[156], b'0');
        assert_eq!(&h[257..265], b"ustar\x0000");
        assert!(
            h[265..329].iter().all(|&b| b == 0),
            "empty user and group names"
        );
        assert_eq!(parse_octal(&h[148..156]), Some(u64::from(checksum(h))));
        assert_eq!(&tar[512..523], b"format = 1\n");
        assert!(tar[523..].iter().all(|&b| b == 0));
    }

    #[test]
    fn checksum_matches_reference_tar() {
        // The `tar` crate computes the checksum independently.
        let tar = to_vec(&[file("src/lib.rs", b"pub fn register() {}\n")]);
        let ours = ::tar::Header::from_byte_slice(&tar[..512]);
        let mut theirs = ours.clone();
        theirs.set_cksum();
        assert_eq!(ours.cksum().unwrap(), theirs.cksum().unwrap());
    }

    #[test]
    fn pax_records() {
        // The length counts itself, including when adding a digit changes it.
        for value_len in [
            0usize, 1, 2, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 990, 991, 992, 993, 994,
            995, 996, 1000,
        ] {
            let value = "a".repeat(value_len);
            let record = pax_record("path", &value);
            let text = String::from_utf8(record.clone()).unwrap();
            let (len, _) = text.split_once(' ').unwrap();
            assert_eq!(len.parse::<usize>().unwrap(), record.len(), "{text:?}");
            assert_eq!(parse_pax(&record).unwrap(), value);
        }
    }

    #[test]
    fn long_paths_use_pax_and_round_trip() {
        let long = format!("assets/{}/ü{}.png", "d".repeat(80), "x".repeat(40));
        assert!(long.len() > NAME_LEN);
        let exactly_100 = format!("src/{}", "n".repeat(96));
        assert_eq!(exactly_100.len(), 100);
        let files = vec![
            file(&long, b"png"),
            file(&exactly_100, b""),
            file("mod.toml", b"m"),
        ];
        let tar = to_vec(&files);
        assert_eq!(tar.len(), len(&files));
        assert_eq!(read(&tar, 10).unwrap(), files);
        assert!(is_canonical(&files, &tar));
        // The first entry is a PAX header.
        assert_eq!(tar[156], b'x');
        assert_eq!(&tar[..PAX_NAME.len()], PAX_NAME);
        // Truncation never splits a character.
        let multibyte = format!("{}ü", "a".repeat(99));
        assert_eq!(truncate_name(&multibyte).len(), 99);
    }

    #[test]
    fn standard_tar_readers_agree() {
        let long = format!("data/{}/end.txt", "deep/".repeat(30));
        let files = vec![
            file("README.md", b"# Hi\n"),
            file(&long, &[7u8; 1500]),
            file("src/lib.rs", b""),
        ];
        let tar = to_vec(&files);
        let mut archive = ::tar::Archive::new(&tar[..]);
        let mut seen = Vec::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let header = entry.header();
            assert_eq!(header.entry_type(), ::tar::EntryType::Regular);
            assert_eq!(header.mode().unwrap(), 0o644);
            assert_eq!(header.mtime().unwrap(), 0);
            assert_eq!(header.uid().unwrap(), 0);
            assert_eq!(header.gid().unwrap(), 0);
            assert_eq!(header.username().unwrap(), Some(""));
            let path = entry.path().unwrap().to_str().unwrap().to_string();
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
            seen.push(file(&path, &bytes));
        }
        assert_eq!(seen, files);
    }

    #[test]
    fn reader_rejects_malformed_streams() {
        let files = vec![file("a", b"hello"), file("b", b"world")];
        let tar = to_vec(&files);
        assert!(read(&tar, 10).is_ok());
        assert_eq!(read(&tar, 1).unwrap_err(), ReadError::TooManyFiles(1));
        // Truncations at every block boundary and a few odd offsets.
        for cut in (0..tar.len()).step_by(97).chain([
            511,
            512,
            513,
            tar.len() - 1024,
            tar.len() - 512,
            tar.len() - 1,
        ]) {
            assert!(read(&tar[..cut], 10).is_err(), "cut at {cut}");
        }
        // Trailing data.
        let mut extra = tar.clone();
        extra.extend_from_slice(&[0u8; 512]);
        assert_eq!(
            read(&extra, 10).unwrap_err(),
            ReadError::Malformed("data after the end-of-archive blocks".into())
        );
        // Corrupted header byte → checksum mismatch.
        let mut bad = tar.clone();
        bad[0] = b'z';
        assert!(malformed(read(&bad, 10)).contains("checksum"));
        // Directory entry with a valid checksum.
        let mut dir = tar.clone();
        dir[156] = b'5';
        fix_checksum(&mut dir[..512]);
        assert!(malformed(read(&dir, 10)).contains("not a regular file"));
        // Symlink entry.
        let mut link = tar.clone();
        link[156] = b'2';
        fix_checksum(&mut link[..512]);
        assert!(malformed(read(&link, 10)).contains("not a regular file"));
        // Non-zero mtime.
        let mut mtime = tar.clone();
        mtime[136..148].copy_from_slice(b"14000000000\0");
        fix_checksum(&mut mtime[..512]);
        assert!(malformed(read(&mtime, 10)).contains("mtime"));
        // Huge size.
        let mut huge = tar.clone();
        huge[124..136].copy_from_slice(b"77777777777\0");
        fix_checksum(&mut huge[..512]);
        assert!(malformed(read(&huge, 10)).contains("truncated"));
        // Base-256 size.
        let mut b256 = tar.clone();
        b256[124] = 0x80;
        fix_checksum(&mut b256[..512]);
        assert!(malformed(read(&b256, 10)).contains("size"));
    }

    #[test]
    fn non_canonical_details_are_detected_by_comparison() {
        let files = vec![file("src/lib.rs", b"x")];
        let tar = to_vec(&files);
        // A user name, readable but not canonical.
        let mut named = tar.clone();
        named[265..269].copy_from_slice(b"root");
        fix_checksum(&mut named[..512]);
        let parsed = read(&named, 10).unwrap();
        assert_eq!(parsed, files);
        assert!(!is_canonical(&parsed, &named));
        // The same path split into ustar prefix + name.
        let mut split = tar.clone();
        split[..100].fill(0);
        split[..6].copy_from_slice(b"lib.rs");
        split[345..348].copy_from_slice(b"src");
        fix_checksum(&mut split[..512]);
        let parsed = read(&split, 10).unwrap();
        assert_eq!(parsed, files);
        assert!(!is_canonical(&parsed, &split));
        // Different octal formatting of the mode.
        let mut mode = tar.clone();
        mode[100..108].copy_from_slice(b"000644 \0");
        fix_checksum(&mut mode[..512]);
        let parsed = read(&mode, 10).unwrap();
        assert!(!is_canonical(&parsed, &mode));
        assert!(is_canonical(&files, &tar));
    }

    #[test]
    fn pax_parsing_is_strict() {
        assert!(parse_pax(b"").is_err());
        assert!(parse_pax(b"13 path=abcd\n").is_ok());
        assert!(parse_pax(b"12 path=abcd\n").is_err());
        assert!(parse_pax(b"99 path=abcd\n").is_err());
        assert!(parse_pax(b"13 path=abcdX").is_err());
        assert!(parse_pax(b"x path=abcd\n").is_err());
        assert!(
            parse_pax(b"13 mtim=abcd\n")
                .unwrap_err()
                .contains("unsupported")
        );
        assert!(
            parse_pax(b"13 path=abcd\n13 path=abcd\n")
                .unwrap_err()
                .contains("duplicate")
        );
        assert!(parse_pax(b"11 pathabcd\n").is_err());
        assert!(parse_pax(b"1 \n").is_err());
        assert!(parse_pax(b"0 path=\n").is_err());
        assert!(parse_pax(b"12 path=\xff\xfe\n").is_err());
    }

    fn malformed(result: Result<Vec<PackageFile>, ReadError>) -> String {
        match result {
            Err(ReadError::Malformed(message)) => message,
            other => panic!("expected a malformed-tar error, got {other:?}"),
        }
    }

    fn fix_checksum(h: &mut [u8]) {
        h[148..156].fill(b' ');
        let sum = checksum(h);
        write_octal(&mut h[148..154], u64::from(sum));
        h[154] = 0;
        h[155] = b' ';
    }
}

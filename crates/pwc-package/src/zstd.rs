//! The Zstandard container (docs/spec/package-format.md, "Container"): exactly one frame. The
//! compression level is not part of the format; reading accepts any valid frame.

use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};
use ruzstd::encoding::{CompressionLevel, compress_to_vec};

/// Bytes decoded per step before the output size is checked (a frame's blocks are at most
/// 128 KiB each, so the output never overshoots the limit by more than this plus one block).
const STEP: usize = 1 << 20;

/// Compress into one Zstandard frame.
pub(crate) fn compress(data: &[u8]) -> Vec<u8> {
    compress_to_vec(data, CompressionLevel::Fastest)
}

/// Decompress exactly one Zstandard frame, refusing output beyond `limit` bytes (decompression
/// bombs), trailing bytes after the frame, skippable or dictionary frames and checksum
/// mismatches. Never panics on malformed input (ruzstd reports errors instead).
pub(crate) fn decompress(data: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let mut source = data;
    let mut decoder = FrameDecoder::new();
    decoder
        .reset(&mut source)
        .map_err(|e| format!("not a Zstandard frame ({e})"))?;
    let mut out = Vec::new();
    loop {
        let finished = decoder
            .decode_blocks(&mut source, BlockDecodingStrategy::UptoBytes(STEP))
            .map_err(|e| format!("corrupt Zstandard data ({e})"))?;
        decoder
            .collect_to_writer(&mut out)
            .map_err(|e| format!("corrupt Zstandard data ({e})"))?;
        if out.len() > limit {
            return Err(format!(
                "decompresses to more than {limit} bytes, beyond the package size limits"
            ));
        }
        if finished {
            break;
        }
    }
    if let Some(stored) = decoder.get_checksum_from_data()
        && decoder.get_calculated_checksum() != Some(stored)
    {
        return Err("Zstandard checksum mismatch".into());
    }
    if !source.is_empty() {
        return Err("data after the Zstandard frame (a .pwcmod is exactly one frame)".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(len: usize) -> Vec<u8> {
        // Compressible but not trivial.
        (0..len)
            .map(|i| ((i * 7) % 251) as u8 ^ ((i >> 10) as u8))
            .collect()
    }

    #[test]
    fn round_trip() {
        for len in [0, 1, 511, 4096, 200_000, 3_000_000] {
            let data = sample(len);
            let compressed = compress(&data);
            assert_eq!(decompress(&compressed, usize::MAX).unwrap(), data, "{len}");
        }
    }

    #[test]
    fn compression_is_deterministic() {
        let data = sample(500_000);
        assert_eq!(compress(&data), compress(&data));
    }

    #[test]
    fn uncompressed_frames_are_accepted() {
        // Any valid frame is accepted, whatever the level that made it.
        let data = sample(300_000);
        let raw = compress_to_vec(&data[..], CompressionLevel::Uncompressed);
        assert_eq!(decompress(&raw, usize::MAX).unwrap(), data);
    }

    #[test]
    fn limit_is_enforced() {
        let data = vec![0u8; 5_000_000];
        let compressed = compress(&data);
        assert!(compressed.len() < 100_000, "zeros compress well");
        let err = decompress(&compressed, 1_000_000).unwrap_err();
        assert!(err.contains("more than"), "{err}");
        assert_eq!(decompress(&compressed, 5_000_000).unwrap().len(), 5_000_000);
    }

    #[test]
    fn rejects_trailing_data_and_second_frames() {
        let one = compress(b"hello");
        let mut two = one.clone();
        two.extend_from_slice(&one);
        assert!(
            decompress(&two, usize::MAX)
                .unwrap_err()
                .contains("after the Zstandard frame")
        );
        let mut junk = one.clone();
        junk.push(0);
        assert!(decompress(&junk, usize::MAX).is_err());
    }

    #[test]
    fn rejects_skippable_frames_and_garbage() {
        // A skippable frame (magic 0x184D2A50) with 4 bytes of payload.
        let skippable = [0x50, 0x2A, 0x4D, 0x18, 4, 0, 0, 0, 1, 2, 3, 4];
        assert!(decompress(&skippable, usize::MAX).is_err());
        assert!(decompress(&[], usize::MAX).is_err());
        assert!(decompress(b"not zstd at all", usize::MAX).is_err());
        assert!(decompress(&[0x28, 0xB5, 0x2F, 0xFD], usize::MAX).is_err());
    }

    #[test]
    fn truncations_and_bit_flips_never_panic() {
        let data = sample(100_000);
        let compressed = compress(&data);
        for cut in 0..compressed.len().min(400) {
            assert!(
                decompress(&compressed[..cut], usize::MAX).is_err(),
                "cut at {cut}"
            );
        }
        for cut in (0..compressed.len()).step_by(37) {
            let _ = decompress(&compressed[..cut], usize::MAX);
        }
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..2000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mut corrupt = compressed.clone();
            let at = (state as usize) % corrupt.len();
            corrupt[at] ^= 1 << ((state >> 32) % 8);
            // Either an error or some output; never a panic, never beyond the limit.
            if let Ok(out) = decompress(&corrupt, 200_000) {
                assert!(out.len() <= 200_000);
            }
        }
    }
}

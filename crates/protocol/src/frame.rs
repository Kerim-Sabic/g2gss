use bytes::BytesMut;
use thiserror::Error;

/// Number of bytes in the fixed media frame header.
pub const FRAME_HEADER_LEN: usize = 24;
/// Marks a frame as containing a keyframe.
pub const FLAG_KEYFRAME: u32 = 1 << 0;
/// Marks a frame as part of an intra-refresh sequence.
pub const FLAG_INTRA_REFRESH: u32 = 1 << 1;
/// Marks the final slice for a frame sequence number.
pub const FLAG_LAST_SLICE: u32 = 1 << 2;
/// Maximum flag bits preserved by the fixed header packing.
pub const MAX_ENCODED_FLAGS: u32 = (1 << 14) - 1;

const CODEC_SHIFT: u32 = 14;
const SLICE_IDX_SHIFT: u32 = 16;
const SLICE_COUNT_SHIFT: u32 = 24;
const CODEC_MASK: u32 = 0b11;

/// Codec identifier carried by a media frame header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Codec {
    /// H.264/AVC video.
    H264 = 0,
    /// H.265/HEVC video.
    Hevc = 1,
    /// AV1 video.
    Av1 = 2,
}

impl TryFrom<u8> for Codec {
    type Error = FrameHeaderError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::H264),
            1 => Ok(Self::Hevc),
            2 => Ok(Self::Av1),
            other => Err(FrameHeaderError::InvalidCodec(other)),
        }
    }
}

/// Errors returned by fixed frame header decoding.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FrameHeaderError {
    /// The buffer ended before a complete fixed-size header was available.
    #[error("frame header truncated: needed {needed} bytes, got {actual}")]
    Truncated {
        /// Required byte length.
        needed: usize,
        /// Actual byte length.
        actual: usize,
    },
    /// The packed codec field does not map to a supported codec.
    #[error("invalid frame header codec: {0}")]
    InvalidCodec(u8),
}

/// Fixed-size media frame header used on the datagram hot path.
///
/// The public accessors expose the semantic fields from `proto/frame.proto`.
/// The in-memory representation packs codec, flags, and slice metadata into a
/// single control word so the header remains exactly 24 bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct FrameHeader {
    seq: u64,
    pts_us: u64,
    stream_id: u32,
    control: u32,
}

impl FrameHeader {
    /// Creates a fixed-size frame header.
    pub const fn new(
        stream_id: u32,
        seq: u64,
        pts_us: u64,
        codec: Codec,
        flags: u32,
        slice_idx: u8,
        slice_count: u8,
    ) -> Self {
        Self {
            seq,
            pts_us,
            stream_id,
            control: pack_control(codec, flags, slice_idx, slice_count),
        }
    }

    /// Returns the media stream identifier.
    pub const fn stream_id(self) -> u32 {
        self.stream_id
    }

    /// Returns the monotonically increasing frame sequence number.
    pub const fn seq(self) -> u64 {
        self.seq
    }

    /// Returns the presentation timestamp in microseconds.
    pub const fn pts_us(self) -> u64 {
        self.pts_us
    }

    /// Returns the packed codec.
    pub fn codec(self) -> Result<Codec, FrameHeaderError> {
        Codec::try_from(((self.control >> CODEC_SHIFT) & CODEC_MASK) as u8)
    }

    /// Returns the frame flags.
    pub const fn flags(self) -> u32 {
        self.control & MAX_ENCODED_FLAGS
    }

    /// Returns the zero-based slice index.
    pub const fn slice_idx(self) -> u8 {
        ((self.control >> SLICE_IDX_SHIFT) & 0xff) as u8
    }

    /// Returns the total slice count for the frame.
    pub const fn slice_count(self) -> u8 {
        ((self.control >> SLICE_COUNT_SHIFT) & 0xff) as u8
    }

    /// Encodes this header into a preallocated byte buffer.
    ///
    /// Returns `0` and leaves `dst` untouched when the spare capacity is less
    /// than [`FRAME_HEADER_LEN`]. Otherwise, appends 24 bytes and returns 24.
    pub fn encode_to(&self, dst: &mut BytesMut) -> usize {
        if dst.capacity().saturating_sub(dst.len()) < FRAME_HEADER_LEN {
            return 0;
        }

        dst.extend_from_slice(&self.stream_id.to_le_bytes());
        dst.extend_from_slice(&self.seq.to_le_bytes());
        dst.extend_from_slice(&self.pts_us.to_le_bytes());
        dst.extend_from_slice(&self.control.to_le_bytes());

        FRAME_HEADER_LEN
    }

    /// Decodes one fixed header and returns the remaining payload bytes.
    pub fn decode(src: &[u8]) -> Result<(Self, &[u8]), FrameHeaderError> {
        if src.len() < FRAME_HEADER_LEN {
            return Err(FrameHeaderError::Truncated {
                needed: FRAME_HEADER_LEN,
                actual: src.len(),
            });
        }

        let header = &src[..FRAME_HEADER_LEN];
        let payload = &src[FRAME_HEADER_LEN..];
        let stream_id = read_u32_le(&header[0..4]);
        let seq = read_u64_le(&header[4..12]);
        let pts_us = read_u64_le(&header[12..20]);
        let control = read_u32_le(&header[20..24]);
        let frame = Self {
            seq,
            pts_us,
            stream_id,
            control,
        };

        let _ = frame.codec()?;

        Ok((frame, payload))
    }
}

const fn pack_control(codec: Codec, flags: u32, slice_idx: u8, slice_count: u8) -> u32 {
    (flags & MAX_ENCODED_FLAGS)
        | ((codec as u32) << CODEC_SHIFT)
        | ((slice_idx as u32) << SLICE_IDX_SHIFT)
        | ((slice_count as u32) << SLICE_COUNT_SHIFT)
}

fn read_u32_le(src: &[u8]) -> u32 {
    let mut bytes = [0_u8; 4];
    bytes.copy_from_slice(src);
    u32::from_le_bytes(bytes)
}

fn read_u64_le(src: &[u8]) -> u64 {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(src);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
pub mod tests {
    use std::mem::size_of;

    use bytes::BytesMut;

    use super::{
        Codec, FLAG_INTRA_REFRESH, FLAG_KEYFRAME, FLAG_LAST_SLICE, FRAME_HEADER_LEN, FrameHeader,
        FrameHeaderError,
    };

    #[test]
    fn header_is_fixed_size() {
        assert_eq!(size_of::<FrameHeader>(), FRAME_HEADER_LEN);
    }

    #[test]
    fn encode_decode_roundtrip_splits_payload() {
        let header = FrameHeader::new(
            7,
            42,
            1_234_567,
            Codec::Av1,
            FLAG_KEYFRAME | FLAG_INTRA_REFRESH | FLAG_LAST_SLICE,
            3,
            4,
        );
        let mut bytes = BytesMut::with_capacity(FRAME_HEADER_LEN + 3);
        let written = header.encode_to(&mut bytes);
        bytes.extend_from_slice(&[9, 8, 7]);

        let (decoded, payload) = match FrameHeader::decode(&bytes) {
            Ok(decoded) => decoded,
            Err(error) => panic!("{error}"),
        };

        assert_eq!(written, FRAME_HEADER_LEN);
        assert_eq!(decoded, header);
        assert_eq!(decoded.stream_id(), 7);
        assert_eq!(decoded.seq(), 42);
        assert_eq!(decoded.pts_us(), 1_234_567);
        assert_eq!(decoded.codec(), Ok(Codec::Av1));
        assert_eq!(
            decoded.flags(),
            FLAG_KEYFRAME | FLAG_INTRA_REFRESH | FLAG_LAST_SLICE
        );
        assert_eq!(decoded.slice_idx(), 3);
        assert_eq!(decoded.slice_count(), 4);
        assert_eq!(payload, &[9, 8, 7]);
    }

    #[test]
    fn encode_requires_spare_capacity() {
        let header = FrameHeader::new(1, 2, 3, Codec::H264, 0, 0, 1);
        let mut bytes = BytesMut::with_capacity(FRAME_HEADER_LEN - 1);

        assert_eq!(header.encode_to(&mut bytes), 0);
        assert!(bytes.is_empty());
    }

    #[test]
    fn decode_rejects_truncated_input() {
        let error = match FrameHeader::decode(&[0; FRAME_HEADER_LEN - 1]) {
            Ok(_) => panic!("truncated header decoded successfully"),
            Err(error) => error,
        };

        assert_eq!(
            error,
            FrameHeaderError::Truncated {
                needed: FRAME_HEADER_LEN,
                actual: FRAME_HEADER_LEN - 1
            }
        );
    }
}

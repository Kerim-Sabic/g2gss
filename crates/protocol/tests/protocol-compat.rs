use bytes::BytesMut;
use protocol::control::{
    Answer, Bitrate, Candidate, ControlMessage, Hello, InputEvent, Nack, Offer, Ping,
    PlaceholderInput, Pong, RefFrameInvalidate, RoomCode, SeqRange, SignalingMessage,
    control_message, decode_control, decode_signaling, encode_control, encode_signaling,
    input_event, signaling_message,
};
use protocol::frame::{
    Codec, FLAG_INTRA_REFRESH, FLAG_KEYFRAME, FLAG_LAST_SLICE, FRAME_HEADER_LEN, FrameHeader,
};

const FIXTURE: &[u8] = include_bytes!("fixtures/v0_1_0.bin");
const MAGIC: &[u8; 8] = b"STRM0100";
const RECORD_FRAME_HEADER: u8 = 1;
const RECORD_CONTROL: u8 = 2;
const RECORD_SIGNALING: u8 = 3;
const CONTROL_ALL_VARIANTS: u16 = 0b0111_1111;
const SIGNALING_ALL_VARIANTS: u8 = 0b0000_1111;

#[test]
fn v0_1_0_fixture_decodes() {
    let expected = fixture_bytes();
    assert_eq!(FIXTURE, expected.as_slice());

    let mut cursor = FixtureCursor::new(FIXTURE);
    assert_eq!(cursor.take(MAGIC.len()), MAGIC);
    let record_count = cursor.read_u32_le();
    let mut frame_headers = 0_u32;
    let mut control_mask = 0_u16;
    let mut signaling_mask = 0_u8;

    for _ in 0..record_count {
        let tag = cursor.read_u8();
        let len = cursor.read_u32_le() as usize;
        let record = cursor.take(len);

        match tag {
            RECORD_FRAME_HEADER => {
                let (header, payload) = match FrameHeader::decode(record) {
                    Ok(decoded) => decoded,
                    Err(error) => panic!("{error}"),
                };

                assert_eq!(header.stream_id(), 7);
                assert_eq!(header.seq(), 42);
                assert_eq!(header.pts_us(), 1_234_567);
                assert_eq!(header.codec(), Ok(Codec::Av1));
                assert_eq!(
                    header.flags(),
                    FLAG_KEYFRAME | FLAG_INTRA_REFRESH | FLAG_LAST_SLICE
                );
                assert_eq!(header.slice_idx(), 3);
                assert_eq!(header.slice_count(), 4);
                assert!(payload.is_empty());
                frame_headers += 1;
            }
            RECORD_CONTROL => {
                let message = match decode_control(record) {
                    Ok(message) => message,
                    Err(error) => panic!("{error}"),
                };

                control_mask |= control_variant_bit(&message);
            }
            RECORD_SIGNALING => {
                let message = match decode_signaling(record) {
                    Ok(message) => message,
                    Err(error) => panic!("{error}"),
                };

                signaling_mask |= signaling_variant_bit(&message);
            }
            other => panic!("unknown fixture record tag {other}"),
        }
    }

    assert_eq!(frame_headers, 1);
    assert_eq!(control_mask, CONTROL_ALL_VARIANTS);
    assert_eq!(signaling_mask, SIGNALING_ALL_VARIANTS);
    assert!(cursor.is_empty());
}

#[test]
#[ignore = "regenerates the committed compatibility fixture"]
fn regenerate_v0_1_0_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("v0_1_0.bin");

    if let Err(error) = std::fs::write(path, fixture_bytes()) {
        panic!("{error}");
    }
}

fn fixture_bytes() -> Vec<u8> {
    let mut fixture = Vec::new();
    fixture.extend_from_slice(MAGIC);

    let frame = encoded_frame_header();
    let controls = control_messages();
    let signaling = signaling_messages();
    let record_count = 1 + controls.len() + signaling.len();
    fixture.extend_from_slice(&(record_count as u32).to_le_bytes());
    append_record(&mut fixture, RECORD_FRAME_HEADER, &frame);

    for message in controls {
        let mut encoded = Vec::new();
        if let Err(error) = encode_control(&message, &mut encoded) {
            panic!("{error}");
        }
        append_record(&mut fixture, RECORD_CONTROL, &encoded);
    }

    for message in signaling {
        let mut encoded = Vec::new();
        if let Err(error) = encode_signaling(&message, &mut encoded) {
            panic!("{error}");
        }
        append_record(&mut fixture, RECORD_SIGNALING, &encoded);
    }

    fixture
}

fn encoded_frame_header() -> BytesMut {
    let header = FrameHeader::new(
        7,
        42,
        1_234_567,
        Codec::Av1,
        FLAG_KEYFRAME | FLAG_INTRA_REFRESH | FLAG_LAST_SLICE,
        3,
        4,
    );
    let mut encoded = BytesMut::with_capacity(FRAME_HEADER_LEN);

    assert_eq!(header.encode_to(&mut encoded), FRAME_HEADER_LEN);

    encoded
}

fn control_messages() -> [ControlMessage; 7] {
    [
        ControlMessage {
            kind: Some(control_message::Kind::Hello(Hello {
                client_version: "0.1.0".to_owned(),
                capabilities: vec!["av1".to_owned(), "hevc444".to_owned()],
            })),
        },
        ControlMessage {
            kind: Some(control_message::Kind::Bitrate(Bitrate {
                target_bps: 25_000_000,
                ts: 42,
            })),
        },
        ControlMessage {
            kind: Some(control_message::Kind::Nack(Nack {
                stream_id: 3,
                seq_ranges: vec![SeqRange { start: 10, end: 12 }],
            })),
        },
        ControlMessage {
            kind: Some(control_message::Kind::RefFrameInvalidate(
                RefFrameInvalidate { last_good_seq: 99 },
            )),
        },
        ControlMessage {
            kind: Some(control_message::Kind::InputEvent(InputEvent {
                kind: Some(input_event::Kind::Placeholder(PlaceholderInput {})),
            })),
        },
        ControlMessage {
            kind: Some(control_message::Kind::Ping(Ping {
                tai64n: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            })),
        },
        ControlMessage {
            kind: Some(control_message::Kind::Pong(Pong {
                tai64n: vec![11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
            })),
        },
    ]
}

fn signaling_messages() -> [SignalingMessage; 4] {
    [
        SignalingMessage {
            kind: Some(signaling_message::Kind::Offer(Offer {
                sdp_blob: b"offer".to_vec(),
            })),
        },
        SignalingMessage {
            kind: Some(signaling_message::Kind::Answer(Answer {
                sdp_blob: b"answer".to_vec(),
            })),
        },
        SignalingMessage {
            kind: Some(signaling_message::Kind::Candidate(Candidate {
                ufrag: "ufrag".to_owned(),
                blob: b"candidate".to_vec(),
            })),
        },
        SignalingMessage {
            kind: Some(signaling_message::Kind::RoomCode(RoomCode {
                code: 123_456,
            })),
        },
    ]
}

fn append_record(fixture: &mut Vec<u8>, tag: u8, bytes: &[u8]) {
    fixture.push(tag);
    fixture.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    fixture.extend_from_slice(bytes);
}

fn control_variant_bit(message: &ControlMessage) -> u16 {
    match &message.kind {
        Some(control_message::Kind::Hello(_)) => 1 << 0,
        Some(control_message::Kind::Bitrate(_)) => 1 << 1,
        Some(control_message::Kind::Nack(_)) => 1 << 2,
        Some(control_message::Kind::RefFrameInvalidate(_)) => 1 << 3,
        Some(control_message::Kind::InputEvent(_)) => 1 << 4,
        Some(control_message::Kind::Ping(_)) => 1 << 5,
        Some(control_message::Kind::Pong(_)) => 1 << 6,
        None => panic!("empty control message in fixture"),
    }
}

fn signaling_variant_bit(message: &SignalingMessage) -> u8 {
    match &message.kind {
        Some(signaling_message::Kind::Offer(_)) => 1 << 0,
        Some(signaling_message::Kind::Answer(_)) => 1 << 1,
        Some(signaling_message::Kind::Candidate(_)) => 1 << 2,
        Some(signaling_message::Kind::RoomCode(_)) => 1 << 3,
        None => panic!("empty signaling message in fixture"),
    }
}

struct FixtureCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> FixtureCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> &'a [u8] {
        let end = self.offset + len;

        if end > self.bytes.len() {
            panic!("fixture ended early");
        }

        let bytes = &self.bytes[self.offset..end];
        self.offset = end;

        bytes
    }

    fn read_u8(&mut self) -> u8 {
        self.take(1)[0]
    }

    fn read_u32_le(&mut self) -> u32 {
        let bytes = self.take(4);
        let mut value = [0_u8; 4];
        value.copy_from_slice(bytes);

        u32::from_le_bytes(value)
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

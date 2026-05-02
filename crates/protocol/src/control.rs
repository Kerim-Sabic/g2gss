use prost::Message;

pub use crate::generated::control::{
    Bitrate, ControlMessage, Hello, InputEvent, Nack, Ping, PlaceholderInput, Pong,
    RefFrameInvalidate, SeqRange, control_message, input_event,
};
pub use crate::generated::signaling::{
    Answer, Candidate, Offer, RoomCode, SignalingMessage, signaling_message,
};

/// Encodes a control message using prost.
pub fn encode_control(
    message: &ControlMessage,
    dst: &mut Vec<u8>,
) -> Result<(), prost::EncodeError> {
    message.encode(dst)
}

/// Decodes a complete control message using prost.
pub fn decode_control(src: &[u8]) -> Result<ControlMessage, prost::DecodeError> {
    ControlMessage::decode(src)
}

/// Encodes a signaling message using prost.
pub fn encode_signaling(
    message: &SignalingMessage,
    dst: &mut Vec<u8>,
) -> Result<(), prost::EncodeError> {
    message.encode(dst)
}

/// Decodes a complete signaling message using prost.
pub fn decode_signaling(src: &[u8]) -> Result<SignalingMessage, prost::DecodeError> {
    SignalingMessage::decode(src)
}

#[cfg(test)]
pub mod tests {
    use super::{
        Answer, Bitrate, Candidate, ControlMessage, Hello, InputEvent, Nack, Offer,
        PlaceholderInput, Pong, RefFrameInvalidate, RoomCode, SeqRange, SignalingMessage,
        control_message, decode_control, decode_signaling, encode_control, encode_signaling,
        input_event, signaling_message,
    };

    #[test]
    fn roundtrip() {
        let messages = [
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
                kind: Some(control_message::Kind::Ping(super::Ping {
                    tai64n: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
                })),
            },
            ControlMessage {
                kind: Some(control_message::Kind::Pong(Pong {
                    tai64n: vec![11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
                })),
            },
        ];

        for message in messages {
            assert_roundtrips_control(message);
        }

        let signaling_messages = [
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
        ];

        for message in signaling_messages {
            assert_roundtrips_signaling(message);
        }
    }

    fn assert_roundtrips_control(message: ControlMessage) {
        let mut encoded = Vec::new();
        if let Err(error) = encode_control(&message, &mut encoded) {
            panic!("{error}");
        }

        let decoded = match decode_control(&encoded) {
            Ok(decoded) => decoded,
            Err(error) => panic!("{error}"),
        };

        assert_eq!(decoded, message);
    }

    fn assert_roundtrips_signaling(message: SignalingMessage) {
        let mut encoded = Vec::new();
        if let Err(error) = encode_signaling(&message, &mut encoded) {
            panic!("{error}");
        }

        let decoded = match decode_signaling(&encoded) {
            Ok(decoded) => decoded,
            Err(error) => panic!("{error}"),
        };

        assert_eq!(decoded, message);
    }
}

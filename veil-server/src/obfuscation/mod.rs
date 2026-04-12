use bytes::{Bytes, BytesMut, BufMut};
use rand::Rng;
use veil_core::protocol::frame::{Frame, FrameType, ChannelId};

/// Common HTTPS response sizes to normalize packet sizes to
const HTTPS_COMMON_SIZES: &[usize] = &[576, 1024, 1280, 1460, 2048, 4096, 8192];

/// Obfuscation pipeline: apply padding, size normalization, burst shaping
pub struct ObfuscationLayer {
    padding_enabled: bool,
    size_normalization: bool,
}

impl ObfuscationLayer {
    pub fn new(padding_enabled: bool, size_normalization: bool) -> Self {
        Self { padding_enabled, size_normalization }
    }

    /// Wrap an outgoing frame with optional padding
    pub fn wrap_outgoing(&self, frame: Frame) -> Bytes {
        let mut encoded = frame.encode();

        if self.size_normalization {
            encoded = self.normalize_size(encoded);
        } else if self.padding_enabled {
            encoded = self.add_random_padding(encoded);
        }

        encoded
    }

    /// Normalize frame size to common HTTPS packet sizes
    fn normalize_size(&self, data: Bytes) -> Bytes {
        let target = HTTPS_COMMON_SIZES
            .iter()
            .find(|&&s| s >= data.len())
            .copied()
            .unwrap_or(data.len());

        if target == data.len() {
            return data;
        }

        let pad_size = target - data.len();
        let pad_frame = Frame::padding(pad_size).encode();

        let mut buf = BytesMut::with_capacity(data.len() + pad_frame.len());
        buf.put_slice(&data);
        buf.put_slice(&pad_frame);
        buf.freeze()
    }

    /// Add a small random padding frame after real data
    fn add_random_padding(&self, data: Bytes) -> Bytes {
        let mut rng = rand::thread_rng();
        let pad_size = rng.gen_range(0..=64);
        if pad_size == 0 {
            return data;
        }

        let pad_frame = Frame::padding(pad_size).encode();
        let mut buf = BytesMut::with_capacity(data.len() + pad_frame.len());
        buf.put_slice(&data);
        buf.put_slice(&pad_frame);
        buf.freeze()
    }

    /// Generate idle noise frame (sent when session is quiet)
    pub fn generate_noise() -> Bytes {
        let mut rng = rand::thread_rng();
        let size = rng.gen_range(32..128);
        Frame::padding(size).encode()
    }
}

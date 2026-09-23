//! Canonical binary encoding: the primitive rules of `docs/m0-design.md` §4.5.
//!
//! Integers are fixed-width little-endian. Byte strings and strings carry a `u32` length;
//! sequences carry a `u32` element count. Nothing depends on the host or a serializer.

/// Appends canonical encodings to a byte buffer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    /// Creates an empty encoder.
    pub fn new() -> Encoder {
        Encoder::default()
    }

    /// The bytes written so far.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Consumes the encoder, returning its bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Writes raw bytes with no length prefix. For fixed-size fields such as magics.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Writes a `u8`.
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Writes a `u16`, little-endian.
    pub fn u16(&mut self, v: u16) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes a `u32`, little-endian.
    pub fn u32(&mut self, v: u32) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes a `u64`, little-endian.
    pub fn u64(&mut self, v: u64) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes a `u128`, little-endian.
    pub fn u128(&mut self, v: u128) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes an `i64`, little-endian two's complement.
    pub fn i64(&mut self, v: i64) {
        self.raw(&v.to_le_bytes());
    }

    /// Writes a `bool` as `0` or `1`.
    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }

    /// Writes a `u32` length, then the bytes.
    ///
    /// # Panics
    ///
    /// If `bytes` is 4 GiB or longer, which no canonical value may be.
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.raw(bytes);
    }

    /// Writes a string as its UTF-8 bytes, length-prefixed.
    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    /// Writes a sequence's `u32` element count. The caller then writes each element.
    ///
    /// # Panics
    ///
    /// If `n` does not fit in a `u32`.
    pub fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("canonical lengths are below 2^32"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_are_fixed_width_little_endian() {
        let mut e = Encoder::new();
        e.u8(0xAB);
        e.u16(0x0102);
        e.u32(0x0102_0304);
        e.u64(0x0102_0304_0506_0708);
        e.i64(-2);
        e.bool(true);
        e.u128(1);
        #[rustfmt::skip]
        let expected: &[u8] = &[
            0xAB,
            0x02, 0x01,
            0x04, 0x03, 0x02, 0x01,
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01,
            0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0x01,
            0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(e.as_bytes(), expected);
    }

    #[test]
    fn strings_and_bytes_are_length_prefixed() {
        let mut e = Encoder::new();
        e.str("h\u{e9}");
        e.bytes(&[]);
        e.len(3);
        assert_eq!(
            e.into_bytes(),
            [3, 0, 0, 0, b'h', 0xC3, 0xA9, 0, 0, 0, 0, 3, 0, 0, 0]
        );
    }
}

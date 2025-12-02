//! TCP Protocol Definitions
//!
//! Pure Rust implementation of TCP protocol structures and constants.

/// TCP header length (excluding options)
pub const TCP_HLEN: usize = 20;

/// TCP header flags
pub const TCP_FIN: u8 = 0x01;
pub const TCP_SYN: u8 = 0x02;
pub const TCP_RST: u8 = 0x04;
pub const TCP_PSH: u8 = 0x08;
pub const TCP_ACK: u8 = 0x10;
pub const TCP_URG: u8 = 0x20;
pub const TCP_ECE: u8 = 0x40;
pub const TCP_CWR: u8 = 0x80;
pub const TCP_FLAGS: u8 = 0x3F;

/// Maximum TCP option bytes
pub const TCP_MAX_OPTION_BYTES: usize = 40;

/// TCP Header Structure
///
/// Fields are in network byte order (big-endian).
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct TcpHdr {
    /// Source port
    pub src: u16,

    /// Destination port
    pub dest: u16,

    /// Sequence number
    pub seqno: u32,

    /// Acknowledgment number
    pub ackno: u32,

    /// Header length (4 bits), reserved (4 bits), and flags (8 bits)
    /// Upper 4 bits: data offset (header length in 32-bit words)
    /// Lower 12 bits: reserved + flags
    pub _hdrlen_rsvd_flags: u16,

    /// Window size
    pub wnd: u16,

    /// Checksum
    pub chksum: u16,

    /// Urgent pointer
    pub urgp: u16,
}

impl TcpHdr {
    /// Get header length in 32-bit words
    ///
    /// Equivalent to C macro: TCPH_HDRLEN(phdr)
    #[inline]
    pub fn hdrlen(&self) -> u16 {
        u16::from_be(self._hdrlen_rsvd_flags) >> 12
    }

    /// Get header length in bytes
    ///
    /// Equivalent to C macro: TCPH_HDRLEN_BYTES(phdr)
    #[inline]
    pub fn hdrlen_bytes(&self) -> u8 {
        (self.hdrlen() << 2) as u8
    }

    /// Get TCP flags
    ///
    /// Equivalent to C macro: TCPH_FLAGS(phdr)
    #[inline]
    pub fn flags(&self) -> u8 {
        (u16::from_be(self._hdrlen_rsvd_flags) & TCP_FLAGS as u16) as u8
    }

    /// Set header length (in 32-bit words)
    ///
    /// Equivalent to C macro: TCPH_HDRLEN_SET(phdr, len)
    #[inline]
    pub fn set_hdrlen(&mut self, len: u16) {
        let current_flags = self.flags();
        self._hdrlen_rsvd_flags = u16::to_be((len << 12) | current_flags as u16);
    }

    /// Set TCP flags
    ///
    /// Equivalent to C macro: TCPH_FLAGS_SET(phdr, flags)
    #[inline]
    pub fn set_flags(&mut self, flags: u8) {
        let hdrlen = self.hdrlen();
        self._hdrlen_rsvd_flags = u16::to_be((hdrlen << 12) | flags as u16);
    }

    /// Set header length and flags together
    ///
    /// Equivalent to C macro: TCPH_HDRLEN_FLAGS_SET(phdr, len, flags)
    #[inline]
    pub fn set_hdrlen_flags(&mut self, len: u16, flags: u8) {
        self._hdrlen_rsvd_flags = u16::to_be((len << 12) | flags as u16);
    }

    /// Set a TCP flag bit
    ///
    /// Equivalent to C macro: TCPH_SET_FLAG(phdr, flags)
    #[inline]
    pub fn set_flag(&mut self, flag: u8) {
        let current = u16::from_be(self._hdrlen_rsvd_flags);
        self._hdrlen_rsvd_flags = u16::to_be(current | flag as u16);
    }

    /// Unset a TCP flag bit
    ///
    /// Equivalent to C macro: TCPH_UNSET_FLAG(phdr, flags)
    #[inline]
    pub fn unset_flag(&mut self, flag: u8) {
        let current = u16::from_be(self._hdrlen_rsvd_flags);
        self._hdrlen_rsvd_flags = u16::to_be(current & !(flag as u16));
    }

    /// Get source port (converted to host byte order)
    #[inline]
    pub fn src_port(&self) -> u16 {
        u16::from_be(self.src)
    }

    /// Get destination port (converted to host byte order)
    #[inline]
    pub fn dest_port(&self) -> u16 {
        u16::from_be(self.dest)
    }

    /// Get sequence number (converted to host byte order)
    #[inline]
    pub fn sequence_number(&self) -> u32 {
        u32::from_be(self.seqno)
    }

    /// Get acknowledgment number (converted to host byte order)
    #[inline]
    pub fn ack_number(&self) -> u32 {
        u32::from_be(self.ackno)
    }

    /// Get window size (converted to host byte order)
    #[inline]
    pub fn window(&self) -> u16 {
        u16::from_be(self.wnd)
    }

    /// Get checksum (converted to host byte order)
    #[inline]
    pub fn checksum(&self) -> u16 {
        u16::from_be(self.chksum)
    }

    /// Get urgent pointer (converted to host byte order)
    #[inline]
    pub fn urgent_pointer(&self) -> u16 {
        u16::from_be(self.urgp)
    }
}

// Ensure the struct is exactly 20 bytes
const _: () = assert!(core::mem::size_of::<TcpHdr>() == TCP_HLEN);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcp_header_size() {
        assert_eq!(core::mem::size_of::<TcpHdr>(), 20);
    }

    #[test]
    fn test_tcp_flags() {
        let mut hdr = TcpHdr {
            src: 0,
            dest: 0,
            seqno: 0,
            ackno: 0,
            _hdrlen_rsvd_flags: 0,
            wnd: 0,
            chksum: 0,
            urgp: 0,
        };

        // Set SYN flag
        hdr.set_hdrlen_flags(5, TCP_SYN);
        assert_eq!(hdr.flags(), TCP_SYN);
        assert_eq!(hdr.hdrlen(), 5);

        // Set SYN+ACK
        hdr.set_flags(TCP_SYN | TCP_ACK);
        assert_eq!(hdr.flags(), TCP_SYN | TCP_ACK);
    }

    #[test]
    fn test_byte_order_conversion() {
        let mut hdr = TcpHdr {
            src: u16::to_be(80),
            dest: u16::to_be(12345),
            seqno: u32::to_be(1000),
            ackno: u32::to_be(2000),
            _hdrlen_rsvd_flags: 0,
            wnd: u16::to_be(8192),
            chksum: 0,
            urgp: 0,
        };

        hdr.set_hdrlen_flags(5, TCP_SYN | TCP_ACK);

        assert_eq!(hdr.src_port(), 80);
        assert_eq!(hdr.dest_port(), 12345);
        assert_eq!(hdr.sequence_number(), 1000);
        assert_eq!(hdr.ack_number(), 2000);
        assert_eq!(hdr.window(), 8192);
        assert_eq!(hdr.flags(), TCP_SYN | TCP_ACK);
        assert_eq!(hdr.hdrlen_bytes(), 20);
    }
}

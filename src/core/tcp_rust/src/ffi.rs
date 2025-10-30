//! FFI (Foreign Function Interface) bindings to lwIP C code
//!
//! This module contains:
//! - Auto-generated bindings from bindgen (C types and functions)
//! - Rust types that match C layouts (#[repr(C)])
//! - Error code definitions

// Include auto-generated bindings from build.rs
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Error type matching C err_t from lwIP
///
/// This must exactly match the values in lwip/err.h
#[repr(i8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ErrT {
    Ok = 0,
    Mem = -1,       // Out of memory
    Buf = -2,       // Buffer error
    Timeout = -3,   // Timeout
    Rte = -4,       // Routing problem
    Inprogress = -5, // Operation in progress
    Val = -6,       // Illegal value
    Wouldblock = -7, // Operation would block
    Use = -8,       // Address in use
    Already = -9,   // Already connecting
    Isconn = -10,   // Connection already established
    Conn = -11,     // Not connected
    If = -12,       // Low-level netif error
    Abrt = -13,     // Connection aborted
    Rst = -14,      // Connection reset
    Clsd = -15,     // Connection closed
    Arg = -16,      // Illegal argument
}

impl ErrT {
    /// Convert to C err_t (i8)
    pub fn to_c(self) -> i8 {
        self as i8
    }

    /// Convert from C err_t (i8)
    pub fn from_c(val: i8) -> Self {
        match val {
            0 => ErrT::Ok,
            -1 => ErrT::Mem,
            -2 => ErrT::Buf,
            -3 => ErrT::Timeout,
            -4 => ErrT::Rte,
            -5 => ErrT::Inprogress,
            -6 => ErrT::Val,
            -7 => ErrT::Wouldblock,
            -8 => ErrT::Use,
            -9 => ErrT::Already,
            -10 => ErrT::Isconn,
            -11 => ErrT::Conn,
            -12 => ErrT::If,
            -13 => ErrT::Abrt,
            -14 => ErrT::Rst,
            -15 => ErrT::Clsd,
            -16 => ErrT::Arg,
            _ => ErrT::Arg, // Unknown error, treat as argument error
        }
    }

    /// Check if this is a success result
    pub fn is_ok(self) -> bool {
        matches!(self, ErrT::Ok)
    }

    /// Check if this is an error result
    pub fn is_err(self) -> bool {
        !self.is_ok()
    }
}

/// TCP timer interval in milliseconds (250ms)
pub const TCP_TMR_INTERVAL: u32 = 250;

/// IP address type constants (may also come from bindgen)
pub const IPADDR_TYPE_V4: u8 = 0;
pub const IPADDR_TYPE_V6: u8 = 6;
pub const IPADDR_TYPE_ANY: u8 = 46;

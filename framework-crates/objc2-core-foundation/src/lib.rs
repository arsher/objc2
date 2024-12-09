//! # Bindings to the `CoreFoundation` framework
//!
//! See [Apple's docs][apple-doc] and [the general docs on framework crates][framework-crates] for more information.
//!
//! [apple-doc]: https://developer.apple.com/documentation/corefoundation/
//! [framework-crates]: https://docs.rs/objc2/latest/objc2/topics/about_generated/index.html
#![no_std]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
// Update in Cargo.toml as well.
#![doc(html_root_url = "https://docs.rs/objc2-core-foundation/0.2.2")]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod generated;
#[cfg(feature = "CFCGTypes")]
mod geometry;

#[allow(unused_imports, unreachable_pub)]
pub use self::generated::*;
#[cfg(feature = "CFCGTypes")]
pub use self::geometry::*;

// MacTypes.h
#[allow(dead_code)]
mod mac_types {
    pub(crate) type Boolean = u8; // unsigned char
    pub(crate) type ConstStr255Param = *const core::ffi::c_char;
    pub(crate) type ConstStringPtr = *const core::ffi::c_char;
    pub(crate) type FourCharCode = u32;
    pub(crate) type LangCode = i16;
    pub(crate) type OSType = FourCharCode;
    pub(crate) type RegionCode = i16;
    pub(crate) type ResType = FourCharCode;
    pub(crate) type StringPtr = *mut core::ffi::c_char;
    pub(crate) type UniChar = u16;
    pub(crate) type UTF32Char = u32; // Or maybe Rust's char?
}

#[allow(unused_imports)]
pub(crate) use self::mac_types::*;
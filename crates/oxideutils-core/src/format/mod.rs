//! Object file format layer (ELF always; PE/Mach-O with `std`).
//!
//! Kernel (`no_std` + `alloc`): use [`object`] + [`elf`] on byte slices.

pub mod archive;
pub mod elf;
pub mod object;
pub mod traits;
pub mod utils;
pub mod wasm;

#[cfg(feature = "std")]
pub mod macho;
#[cfg(feature = "std")]
pub mod pe;

pub use object::OxideObject;
pub use traits::{ObjectView, SectionFlags, SectionKindView, SectionView};

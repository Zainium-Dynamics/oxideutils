//! # oxideutils-core
//!
//! Shared library for **OxideUtils** — a **Zainium Dynamics** product.
//!
//! ## Dual mode: `std` + `no_std`
//!
//! | Mode | Feature flags | Use case |
//! |------|---------------|----------|
//! | **std** (default) | `std`, tools, file I/O, CLI | Host / userland (`oxide-objdump`, …) |
//! | **no_std + alloc** | `--no-default-features --features alloc,disasm` | **Zainium kernel** / freestanding |
//!
//! Kernel profile example:
//! ```text
//! oxideutils-core = { path = "...", default-features = false, features = ["alloc", "disasm", "kernel"] }
//! ```
//!
//! **Not a GNU project.**

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::module_inception)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(all(not(feature = "alloc"), not(feature = "std")))]
compile_error!("oxideutils-core requires feature `alloc` or `std` (std implies alloc)");

#[cfg(feature = "alloc")]
pub mod prelude;

pub mod error;
pub mod format;
pub mod symbols;
pub mod utils;

#[cfg(feature = "alloc")]
pub mod archive;

#[cfg(feature = "disasm")]
pub mod disasm;

// ---- std-only subsystems (host tools, kernel leaves these out) ----
#[cfg(feature = "std")]
pub mod addr2line_util;
/// GNU `ar` create/modify/ranlib (`std` only).
#[cfg(feature = "std")]
pub mod archive_write;
/// Host configuration & CLI helpers (`std` only). TOML: see `cli::config::OxideToml`.
#[cfg(feature = "std")]
pub mod cli;
#[cfg(feature = "std")]
pub mod objcopy;
#[cfg(feature = "std")]
pub mod strip;

#[cfg(feature = "alloc")]
pub use archive::{OxideArchive, is_archive};
pub use error::{OxideError, Result};
pub use format::object::OxideObject;
pub use format::{ObjectView, SectionView};

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// `true` when built with the `std` feature (userland).
pub const HAS_STD: bool = cfg!(feature = "std");

/// `true` when disassembly backend is linked.
pub const HAS_DISASM: bool = cfg!(feature = "disasm");

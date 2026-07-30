//! oxide-ar binary entry — like binutils `not-ranlib.c` / `maybe-ranlib.c`.
//!
//! When argv0 ends in `ranlib` (hardlink/symlink install), dispatches to
//! ranlib_main the same way ar.c does when `is_ranlib == -1`.

fn main() -> std::process::ExitCode {
    oxide_ar::maybe_main()
}

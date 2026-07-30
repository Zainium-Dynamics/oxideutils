//! oxide-ranlib binary entry — like binutils `is-ranlib.c`
//! (`#define is_ranlib 1` + include ar.c). Always runs ranlib_main.

fn main() -> std::process::ExitCode {
    oxide_ar::ranlib_main()
}

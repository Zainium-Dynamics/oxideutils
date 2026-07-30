#[path = "../../build/oxide_build.rs"]
mod oxide_build;

fn main() {
    oxide_build::for_package("ar");
}

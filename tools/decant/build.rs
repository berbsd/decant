//! Bake the compile-time target triple into the binary so `decant update`
//! knows which release asset to fetch.

fn main() {
  let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());
  println!("cargo:rustc-env=DECANT_TARGET={target}");
}

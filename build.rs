use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=FENCE_ROOT_HINT");

    let hint = env::var("FENCE_ROOT_HINT").ok();

    if let Some(raw_hint) = hint {
        let candidate = PathBuf::from(raw_hint);
        let canonical = candidate.canonicalize().unwrap_or(candidate);

        println!("cargo:rustc-env=FENCE_ROOT_HINT={}", canonical.display());
    }
}

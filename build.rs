use std::env;
use std::path::PathBuf;

// Embed a repo-root hint into the compiled helpers when tools/sync_bin_helpers.sh
// sets FENCE_ROOT_HINT, giving binaries a fallback path to locate schemas/docs
// even if launched from outside the repo. Re-run when that hint changes.
fn main() {
    println!("cargo:rerun-if-env-changed=FENCE_ROOT_HINT");

    let hint = env::var("FENCE_ROOT_HINT").ok();

    if let Some(raw_hint) = hint {
        let candidate = PathBuf::from(raw_hint);
        let canonical = candidate.canonicalize().unwrap_or(candidate);

        println!("cargo:rustc-env=FENCE_ROOT_HINT={}", canonical.display());
    }
}

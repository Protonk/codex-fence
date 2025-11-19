# Helper authoring guide

When you add a helper under `lib/`, follow these rules:

- Every script must start with the standard banner used in the existing helpers:
  keep helper utilities portable, pure, and free of global state. Copy those
  lines verbatim when creating a new file so the expectations stay visible.
- Maintain a 1:1 mapping between helper functions and files. A new helper
  requires a new `.sh` file whose basename matches the function it contains.
  Avoid dumping unrelated helpers into a shared file.

Probes should source only the helpers they need to stay lightweight.

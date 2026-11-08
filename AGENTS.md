# AGENTS.md

fynix is a reactive UI framework.

- `crates/fynix` is the core. It is `no_std` with `alloc`: `std`
  only behind the `std` feature and in tests.
- `crates/bevy_fynix` is the Bevy backend and its views.
- `crates/fynix_macros` holds the macros.
- `crates/bevy_fynix/examples/` has `gallery` and `dock`. Run one with
  `cargo run -p bevy_fynix --example gallery`.

## Branches and pull requests

- Prefix a branch with the name of the person it is for, as in
  `nixon/virtual-list`.
- `main` is linear. Pull requests are squash merged, and a branch
  catches up with `main` by rebase, never by a merge commit.
- A pull request's description says only what changed. No section
  for the checks that passed, the tests added, what was left out or
  what comes next.

## Checks

All must be clean before a commit. CI runs the same.

```sh
cargo +nightly fmt --all
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo check --workspace --no-default-features
cargo test --workspace --all-features --all-targets
cargo test --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --document-private-items
```

Formatting needs a nightly: `rustfmt.toml` uses unstable options,
which stable rustfmt skips without a word.

## Code

- The [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
  are the base.
- A boolean method reads as a question: `is_`, `has_`, `can_`.
- Turbofish on the call, not a type annotation on the binding.
- Moved code is imported from where it now lives. No `pub use` kept
  to spare the old paths.

## Tests

Test only what can break.

- Keep a test where a mistake is easy to make and hard to see in
  review: maths with a sign or an axis to get wrong, layout and
  scroll edge cases, a behaviour a user asked for by name.
- Skip a test that restates the line it covers, or that checks
  another crate, such as Bevy.
- When it's a toss-up whether a test earns its place, leave it out.

## Comments

As few as possible.

- No comment where the code explains itself. Public items get a doc
  comment that says what they are, briefly.
- Describe the current code, never the change that produced it.
- Behaviour of one call site is a `//` comment at that line, not in
  the doc comment.
- Plain ASCII: no em dashes or smart quotes. Full sentences ending
  with a full stop.
- Write a comment on one line and let nightly rustfmt wrap it.

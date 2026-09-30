# Fuzzing the rqlang syntax layer

Not a workspace member, so `cargo test` at the repository root skips it. The same
invariants run deterministically on every repository fixture in
`src/rq-lib/tests/syntax_invariants.rs`, which `cargo test` does run — fuzzing is
what looks for the inputs nobody wrote a fixture for.

## Targets

| Target | Asserts |
|---|---|
| `tokenize` | token spans are contiguous, land on char boundaries, match their value, and cover the whole input; a tokenizer error carries an in-bounds, one-based position |
| `parse` | `analyze` never panics and reports in-bounds error spans; `RqFile::from_content_lenient` never panics on any input |

## How it normally runs

`.github/workflows/fuzz_nightly.yaml` runs both targets every night for ten minutes
each. It seeds the corpus from the `.rq` fixtures under `src/cli/tests` and `tests/uat`,
carries the grown corpus between runs through the Actions cache, and on a crash uploads
the offending input as an artifact and opens (or comments on) a `Fuzzing: <target>`
issue with the bytes base64 encoded.

Run it on demand from the Actions tab: it takes a `max_total_time` input, so
`max_total_time=60` is a quick smoke test of the workflow itself.

There is no reason to run it by hand as part of everyday work. Reach for the local
setup below when you are chasing a crash the nightly reported, or when you have just
rewritten something in `syntax/` and do not want to wait until tomorrow.

## Running it locally

```bash
rustup toolchain install nightly
cargo install cargo-fuzz
```

Seed the corpus the way the workflow does:

```bash
mkdir -p fuzz/corpus/tokenize
while IFS= read -r fixture; do
  cp "$fixture" "fuzz/corpus/tokenize/seed_$(printf '%s' "$fixture" | shasum | cut -c1-16).rq"
done < <(find src/cli/tests tests/uat -name '*.rq')
cp -r fuzz/corpus/tokenize fuzz/corpus/parse
```

Then, from the repository root:

```bash
cargo +nightly fuzz run tokenize -- -max_total_time=300
cargo +nightly fuzz run parse -- -max_total_time=300
```

`cargo install` builds cargo-fuzz for the host, so the default target is right. A
prebuilt cargo-fuzz (`cargo binstall`, `taiki-e/install-action`) may be musl-linked and
then defaults to a musl target, which fails with `sanitizer is incompatible with
statically linked libc`. Pass `--target "$(rustc -vV | sed -n 's/^host: //p')"` in that
case, the way the workflow does.

## After a crash

`cargo fuzz` writes the offending input to `fuzz/artifacts/<target>/`. Reproduce it with

```bash
cargo +nightly fuzz run <target> fuzz/artifacts/<target>/<file>
```

then add that input to `ADVERSARIAL_SOURCES` in `src/rq-lib/tests/syntax_invariants.rs`,
or as a `check` fixture when it is a diagnostic problem rather than a panic, so the
regression is covered by `cargo test` from then on.

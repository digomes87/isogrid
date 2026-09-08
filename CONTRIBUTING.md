# Contributing to isogrid

Thanks for taking a look. This is a hobby project built in the open; small,
well-tested pull requests are the easiest kind to merge.

## Ground rules

- **English everywhere** — code, comments, identifiers, commits, docs, issues.
- **`main` is always green.** Work on a short-lived branch (`feat/...`,
  `fix/...`, `docs/...`) and open a pull request.
- **Nothing merges without a test, a doc and a green CI run.**
- **The engine does not know about the game.** If a change mentions rides,
  guests or money, it belongs in [openpark](https://github.com/digomes87/openpark).

## Getting set up

```sh
rustup component add rustfmt clippy
cargo install just cargo-llvm-cov cargo-deny lefthook
lefthook install   # runs fmt + clippy before each commit
```

## The loop

```sh
just check   # fmt, clippy, tests and docs — what CI runs
just test    # tests only
just cov     # coverage report
```

## Commits

[Conventional Commits](https://www.conventionalcommits.org/), and one logical
change per commit:

```
feat(camera): add zoom clamped to a configurable range

fix(iso): round half-tiles toward negative infinity in screen_to_grid

docs(adr): record why macroquad was chosen over wgpu
```

Types in use: `feat`, `fix`, `docs`, `refactor`, `test`, `perf`, `chore`, `ci`,
`build`. A breaking change gets a `!` (`feat!:`) and a `BREAKING CHANGE:` footer;
releases are cut from these by `release-plz`.

## Tests

- Pure maths gets a property test (`proptest`) alongside its unit tests.
- Anything that touches simulation order gets a determinism test: same seed,
  same state after N ticks.
- Every public item carries a rustdoc example, and examples are compiled and run
  as doc tests.

## Architecture decisions

Choices that are expensive to reverse get a short record in
[`docs/adr/`](docs/adr/). Copy the newest one, bump the number, keep it under a
page.

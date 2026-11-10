<!-- 0.1.5: data of the tests, not examples. -->
# Fixtures

- `v0.1.4-blocks/`: the `.ag` and `metagente.toml` code blocks of `README.md` and `docs/` exactly as they were at tag `v0.1.4`.
- `v0.1.4-golden/`: what Metagente 0.1.4 printed for them, for `examples/` and for the samples (`metagente check`, with the
  version number replaced by `<version>`).

Both were made once, before any change of 0.1.5, by `scripts/freeze-v0.1.4.sh` with a binary built from the 0.1.4 tree.
`tests/integration/examples_unchanged.rs` proves that every later release still gives the same result. Do not edit them
and do not run the script again.

# material-color-utilities (C++ subset)

From https://github.com/material-foundation/material-color-utilities at
`5b3618b16fdc3825e21d5679bafd144662088ea1`, Apache-2.0 (see `LICENSE`).

Swiftify uses it to build Material 3 schemes from a seed colour: the built-in
fallback scheme and the artwork-tinted player scheme.

Changes from upstream:

- Only `cpp/` is kept, without the quantizers (`celebi`, `wsmeans`, `wu`) and
  `score`. `quantize/lab` stays because the temperature cache uses it.
- `utils.cc` formats `HexFromArgb` with `snprintf` instead of Abseil, so the
  subset has no dependencies beyond the C++ standard library.

# Spellcheck dictionaries

Hunspell dictionaries embedded into the binary (zlib-compressed by
`build.rs`, decompressed lazily at runtime). ~6.0 MB uncompressed,
~1.6 MB compressed.

- `en_US.aff` / `en_US.dic` — US English, derived from SCOWL via
  [wooorm/dictionaries](https://github.com/wooorm/dictionaries) (`en`).
  License: MIT AND BSD — see `LICENSE.en_US.txt`.
- `pt_BR.aff` / `pt_BR.dic` — Brazilian Portuguese (VerO, LibreOffice),
  via [wooorm/dictionaries](https://github.com/wooorm/dictionaries) (`pt`).
  License: LGPL-3.0 OR MPL-2.0 — distributed here under MPL-2.0; see
  `LICENSE.pt_BR.txt`.

Sources: `https://github.com/wooorm/dictionaries/tree/main/dictionaries/{en,pt}`
(raw `.dic`/`.aff` files committed verbatim, uncompressed).

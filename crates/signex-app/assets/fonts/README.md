# Bundled fonts

Compiled into the binary by `src/fonts/bundled.rs` and registered with iced at
startup.

## Iosevka (canvas text)

`Iosevka-Regular.ttf`, `Iosevka-Bold.ttf`, `Iosevka-Italic.ttf` and
`Iosevka-BoldItalic.ttf` are subsets of the matching files in
`PkgTTF-Iosevka-34.3.0.zip` from the Iosevka 34.3.0 release. All four keep the
same 1077 codepoints and drop the OpenType layout features, so they share one
set of advance widths. Iosevka is licensed under the SIL Open Font License 1.1;
see `OFL-Iosevka.md`.

To cut a face the same way, take the codepoint list from an existing subset and
run fontTools' `pyftsubset` with no layout features:

```sh
python -c "from fontTools.ttLib import TTFont; \
print(','.join(f'{c:X}' for c in sorted(TTFont('Iosevka-Regular.ttf').getBestCmap())))" > unicodes.txt
pyftsubset release/Iosevka-Italic.ttf --unicodes-file=unicodes.txt \
  --layout-features='' --output-file=Iosevka-Italic.ttf
```

Run against the release's `Iosevka-Regular.ttf`, this recipe reproduces the
bundled `Iosevka-Regular.ttf` byte for byte.

## Roboto (UI chrome)

`Roboto-Regular.ttf` and `Roboto-Bold.ttf`.

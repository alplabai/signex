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

The canvas loads these faces alongside the system fonts. On a machine that also
has Iosevka installed, the system "Iosevka" family can be picked instead of the
bundled faces, so italic canvas text may then use the installed Oblique rather
than the bundled Italic.

## Roboto (UI chrome)

`Roboto-Regular.ttf` and `Roboto-Bold.ttf` are the unmodified static Regular
and Bold faces of Roboto 3.015 (`Version 3.015` in the name table), from the
Roboto Classic project, <https://github.com/googlefonts/roboto-classic>. Both
are full faces (3391 glyphs, hinted, OpenType layout kept). Roboto is licensed
under the SIL Open Font License 1.1, as each file's name table states; see
`OFL-Roboto.md`.

## Licenses in release builds

Every font here ships with its license text: each `OFL-*.md` in this folder is
copied into every release package under `licenses/fonts`
(`/usr/share/doc/signex/fonts` in the Linux packages):
`installer/copy-font-licenses.sh` for the `.deb`, the AppImage, the macOS
bundle and the Linux tarball, `installer/windows/signex.iss` for the Windows
installer, and a `Copy-Item` step in `release.yml` for the Windows portable
ZIP. The release workflow checks the `.deb`, the tarball and the DMG for
them before publishing. A test in
`src/fonts/bundled.rs` fails when a `.ttf` here has no `OFL-<Family>.md`.

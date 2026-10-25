# Test fonts

`Iosevka-Regular.ttf` is a copy of the Iosevka face the app bundles, kept here so
`signex-gfx` unit tests can shape real glyphs without depending on `signex-app`
or on system fonts. Iosevka is licensed under the SIL Open Font License 1.1.

`Iosevka-Italic.ttf` is the same copy of the app's italic face, so the italic
mapping can be checked against a real italic Iosevka.

The pixel goldens in `../golden/pixels/` (`tests/pixel_goldens.rs`) shape text
with these two faces only, through a hermetic font database, so a system
Iosevka cannot change the output (#654). Regenerate the references with
`SIGNEX_UPDATE_GOLDENS=1`; see `../golden/pixels/README.md`.

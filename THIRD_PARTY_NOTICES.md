# Third-party notices

Signex's own licence is in [LICENSE](LICENSE) and [NOTICE](NOTICE). This file
carries the NOTICE text of third-party components whose licence requires it to
travel with Signex (Apache License, Version 2.0, section 4(d)). The full text
of the Apache License, Version 2.0 is in [LICENSE-APACHE](LICENSE-APACHE).

## solidcore 1.0.1

- Use: the exact B-rep kernel under `crates/signex-mcad`.
- Source: <https://crates.io/crates/solidcore>
- Licence: Apache-2.0

Its NOTICE file, verbatim:

```text
solidcore
Copyright (c) 2026 The solidcore contributors

This product is a Rust port (a derivative work) of the SMLib and SM_API
components of USD BRep, https://github.com/NVIDIA-Omniverse/usd-brep
(v1.0.1, commit 3fd5b95), licensed under the Apache License, Version 2.0.
See ATTRIBUTION.md for exactly which upstream material is used.

The upstream work carries the following notice:

  USD BRep is Copyright (c) 2020-2026, NVIDIA CORPORATION & AFFILIATES. All
  rights reserved. NVIDIA-authored portions of this product are licensed under
  the Apache License, Version 2.0, except where otherwise noted.

The upstream SMLib and SM_API source files carry the SPDX header:

  SPDX-FileCopyrightText: Copyright (c) 2026 NVIDIA CORPORATION & AFFILIATES. All rights reserved.
  SPDX-License-Identifier: Apache-2.0

Upstream third-party notices (THIRD_PARTY_NOTICES.md at the pinned commit) cover
OpenUSD, oneTBB, hwloc, MaterialX, OpenSubdiv, CPython, HACL*, pybind11, the
UsdSolid proposals, usd-validation-nvidia, the repo bootstrap scripts and optional
Python GUI tooling. None of those components is part of SMLib or SM_API, and none
is used, ported or redistributed by solidcore, so no further third-party notices
apply.

Modifications: the C++ sources were translated to Rust and restructured (arena
topology, typed ids, error values instead of status codes, explicit kernel
state instead of a global context). Each ported Rust file names the upstream
file(s) it derives from in its header.
```

## typst-assets 0.15.1 fonts

- Use: the fonts the Design Notebook compiles and exports its pages with,
  embedded in `signex-notebook` (and so in the app and `signex-cli`).
- Source: <https://crates.io/crates/typst-assets>
- Licence: the crate is Apache-2.0; its fonts are licensed on their own terms,
  each shipped in full in `licenses/fonts/` (from `crates/signex-notebook/fonts/`):
  - Libertinus Serif: SIL Open Font License 1.1,
    `OFL-LibertinusSerif.md`. Copyright © 2012-2024 The Libertinus Project
    Authors, with Reserved Font Name "Linux Libertine", "Biolinum", "STIX Fonts".
  - New Computer Modern (NewCM10, NewCMMath): GUST Font License 1.0,
    `GUST-NewComputerModern.md`.
  - DejaVu Sans Mono: the Bitstream Vera and Arev font licences,
    `BitstreamVera-DejaVuSansMono.md`. Copyright (c) 2003 by Bitstream, Inc.;
    Copyright (c) 2006 by Tavmjong Bah; DejaVu changes are in the public domain.

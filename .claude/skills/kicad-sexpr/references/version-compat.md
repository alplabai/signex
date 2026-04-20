# KiCad Version Compatibility Reference — 8 / 9 / 10

> Data sourced from: `pcb_io_kicad_sexpr.h`, `sch_io_kicad_sexpr_parser.cpp`,
> `dev-docs.kicad.org`, and KiCad 10.0.0 release notes (March 2026)

---

## File Header Format

The first line of every `.kicad_pcb`, `.kicad_sch`, `.kicad_sym`, `.kicad_mod` file:

```scheme
; ── KiCad 8 ──
(kicad_pcb (version 20240108) (generator "pcbnew") (generator_version "8.0")

; ── KiCad 9 ──
(kicad_pcb (version 20241229) (generator "pcbnew") (generator_version "9.0")

; ── KiCad 10 ──
(kicad_pcb (version 20250324) (generator "pcbnew") (generator_version "10.0")
```

### `generator_version` Token (KiCad 8+)

Added with KiCad 8. For third-party tools:

```scheme
(generator "my-rust-tool") (generator_version "1.0")
```

The `generator` value must NEVER be `"pcbnew"` or `"eeschema"` —
use your own tool name to avoid confusion with KiCad-generated bugs.

---

## PCB File Version History (SEXPR_BOARD_FILE_VERSION)

Source: `pcb_io_kicad_sexpr.h`

### KiCad 8.0 Series

| Version | Change |
|---------|--------|
| `20230620` | PCB Fields (as footprint properties) |
| `20230730` | Connectivity for graphic shapes |
| `20230825` | `fp_text_box` / `gr_text_box` explicit border flag |
| `20230906` | Multiple image type support |
| `20230913` | Custom pad spoke templates |
| `20231007` | Generative objects (`generator` token) |
| `20231014` | V8 file format normalization |
| `20231212` | Reference image locking/UUID; footprint boolean format |
| `20231231` | `id` to `uuid` for generators and groups |
| **`20240108`** | **KiCad 8.0 release** — teardrop parameters to explicit bools |

### KiCad 9.0 Series

| Version | Change |
|---------|--------|
| `20241228` | Teardrop curve points to bool |
| **`20241229`** | **KiCad 9.0 release** — User layers expanded to arbitrary count |

### KiCad 10.0 Series

| Version | Change |
|---------|--------|
| `20250210` | Textbox `knockout` flag for `gr_text_box` / `fp_text_box` |
| `20250222` | PCB shape hatching (`hatch` / `reverse_hatch` fill types) |
| `20250228` | IPC-4761 via protection (tenting, plugging, capping) |
| `20250302` | Zone hatching offset parameters |
| `20250309` | Component class dynamic assignment rules |
| **`20250324`** | **KiCad 10.0 release** — Jumper pad token |

---

## KiCad 8 to 9 to 10: Token Changes

### Critical Rules for Third-Party Tools

1. **`generator_version` is required** (KiCad 8+):
   ```scheme
   (generator "your-tool-name") (generator_version "1.0")
   ```

2. **Use `uuid`, not `id`** (KiCad 8+, from version `20231231`):
   ```scheme
   ; Correct (KiCad 8+)
   (group "group-name" (uuid xxxxxxxx-...) ...)
   ; Old (KiCad 7 and earlier)
   (group "group-name" (id xxxxxxxx-...) ...)
   ```

3. **Footprint boolean format** (KiCad 8+, from `20231212`):
   ```scheme
   ; Correct — bare token as flag
   (remove_unused_layers)
   ; Old format
   (remove_unused_layers yes)
   ```

### New Tokens Added to Footprint (KiCad 7-8)

```scheme
(footprint "Lib:Part"
  [(private_layers LAYER_LIST)]          ; KiCad 7+ — layers private to this footprint
  [(net_tie_pad_groups "P1,P2" "P3,P4")] ; KiCad 7+ — net-tie pad groups
)
```

**`private_layers` usage:**
```scheme
(private_layers "F.Fab" "B.Fab")
```

**`net_tie_pad_groups` usage:**
```scheme
; Each group is a quoted string with comma-separated pad names
(net_tie_pad_groups "1,2" "3,4")
```

### Pad Attribute Changes (KiCad 8)

```scheme
; KiCad 8+ pad attr
(attr smd [board_only] [exclude_from_pos_files] [exclude_from_bom] [dnp])
;                                                                    ^^^
;                                                              added in KiCad 8
```

The `dnp` (do not place) flag was also added to footprint-level `attr` in KiCad 8:
```scheme
(attr through_hole [dnp])
```

### Teardrop Changes (KiCad 8-9)

KiCad 8 (`20240108`) converted teardrop parameters to explicit bools:
```scheme
; KiCad 8+
(teardrop (best_length_ratio 0.5) (max_length 1.0)
          (best_width_ratio 0.5) (max_width 2.0)
          (curve_points 5) (filter_points yes))

; KiCad 9 (20241228): additional bool flags
(teardrop ... (prefer_zone_connections yes) (allow_two_segments yes))
```

---

## KiCad 10 New Tokens

### 1. Textbox Knockout (`20250210`)

`knockout` flag added to `fp_text_box` and `gr_text_box`:

```scheme
(gr_text_box [locked]
  "TEXT"
  (start X Y) (end X Y)
  (angle ANGLE)
  (layer LAYER)
  (uuid UUID)
  (effects ...)
  [(stroke ...)]
  [knockout]   ; KiCad 10 new — text rendered inverted against background color
)
```

### 2. Shape Hatching (`20250222`)

Hatching fill support for graphic shapes (`gr_rect`, `gr_circle`, `gr_poly`, `fp_rect`, etc.):

```scheme
; KiCad 10+ — fill types extended
(fill (type none|solid|hatch|reverse_hatch))

; Hatch with parameters:
(fill (type hatch) (hatch_distance 1.0) (hatch_angle 45))
```

### 3. IPC-4761 Via Protection (`20250228`)

```scheme
(via
  (at X Y) (size D) (drill D) (layers L1 L2)
  [(tenting front|back|both)]    ; KiCad 10 new
  [(plugging front|back|both)]   ; KiCad 10 new
  [(capping)]                    ; KiCad 10 new
  (net N)
  (tstamp UUID)
)
```

### 4. Jumper Pad (`20250324`)

```scheme
; Jumper type in footprint attr
(attr [smd|through_hole] [jumper] ...)

; Jumper pad groups — which pads can be jumpered together
(jumper_pad_groups "1" "2")
```

### 5. Barcode Token (KiCad 10)

```scheme
; PCB barcode
(gr_barcode
  (at X Y)
  (layer LAYER)
  (uuid UUID)
  (type qrcode|datamatrix|pdf417|ean13|ean8|upca|upce|code39|code128)
  (data "TEXT_OR_VARIABLE")
  [(size W H)]
  [(locked)]
)

; Footprint barcode
(fp_barcode
  (at X Y)
  (layer LAYER)
  (uuid UUID)
  (type qrcode|datamatrix|...)
  (data "TEXT")
  [(size W H)]
)
```

### 6. Point Token (KiCad 10)

Zero-dimensional object for snapping and position marking:

```scheme
(gr_point
  (at X Y)
  (layer LAYER)
  (uuid UUID)
  [(locked)]
)
```

### 7. Footprint Inner-Layer Objects (KiCad 10)

Before KiCad 10, footprint graphics could only be placed on F.* and B.* layers.
Now `In1.Cu` through `In30.Cu` and inner technical layers are supported:

```scheme
; KiCad 10+ — inner layers now supported
(fp_line (start -1 0) (end 1 0)
  (layer "In1.Cu")     ; was invalid before KiCad 10
  (stroke (width 0.1) (type solid))
  (uuid UUID))

(fp_rect (start -1 -1) (end 1 1)
  (layer "In2.Cu")     ; was invalid before KiCad 10
  (stroke (width 0.1) (type solid))
  (uuid UUID))
```

---

## Rust Generator Version Targeting

```rust
/// Target KiCad version for file generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KiCadVersion {
    V8,   // version 20240108
    V9,   // version 20241229
    V10,  // version 20250324  <- recommended target
}

impl KiCadVersion {
    pub fn file_version(&self) -> u32 {
        match self {
            KiCadVersion::V8  => 20240108,
            KiCadVersion::V9  => 20241229,
            KiCadVersion::V10 => 20250324,
        }
    }

    pub fn version_str(&self) -> &'static str {
        match self {
            KiCadVersion::V8  => "8.0",
            KiCadVersion::V9  => "9.0",
            KiCadVersion::V10 => "10.0",
        }
    }
}

/// Generate PCB file header node
pub fn pcb_header(tool: &str, tool_version: &str, kicad: KiCadVersion) -> Node {
    Node::List(vec![
        Node::Atom(Atom::Raw("kicad_pcb".into())),
        Node::List(vec![
            Node::Atom(Atom::Raw("version".into())),
            Node::Atom(Atom::Raw(kicad.file_version().to_string())),
        ]),
        Node::List(vec![
            Node::Atom(Atom::Raw("generator".into())),
            Node::Atom(Atom::Str(tool.into())),
        ]),
        Node::List(vec![
            Node::Atom(Atom::Raw("generator_version".into())),
            Node::Atom(Atom::Str(tool_version.into())),
        ]),
    ])
}

// Usage:
// let header = pcb_header("my-rust-pcb-gen", "1.0", KiCadVersion::V10);
```

---

## Backward Compatibility Summary

| Feature | KiCad 8 | KiCad 9 | KiCad 10 |
|---------|:-------:|:-------:|:--------:|
| `generator_version` token | Added | Yes | Yes |
| `private_layers` in footprint | Yes | Yes | Yes |
| `net_tie_pad_groups` | Yes | Yes | Yes |
| `uuid` not `id` for generators | Required | Yes | Yes |
| `dnp` attr flag | Added | Yes | Yes |
| Arbitrary user layer count | No (User.1-9 only) | Yes | Yes |
| Textbox `knockout` | No | No | Yes (20250210) |
| Shape `hatch` fill | No | No | Yes (20250222) |
| IPC-4761 via protection | No | No | Yes (20250228) |
| Jumper pad | No | No | Yes (20250324) |
| `barcode` token | No | No | Yes |
| `point` token | No | No | Yes |
| Footprint inner-layer objects | No | No | Yes |

---

## Minimum Valid PCB File (KiCad 10)

```scheme
(kicad_pcb (version 20250324) (generator "my-tool") (generator_version "1.0")
  (general
    (thickness 1.6)
  )
  (paper "A4")
  (layers
    (0  "F.Cu"      signal)
    (31 "B.Cu"      signal)
    (36 "B.SilkS"   user)
    (37 "F.SilkS"   user)
    (38 "B.Paste"   user)
    (39 "F.Paste"   user)
    (40 "B.Mask"    user)
    (41 "F.Mask"    user)
    (44 "Edge.Cuts" user)
    (45 "Margin"    user)
    (46 "B.CrtYd"   user)
    (47 "F.CrtYd"   user)
    (48 "B.Fab"     user)
    (49 "F.Fab"     user)
  )
  (setup
    (pad_to_mask_clearance 0)
  )
  (net 0 "")
)
```

---

## Schematic Version Notes

Schematic (`.kicad_sch`) versions are independent from PCB versions.
Schematic tokens added since KiCad 9:

| Feature | Added in |
|---------|---------|
| Design variants (`variant` token) | KiCad 10 |
| Hop-over wire crossings | KiCad 10 |
| Local power symbols (`local_power` flag) | KiCad 10 |
| Jumper connections (`jumper_definition` token) | KiCad 10 |
| Schematic grouping | KiCad 10 |
| Schematic Rule Areas | KiCad 9 |
| Design Blocks (schematic) | KiCad 9 |
| Table token | KiCad 9 |

### KiCad 10 Schematic: Variants

```scheme
(kicad_sch (version ...) (generator ...) ...
  [(variant_definitions
    (variant "DEFAULT")
    (variant "LITE")
    (variant "PRO")
  )]
)

; Variant override in symbol property
(symbol "Device:R"
  (property "Value" "10k"
    (variants
      (variant "DEFAULT" "10k")
      (variant "LITE" "4.7k")
    ))
)
```

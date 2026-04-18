# Schematic Symbol Render — TRANSFORM, Flatten, Pin

> Source: `eeschema/sch_symbol.cpp`, `sch_painter.cpp`, `sch_pin.cpp`, `lib_symbol.cpp`
> (KiCad source code, April 2026)

---

## Core architecture (from KiCad source code)

```
LIB_SYMBOL  ──── graphic definition (polyline, arc, circle, rectangle, pin)
     │            in lib coordinates, at origin'de
     │ flatten()
SCH_SYMBOL  ──── instance: at(x,y), m_transform, m_unit, m_bodyStyle
     │            m_libSymbol = flattened copy
     │
SCH_PIN     ──── instance pin, references LIB_PIN'e ,
                  position transformed via m_transform
```

**Critical:** `SCH_SYMBOL::m_libSymbol` a flattened `LIB_SYMBOL`.
The schematic file `lib_symbols` section this flattened copy stores.
`extends` for symbols inheriting via the keyword, `Flatten()` call is required.

---

## TRANSFORM matrix — KiCad's integer matrix

KiCad C++ kodu `TRANSFORM` class:

```cpp
struct TRANSFORM {
    int x1, x2;  // column 1
    int y1, y2;  // column 2
    // Application: x' = x1*x + x2*y,  y' = y1*x + y2*y
};
```

Values are only `{-1, 0, 1}` — isometric transformation.

### Standard transformation table

| Case | x1 | x2 | y1 | y2 | S-expr `at` angle |
|-------|----|----|----|----|-------------------|
| Normal (0°) | 1 | 0 | 0 | -1 | 0 |
| 90° CCW | 0 | 1 | 1 | 0 | 90 |
| 180° | -1 | 0 | 0 | 1 | 180 |
| 270° CCW | 0 | -1 | -1 | 0 | 270 |
| Mirror X | -1 | 0 | 0 | -1 | 0 + mirror x |
| Mirror Y | 1 | 0 | 0 | 1 | 0 + mirror y |

**Why y2=-1 in the normal case?** Schematic Y is up positive, canvas Y is down positive.
The normal transform already flips Y' — therefore `sch_to_px` to avoid conflict with
when passing lib coordinates through the transform,'do **not** flip Y.

### Python TRANSFORM implementation

```python
# S-expr'den TRANSFORM build
TRANSFORMS = {
    (0,   False, False): (1, 0, 0, -1),
    (90,  False, False): (0, 1, 1,  0),
    (180, False, False): (-1,0, 0,  1),
    (270, False, False): (0,-1,-1,  0),
    (0,   True,  False): (-1,0, 0, -1),   # mirror x
    (0,   False, True ): (1, 0, 0,  1),   # mirror y
    (90,  True,  False): (0,-1, 1,  0),
    (90,  False, True ): (0, 1,-1,  0),
    # ... etc. (kombinasyonlar for rotate sonra mirror apply)
}

def make_transform(angle_deg, mirror_x=False, mirror_y=False):
    """Angle ve mirror'dan TRANSFORM (x1,x2,y1,y2) returns."""
    a = math.radians(angle_deg)
    cos_a, sin_a = round(math.cos(a)), round(math.sin(a))
    x1, x2 = cos_a, -sin_a
    y1, y2 = sin_a,  cos_a
    if mirror_x: x1, x2 = -x1, -x2   # X ekseninde mirror
    if mirror_y: y1, y2 = -y1, -y2   # Y ekseninde mirror
    return (x1, x2, y1, y2)

def transform_point(x1,x2,y1,y2, lx, ly):
    """Lib coordinate to instance coordinate convert."""
    return x1*lx + x2*ly, y1*lx + y2*ly
```

---

## LIB_SYMBOL::Flatten — inheritance resolution

`extends` keyword with parent symbolden inheritance alan symboller flatten must be flattened.
KiCad C++ `LIB_SYMBOL::Flatten()` parent's drawing items copies.

```python
def flatten_lib_symbol(sym, lib_symbols):
    """
    sym: parse flattened symbol node'u
    lib_symbols: all lib_symbols dict'i {id: node}
    Returns: flattened drawing item listesi
    """
    items = list(sym.get('drawing', []))
    parent_id = sym.get('extends')
    while parent_id:
        parent = lib_symbols.get(parent_id)
        if not parent:
            break
        # parent's drawing items ekle (pinler dahil)
        items = list(parent.get('drawing', [])) + items
        parent_id = parent.get('extends')
    return items
```

---

## Unit and Body Style filtering

KiCad C++ `SCH_PAINTER`: `m_unit` ve `m_bodyStyle` values matching
alt-symboller are drawn.

Alt-symbol naming: `"PARENT_UNIT_STYLE"`
- `UNIT=0` → all unitlerde common
- `STYLE=1` → normal, `STYLE=2` → De Morgan

```python
def filter_units(lib_sym_items, unit, body_style=1):
    """
    lib_sym in child symbol tokens
    unit ve body_style matching primitives returns.
    """
    result = []
    for child in lib_sym_items:
        if child['type'] != 'symbol':
            continue
        name   = child['id']          # e.g.: "Device_R_1_1"
        parts  = name.rsplit('_', 2)
        if len(parts) == 3:
            try:
                child_unit  = int(parts[1])
                child_style = int(parts[2])
            except ValueError:
                continue
            # unit=0 → her yerde pass; style=0 → her iki style'da pass
            if child_unit not in (0, unit):
                continue
            if child_style not in (0, body_style):
                continue
        result.extend(child.get('drawing', []))
    return result
```

---

## Symbol render — full flow

```python
def render_symbol(ctx, instance, lib_symbols, scale):
    lib_id  = instance['lib_id']           # e.g.: "Device:R"
    lib_sym = lib_symbols.get(lib_id)
    if not lib_sym:
        return                             # lib not found

    at_x    = instance['at'][0]
    at_y    = instance['at'][1]
    angle   = instance['at'][2] if len(instance['at']) > 2 else 0
    mirror  = instance.get('mirror')       # 'x', 'y', or None
    unit    = instance.get('unit', 1)

    # TRANSFORM
    mx = mirror == 'x'
    my = mirror == 'y'
    tx = make_transform(angle, mx, my)     # (x1,x2,y1,y2)

    # Flatten ve unit filter
    all_items  = flatten_lib_symbol(lib_sym, lib_symbols)
    draw_items = filter_units(all_items, unit)

    ctx.save()
    ctx.translate(at_x*scale, -at_y*scale)

    # Lib primitives draw
    for prim in draw_items:
        render_lib_primitive(ctx, prim, tx, scale)

    # Property metinlerini (reference, value) draw
    for prop in instance.get('properties', []):
        if not prop.get('hide') and prop['key'] in ('Reference','Value'):
            render_field_text(ctx, prop, at_x, at_y, tx, scale)

    ctx.restore()


def render_field_text(ctx, prop, sym_x, sym_y, tx, scale):
    """
    Render a symbol field (Reference or Value) at its DISPLAY position.

    KiCad stores field positions as absolute schematic coordinates in the .kicad_sch
    file (SCH_FIELD::GetTextPos()). But the RENDERER uses SCH_FIELD::GetPosition(),
    which applies the symbol's TRANSFORM matrix to the relative field offset:

        rel = field_pos - sym_pos
        display_rel = TRANSFORM.TransformCoordinate(rel)   # x'=x1*x+x2*y, y'=y1*x+y2*y
        display_pos = sym_pos + display_rel

    For a 0° symbol (y2=-1 in TRANSFORM), this negates the Y component of the
    relative offset, effectively mirroring the field to the correct side of the
    body (e.g. Reference above, Value below for a horizontal resistor).

    Equivalent formula: negate Y of rel, then rotate CCW by sym_rotation.

    tx = (x1, x2, y1, y2) TRANSFORM tuple for the SYMBOL (used only for
    determining the text rotation via GetDrawRotation, not for the position).
    """
    at   = prop.get('at', [0, 0, 0])       # [x, y, angle] in schematic coords
    fx, fy, field_angle = at[0], at[1], at[2] if len(at) > 2 else 0

    # --- Compute display position (GetPosition() equivalent) ---
    # TRANSFORM = negate Y of relative offset, then rotate CCW by sym_rotation.
    # (sym_rotation is embedded in tx via make_transform, but we need the raw angle.)
    # We recover it from the tx tuple: at 0° tx=(1,0,0,-1), at 90° tx=(0,1,1,0), etc.
    x1, x2, y1, y2 = tx
    rel_x = fx - sym_x
    rel_y = fy - sym_y
    disp_x = x1 * rel_x + x2 * rel_y
    disp_y = y1 * rel_x + y2 * rel_y
    px = (sym_x + disp_x) * scale
    py = -(sym_y + disp_y) * scale  # Y-down for canvas

    # --- Text rotation: GetDrawRotation() toggles when y1 != 0 (90°/270°) ---
    draw_angle = field_angle
    if y1 != 0:   # symbol is 90° or 270° rotated
        draw_angle = 90.0 if field_angle == 0.0 else 0.0

    # --- Draw ---
    ctx.save()
    ctx.translate(px, py)
    if abs(draw_angle) > 0.1:
        ctx.rotate(-math.radians(draw_angle))  # canvas CW-positive, so negate CCW angle
    ctx.font      = f"{1.27 * scale}px KiCad Font, monospace"
    ctx.fillStyle = REFERENCE_COLOR if prop['key'] == 'Reference' else VALUE_COLOR
    ctx.textAlign = 'left'
    ctx.textBaseline = 'middle'
    ctx.fillText(prop.get('value', ''), 0, 0)
    ctx.restore()


def render_lib_primitive(ctx, prim, tx, scale):
    """tx = (x1,x2,y1,y2) TRANSFORM tuple."""
    t = prim['type']
    x1,x2,y1,y2 = tx

    def tp(lx, ly):  # transform + Y convertme (canvas Y down)
        nx, ny = transform_point(x1,x2,y1,y2, lx, ly)
        return nx*scale, -ny*scale   # lib'de Y up, ekrana Y convert

    if t == 'polyline':
        pts = prim['pts']
        ctx.beginPath()
        ctx.moveTo(*tp(pts[0][0], pts[0][1]))
        for p in pts[1:]: ctx.lineTo(*tp(p[0], p[1]))
        apply_stroke_fill(ctx, prim, scale)

    elif t == 'rectangle':
        s  = prim['start'];  e = prim['end']
        # 4 corneryi transform'dan passir
        corners = [tp(s[0],s[1]), tp(e[0],s[1]),
                   tp(e[0],e[1]), tp(s[0],e[1])]
        ctx.beginPath()
        ctx.moveTo(*corners[0])
        for c in corners[1:]: ctx.lineTo(*c)
        ctx.closePath()
        apply_stroke_fill(ctx, prim, scale)

    elif t == 'circle':
        cx,cy  = prim['center']
        radius = prim['radius']
        # Pass center through transform; radius scale with scales
        px, py = tp(cx, cy)
        ctx.beginPath()
        ctx.arc(px, py, radius*scale, 0, 2*math.pi)
        apply_stroke_fill(ctx, prim, scale)

    elif t == 'arc':
        s = prim['start']; m = prim['mid']; e = prim['end']
        # Transform'dan pass, sonra calculate arc
        ps = tp(s[0],s[1]); pm = tp(m[0],m[1]); pe = tp(e[0],e[1])
        center, r = arc_center_radius(ps, pm, pe)
        if center:
            t1, t2, ccw = arc_sweep(center, ps, pe, pm)
            ctx.beginPath()
            ctx.arc(center[0], center[1], r, t1, t2, ccw)
            ctx.strokeStyle = SYMBOL_COLOR
            ctx.lineWidth   = DEFAULT_LINE_WIDTH * scale
            ctx.stroke()

    elif t == 'pin':
        render_pin(ctx, prim, tp, scale)


def apply_stroke_fill(ctx, prim, scale):
    """Stroke ve fill apply."""
    w     = prim.get('stroke',{}).get('width', DEFAULT_LINE_WIDTH)
    fill  = prim.get('fill',{}).get('type', 'none')
    ctx.lineWidth   = max(w, DEFAULT_LINE_WIDTH) * scale
    ctx.strokeStyle = SYMBOL_COLOR
    if fill == 'outline':
        ctx.fillStyle = SYMBOL_COLOR
        ctx.fill()
    elif fill == 'background':
        ctx.fillStyle = SYMBOL_BG_COLOR
        ctx.fill()
    ctx.stroke()
```

---

## Pin render — from source code

KiCad `sch_painter.cpp::draw(const SCH_PIN*)` — `PIN_LAYOUT_CACHE` uses.

```python
def render_pin(ctx, pin, tp, scale):
    """
    pin: lib_symbol in pin token
    tp: transform function (lx,ly) → (px,py)
    """
    at        = pin['at']              # [x, y, angle]
    length    = pin.get('length', 2.54)
    graphic   = pin['graphic_style']   # line, inverted, clock, ...
    elec_type = pin['elec_type']       # input, output, passive, ...

    # Pin root ve tip
    lx, ly    = at[0], at[1]
    angle_rad = math.radians(at[2])
    ex_lib    = lx + length * math.cos(angle_rad)
    ey_lib    = ly + length * math.sin(angle_rad)

    px, py = tp(lx, ly)
    ex, ey = tp(ex_lib, ey_lib)

    # Body line
    ctx.beginPath()
    ctx.moveTo(px, py)
    ctx.lineTo(ex, ey)
    ctx.strokeStyle = PIN_COLOR        # '#FF8000'
    ctx.lineWidth   = 0.15 * scale
    ctx.lineCap     = 'round'
    ctx.stroke()

    # Endpoint marker (grafik stil)
    BUBBLE_R = 0.397 * scale           # inverted daire radius

    if graphic == 'inverted' or graphic == 'inverted_clock':
        # Small daire, pin ucunun beyond
        dx = math.cos(angle_rad) * 0.397
        dy = math.sin(angle_rad) * 0.397
        bx, by = tp(ex_lib + dx, ey_lib + dy)
        ctx.beginPath()
        ctx.arc(bx, by, BUBBLE_R, 0, 2*math.pi)
        ctx.strokeStyle = PIN_COLOR
        ctx.lineWidth   = 0.15 * scale
        ctx.stroke()

    if graphic in ('clock', 'inverted_clock', 'clock_low', 'edge_clock_high'):
        # Triangle: pin ucunda, body perpendicular to direction dik
        perp = angle_rad + math.pi/2
        CLOCK_SIZE = 0.794 * scale
        tx1, ty1 = tp(ex_lib, ey_lib + 0.794)
        tx2, ty2 = tp(ex_lib, ey_lib - 0.794)
        tx3, ty3 = tp(ex_lib + 0.794, ey_lib)
        ctx.beginPath()
        ctx.moveTo(tx1, ty1)
        ctx.lineTo(tx3, ty3)
        ctx.lineTo(tx2, ty2)
        ctx.strokeStyle = PIN_COLOR
        ctx.lineWidth   = 0.15 * scale
        ctx.stroke()

    if graphic in ('input_low', 'output_low', 'clock_low'):
        # Inverted L shape (active low bar)
        L = 0.794 * scale
        ctx.beginPath()
        ctx.moveTo(ex, ey)
        ctx.lineTo(ex, ey - L)
        ctx.lineTo(ex + L, ey)
        ctx.strokeStyle = PIN_COLOR
        ctx.lineWidth   = 0.15 * scale
        ctx.stroke()

    # Dangling indicator (pin connected if not small square)
    # Connection checking is performed ekle
    # if pin.get('dangling'):
    #     ctx.strokeRect(px-3, py-3, 6, 6)

    # Pin name ve number
    name_eff = pin.get('name_effects', {})
    num_eff  = pin.get('number_effects', {})
    if not name_eff.get('hide'):
        _render_pin_label(ctx, pin.get('name',''), px, py, ex, ey, 'name', scale)
    if not num_eff.get('hide'):
        _render_pin_label(ctx, pin.get('number',''), px, py, ex, ey, 'number', scale)


def _render_pin_label(ctx, text, px, py, ex, ey, role, scale):
    """Pin name or number uygun konuma yaz."""
    if not text or text == '~':
        return
    SIZE = 1.0 * scale
    ctx.font        = f"{SIZE}px KiCad Font, monospace"
    ctx.fillStyle   = PIN_COLOR
    ctx.textBaseline = 'middle'
    # Name: bodynin inner side (tip point beyond)
    # Number: bodynin orta point
    mid_x = (px + ex) / 2
    mid_y = (py + ey) / 2
    offset = 2
    if role == 'name':
        ctx.textAlign = 'left'
        ctx.fillText(text, ex + offset, ey)
    else:
        ctx.textAlign = 'center'
        ctx.fillText(text, mid_x, mid_y - offset)
```

---

## Critical pitfalls (extracted from source code)

1. **TRANSFORM Y convertme:** Lib coordinates Y up positive. `tp()` function
   both applies transform and `-ny` with canvas to canvas Y. Do not do this twice.

2. **unit=0 common grafikler:** Alt-symbol ismi `_0_`contains , all units.
   are drawn. `filter_units` functionnda `child_unit == 0` checking mandatory.

3. **LIB_SYMBOL::Flatten:** `extends` with inheritance symbols parent's
   drawing items **first** gelmeli (child under the child) are drawn). Speciallikle `Device:C`
   gibi symboller another bir base symbolden extend eder.

4. **Pin angles tam derece:** Lib pin `at[2]` tam derece (90, 180, 270, 0).
   Symbol instance `at[2]` is also full degrees. `round(angle/90)*90` with snaple.

5. **De Morgan (bodyStyle=2):** Logic gates alternatif symbol shape.
   `_X_2` sub-symbol. Basit render for `body_style=1` sufficient.

6. **PIN_LAYOUT_CACHE:** KiCad C++ expensive text extent calculations cache'ler.
   Python'da `ctx.measureText()` with approximation sufficient; for each pin
   separateca cache tutmana gerek yok.

7. **Symbol fields (Reference, Value):** `at` coordinates GLOBAL coordinates —
   instance `at`'e is NOT offset. Direkt sch_to_px with convert.

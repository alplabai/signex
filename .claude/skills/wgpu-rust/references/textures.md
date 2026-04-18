# Textures

---

## Creating a texture

```rust
let texture = device.create_texture(&wgpu::TextureDescriptor {
    label:           Some("Diffuse Texture"),
    size: wgpu::Extent3d {
        width:                image_width,
        height:               image_height,
        depth_or_array_layers: 1,
    },
    mip_level_count: 1,           // or log2(max(width, height)) + 1 for full mip chain
    sample_count:    1,
    dimension:       wgpu::TextureDimension::D2,
    format:          wgpu::TextureFormat::Rgba8UnormSrgb,  // sRGB input
    usage:           wgpu::TextureUsages::TEXTURE_BINDING  // can be bound to shaders
                   | wgpu::TextureUsages::COPY_DST,         // can receive uploaded data
    view_formats:    &[],
});
```

---

## Uploading pixel data

```rust
// rgba8 image data:
let rgba_bytes: &[u8] = image.as_raw();

queue.write_texture(
    wgpu::TexelCopyTextureInfo {
        texture:   &texture,
        mip_level: 0,
        origin:    wgpu::Origin3d::ZERO,
        aspect:    wgpu::TextureAspect::All,
    },
    rgba_bytes,
    wgpu::TexelCopyBufferLayout {
        offset:         0,
        bytes_per_row:  Some(4 * image_width),   // 4 bytes per pixel (RGBA)
        rows_per_image: Some(image_height),
    },
    texture.size(),
);
```

---

## Texture view and sampler

```rust
let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    label:            Some("Diffuse Sampler"),
    address_mode_u:   wgpu::AddressMode::ClampToEdge,
    address_mode_v:   wgpu::AddressMode::ClampToEdge,
    address_mode_w:   wgpu::AddressMode::ClampToEdge,
    mag_filter:       wgpu::FilterMode::Linear,
    min_filter:       wgpu::FilterMode::Linear,
    mipmap_filter:    wgpu::FilterMode::Nearest,
    ..Default::default()
});
```

### AddressMode

| Mode | Description |
|------|-------------|
| `ClampToEdge` | Clamp UV to [0,1]; edge pixels repeat |
| `Repeat` | Tile |
| `MirrorRepeat` | Mirror-tile |

### FilterMode

| Mode | Description |
|------|-------------|
| `Nearest` | Pixel-perfect (pixelated look) |
| `Linear` | Bilinear interpolation |

---

## sRGB vs linear

> **Rule**: store colour data in `Rgba8UnormSrgb`. Non-colour data (normals, metallic maps) use `Rgba8Unorm`.

| Format | Use |
|--------|-----|
| `Rgba8UnormSrgb` | Diffuse/albedo textures (GPU converts sRGB -> linear on sample) |
| `Rgba8Unorm` | Normal maps, roughness, metallic |
| `Rgba16Float` | HDR textures, render targets |
| `Depth32Float` | Depth buffer |
| `Bgra8UnormSrgb` | Common surface/swapchain format on Windows |

The surface format returned by `get_capabilities` is usually `Bgra8UnormSrgb` or `Rgba8UnormSrgb`. Match your render target format to the surface format. Output to an sRGB surface format gives automatic linear->sRGB conversion in the fragment output.

---

## Render target texture

```rust
let render_texture = device.create_texture(&wgpu::TextureDescriptor {
    label:           Some("Off-screen Render Target"),
    size:            wgpu::Extent3d { width: 1920, height: 1080, depth_or_array_layers: 1 },
    mip_level_count: 1,
    sample_count:    1,
    dimension:       wgpu::TextureDimension::D2,
    format:          wgpu::TextureFormat::Rgba16Float,
    usage:           wgpu::TextureUsages::RENDER_ATTACHMENT   // render into it
                   | wgpu::TextureUsages::TEXTURE_BINDING     // sample in next pass
                   | wgpu::TextureUsages::COPY_SRC,            // optional: copy to staging
    view_formats:    &[],
});
```

---

## Texture arrays and cube maps

```rust
// Texture array (6 layers = cube map, or N layers for sprite atlas):
let cube_texture = device.create_texture(&wgpu::TextureDescriptor {
    size: wgpu::Extent3d { width: 512, height: 512, depth_or_array_layers: 6 },
    ..texture_desc_base
});

let cube_view = cube_texture.create_view(&wgpu::TextureViewDescriptor {
    dimension:   Some(wgpu::TextureViewDimension::Cube),
    array_layer_count: Some(6),
    ..Default::default()
});
```

WGSL: `var t: texture_cube<f32>;`

---

## Mipmaps

wgpu does **not** generate mipmaps automatically. Either:
1. Provide all mip levels manually via `write_texture` per mip level.
2. Use a compute shader to generate mips.
3. Use the `wgpu::util::generate_mipmap` helper (available in some versions).

```rust
// Upload mip level 1 (half resolution):
queue.write_texture(
    wgpu::TexelCopyTextureInfo {
        texture: &texture, mip_level: 1, origin: wgpu::Origin3d::ZERO,
        aspect: wgpu::TextureAspect::All,
    },
    &mip1_bytes,
    wgpu::TexelCopyBufferLayout {
        offset: 0,
        bytes_per_row: Some(4 * (image_width / 2)),
        rows_per_image: Some(image_height / 2),
    },
    wgpu::Extent3d { width: image_width / 2, height: image_height / 2, depth_or_array_layers: 1 },
);
```

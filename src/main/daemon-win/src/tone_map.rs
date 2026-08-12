use crate::modules::recorder_types::fit_rect;
use std::ffi::CStr;
use windows::core::PCSTR;
use windows::Win32::Graphics::Direct3D::Fxc::{D3DCompile, D3DCOMPILE_OPTIMIZATION_LEVEL3};
use windows::Win32::Graphics::Direct3D::{ID3DBlob, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST};
use windows::Win32::Graphics::Direct3D11::{
    ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11PixelShader, ID3D11RenderTargetView,
    ID3D11SamplerState, ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader,
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BUFFER_DESC, D3D11_COMPARISON_NEVER, D3D11_CPU_ACCESS_WRITE,
    D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_WRITE_DISCARD,
    D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DYNAMIC, D3D11_VIEWPORT,
};

#[cfg(test)]
const LUMA_RED: f32 = 0.2126;
#[cfg(test)]
const LUMA_GREEN: f32 = 0.7152;
#[cfg(test)]
const LUMA_BLUE: f32 = 0.0722;

const SHADER_SOURCE: &str = r#"
Texture2D<float4> Source : register(t0);
SamplerState Bilinear : register(s0);

cbuffer Params : register(b0)
{
    int2 Offset;
    float InverseWhiteScale;
    float Padding;
};

cbuffer FitParams : register(b1)
{
    float2 SourceOrigin;
    float2 SourceExtent;
    float2 TextureExtent;
    float FitInverseWhiteScale;
    float FitPadding;
};

struct FitVertex
{
    float4 position : SV_POSITION;
    float2 unit : TEXCOORD0;
};

static const float3 Luma = float3(0.2126, 0.7152, 0.0722);

float EncodeChannel(float value)
{
    float clamped = saturate(value);
    return clamped <= 0.0031308
        ? clamped * 12.92
        : 1.055 * pow(clamped, 1.0 / 2.4) - 0.055;
}

float3 ToneMap(float3 color, float inverseWhiteScale)
{
    float3 linearColor = max(color * inverseWhiteScale, 0.0);
    float luminance = dot(linearColor, Luma);

    if (luminance > 1.0)
    {
        linearColor *= 1.0 / luminance;
    }

    return float3(
        EncodeChannel(linearColor.r),
        EncodeChannel(linearColor.g),
        EncodeChannel(linearColor.b));
}

float2 TriangleCorner(uint id)
{
    return float2((id << 1) & 2, id & 2);
}

float4 VertexMain(uint id : SV_VertexID) : SV_POSITION
{
    float2 corner = TriangleCorner(id);
    return float4(corner * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
}

float4 PixelMain(float4 position : SV_POSITION) : SV_TARGET
{
    int3 coordinate = int3(int2(position.xy) + Offset, 0);
    return float4(ToneMap(Source.Load(coordinate).rgb, InverseWhiteScale), 1.0);
}

FitVertex FitVertexMain(uint id : SV_VertexID)
{
    float2 corner = TriangleCorner(id);
    FitVertex output;
    output.position = float4(corner * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    output.unit = corner;
    return output;
}

float4 FitPixelMain(FitVertex input) : SV_TARGET
{
    float2 texel = SourceOrigin + input.unit * SourceExtent;
    float3 sampled = Source.SampleLevel(Bilinear, texel / TextureExtent, 0).rgb;

    if (FitInverseWhiteScale <= 0.0)
    {
        return float4(sampled, 1.0);
    }

    return float4(ToneMap(sampled, FitInverseWhiteScale), 1.0);
}
"#;

const LETTERBOX_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[repr(C)]
#[derive(Clone, Copy)]
struct Params {
    offset_x: i32,
    offset_y: i32,
    inverse_white_scale: f32,
    padding: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FitParams {
    source_origin: [f32; 2],
    source_extent: [f32; 2],
    texture_extent: [f32; 2],
    inverse_white_scale: f32,
    padding: f32,
}

#[cfg(test)]
pub fn reference_tone_map(white_scale: f32, red: f32, green: f32, blue: f32) -> [u8; 3] {
    let inverse = 1.0 / white_scale.max(1.0);
    let mut color = [
        (red * inverse).max(0.0),
        (green * inverse).max(0.0),
        (blue * inverse).max(0.0),
    ];
    let luminance = LUMA_RED * color[0] + LUMA_GREEN * color[1] + LUMA_BLUE * color[2];

    if luminance > 1.0 {
        let gain = 1.0 / luminance;
        for channel in &mut color {
            *channel *= gain;
        }
    }

    [
        encode_channel(color[0]),
        encode_channel(color[1]),
        encode_channel(color[2]),
    ]
}

#[cfg(test)]
fn encode_channel(value: f32) -> u8 {
    let clamped = value.clamp(0.0, 1.0);
    let encoded = if clamped <= 0.0031308 {
        clamped * 12.92
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

fn compile(entry: &CStr, target: &CStr) -> Result<ID3DBlob, String> {
    let mut code = None;
    let mut errors = None;
    let result = unsafe {
        D3DCompile(
            SHADER_SOURCE.as_ptr() as *const _,
            SHADER_SOURCE.len(),
            None,
            None,
            None,
            PCSTR(entry.as_ptr() as *const u8),
            PCSTR(target.as_ptr() as *const u8),
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        )
    };

    if let Err(error) = result {
        let detail = errors
            .and_then(|blob| unsafe {
                let pointer = blob.GetBufferPointer() as *const u8;
                let length = blob.GetBufferSize();
                std::str::from_utf8(std::slice::from_raw_parts(pointer, length))
                    .ok()
                    .map(str::to_string)
            })
            .unwrap_or_else(|| error.to_string());
        return Err(detail);
    }

    code.ok_or_else(|| "Shader compiler produced no bytecode".to_string())
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())
    }
}

pub fn source_view(
    device: &ID3D11Device,
    source: &ID3D11Texture2D,
) -> Result<ID3D11ShaderResourceView, String> {
    let mut view = None;
    unsafe {
        device
            .CreateShaderResourceView(source, None, Some(&mut view))
            .map_err(|error| format!("Failed to view the captured frame: {error}"))?;
    }
    view.ok_or_else(|| "Captured frame view was not created".to_string())
}

pub fn target_view(
    device: &ID3D11Device,
    target: &ID3D11Texture2D,
) -> Result<ID3D11RenderTargetView, String> {
    let mut view = None;
    unsafe {
        device
            .CreateRenderTargetView(target, None, Some(&mut view))
            .map_err(|error| format!("Failed to view the encoder texture: {error}"))?;
    }
    view.ok_or_else(|| "Encoder texture view was not created".to_string())
}

fn create_shaders(
    device: &ID3D11Device,
    vertex_entry: &CStr,
    pixel_entry: &CStr,
    label: &str,
) -> Result<(ID3D11VertexShader, ID3D11PixelShader), String> {
    let vertex_code = compile(vertex_entry, c"vs_5_0")?;
    let pixel_code = compile(pixel_entry, c"ps_5_0")?;

    let mut vertex = None;
    unsafe {
        device
            .CreateVertexShader(blob_bytes(&vertex_code), None, Some(&mut vertex))
            .map_err(|error| format!("Failed to create {label} vertex shader: {error}"))?;
    }
    let mut pixel = None;
    unsafe {
        device
            .CreatePixelShader(blob_bytes(&pixel_code), None, Some(&mut pixel))
            .map_err(|error| format!("Failed to create {label} pixel shader: {error}"))?;
    }

    Ok((
        vertex.ok_or_else(|| format!("The {label} vertex shader was not created"))?,
        pixel.ok_or_else(|| format!("The {label} pixel shader was not created"))?,
    ))
}

fn create_constants(
    device: &ID3D11Device,
    size: usize,
    label: &str,
) -> Result<ID3D11Buffer, String> {
    let descriptor = D3D11_BUFFER_DESC {
        ByteWidth: size as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut buffer = None;
    unsafe {
        device
            .CreateBuffer(&descriptor, None, Some(&mut buffer))
            .map_err(|error| format!("Failed to create {label} constants: {error}"))?;
    }
    buffer.ok_or_else(|| format!("The {label} constants were not created"))
}

unsafe fn upload_constants<T>(
    context: &ID3D11DeviceContext,
    buffer: &ID3D11Buffer,
    value: &T,
    label: &str,
) -> Result<(), String> {
    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        context
            .Map(buffer, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
            .map_err(|error| format!("Failed to update {label} constants: {error}"))?;
        std::ptr::copy_nonoverlapping(value, mapped.pData as *mut T, 1);
        context.Unmap(buffer, 0);
    }
    Ok(())
}

pub struct ToneMapStage {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    params: ID3D11Buffer,
    inverse_white_scale: f32,
}

impl ToneMapStage {
    pub fn new(device: &ID3D11Device, white_scale: f32) -> Result<Self, String> {
        let (vertex, pixel) = create_shaders(device, c"VertexMain", c"PixelMain", "tone map")?;

        Ok(Self {
            vertex,
            pixel,
            params: create_constants(device, std::mem::size_of::<Params>(), "tone map")?,
            inverse_white_scale: 1.0 / white_scale.max(1.0),
        })
    }

    pub fn run(
        &self,
        context: &ID3D11DeviceContext,
        source: &ID3D11ShaderResourceView,
        target: &ID3D11RenderTargetView,
        offset: (u32, u32),
        size: (u32, u32),
    ) -> Result<(), String> {
        let params = Params {
            offset_x: offset.0 as i32,
            offset_y: offset.1 as i32,
            inverse_white_scale: self.inverse_white_scale,
            padding: 0.0,
        };

        unsafe {
            upload_constants(context, &self.params, &params, "tone map")?;

            context.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: size.0 as f32,
                Height: size.1 as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetInputLayout(None);
            context.VSSetShader(&self.vertex, None);
            context.PSSetShader(&self.pixel, None);
            context.PSSetShaderResources(0, Some(&[Some(source.clone())]));
            context.PSSetConstantBuffers(0, Some(&[Some(self.params.clone())]));
            context.Draw(3, 0);
            context.PSSetShaderResources(0, Some(&[None]));
            context.OMSetRenderTargets(None, None);
        }

        Ok(())
    }
}

/// Scales a captured region into a fixed-size encoder texture, letterboxing
/// whatever does not fill it. A recorded window changes size while the video
/// dimensions cannot, so every window frame goes through this pass.
pub struct FitStage {
    vertex: ID3D11VertexShader,
    pixel: ID3D11PixelShader,
    params: ID3D11Buffer,
    sampler: ID3D11SamplerState,
    inverse_white_scale: f32,
}

impl FitStage {
    pub fn new(device: &ID3D11Device, white_scale: Option<f32>) -> Result<Self, String> {
        let (vertex, pixel) = create_shaders(device, c"FitVertexMain", c"FitPixelMain", "fit")?;
        let descriptor = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            MaxLOD: f32::MAX,
            ..Default::default()
        };
        let mut sampler = None;
        unsafe {
            device
                .CreateSamplerState(&descriptor, Some(&mut sampler))
                .map_err(|error| format!("Failed to create the fit sampler: {error}"))?;
        }

        Ok(Self {
            vertex,
            pixel,
            params: create_constants(device, std::mem::size_of::<FitParams>(), "fit")?,
            sampler: sampler.ok_or("The fit sampler was not created")?,
            inverse_white_scale: white_scale.map_or(0.0, |scale| 1.0 / scale.max(1.0)),
        })
    }

    pub fn run(
        &self,
        context: &ID3D11DeviceContext,
        source: &ID3D11ShaderResourceView,
        target: &ID3D11RenderTargetView,
        content: (u32, u32),
        texture: (u32, u32),
        frame: (u32, u32),
    ) -> Result<(), String> {
        let fit = fit_rect(content, frame);
        if fit.width <= 0.0 || fit.height <= 0.0 {
            return Err("The captured window has no visible area".to_string());
        }

        let params = FitParams {
            source_origin: [0.0, 0.0],
            source_extent: [content.0 as f32, content.1 as f32],
            texture_extent: [texture.0 as f32, texture.1 as f32],
            inverse_white_scale: self.inverse_white_scale,
            padding: 0.0,
        };

        unsafe {
            upload_constants(context, &self.params, &params, "fit")?;

            context.OMSetRenderTargets(Some(&[Some(target.clone())]), None);
            context.ClearRenderTargetView(target, &LETTERBOX_COLOR);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                TopLeftX: fit.x as f32,
                TopLeftY: fit.y as f32,
                Width: fit.width as f32,
                Height: fit.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            }]));
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.IASetInputLayout(None);
            context.VSSetShader(&self.vertex, None);
            context.PSSetShader(&self.pixel, None);
            context.PSSetShaderResources(0, Some(&[Some(source.clone())]));
            context.PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            context.PSSetConstantBuffers(1, Some(&[Some(self.params.clone())]));
            context.Draw(3, 0);
            context.PSSetShaderResources(0, Some(&[None]));
            context.OMSetRenderTargets(None, None);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::reference_tone_map;
    use crate::display_color::ToneMapper;

    fn assert_matches(white_scale: f32, red: f32, green: f32, blue: f32) {
        let expected = ToneMapper::new(white_scale).map(red, green, blue);
        let actual = reference_tone_map(white_scale, red, green, blue);

        for channel in 0..3 {
            assert!(
                expected[channel].abs_diff(actual[channel]) <= 1,
                "white_scale {white_scale} rgb ({red}, {green}, {blue}) channel {channel}: screenshot {expected:?} shader {actual:?}"
            );
        }
    }

    #[test]
    fn matches_the_screenshot_mapper_across_the_sdr_range() {
        for step in 0..=64 {
            let value = step as f32 / 64.0;
            assert_matches(2.5, value, value, value);
        }
    }

    #[test]
    fn matches_the_screenshot_mapper_on_highlights() {
        for step in 1..=32 {
            let value = 1.0 + step as f32 / 4.0;
            assert_matches(2.5, value, value * 0.5, value * 0.25);
        }
    }

    #[test]
    fn matches_the_screenshot_mapper_across_white_levels() {
        for scale in [1.0, 1.25, 2.0, 2.5, 4.0, 6.25] {
            assert_matches(scale, 0.8, 0.4, 0.1);
            assert_matches(scale, 3.0, 3.0, 3.0);
        }
    }

    #[test]
    fn clamps_negative_channels_to_black() {
        assert_eq!(reference_tone_map(2.5, -1.0, -1.0, -1.0), [0, 0, 0]);
    }

    #[test]
    fn maps_white_at_the_sdr_level_to_full_white() {
        assert_eq!(reference_tone_map(2.5, 2.5, 2.5, 2.5), [255, 255, 255]);
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::{reference_tone_map, FitStage, ToneMapStage};
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0};
    use windows::Win32::Graphics::Direct3D11::{
        D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
        D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ,
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
        D3D11_SDK_VERSION, D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
        D3D11_USAGE_STAGING,
    };
    use windows::Win32::Graphics::Dxgi::Common::{
        DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_SAMPLE_DESC,
    };

    const SOURCE_WIDTH: u32 = 4;
    const SOURCE_HEIGHT: u32 = 2;
    const CROP_WIDTH: u32 = 2;
    const CROP_HEIGHT: u32 = 2;
    const CROP_X: u32 = 1;
    const CROP_Y: u32 = 0;
    const WHITE_SCALE: f32 = 2.5;

    fn f32_to_half(value: f32) -> u16 {
        let bits = value.to_bits();
        let sign = ((bits >> 16) & 0x8000) as u16;
        let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
        let mantissa = bits & 0x007f_ffff;

        if exponent <= 0 {
            return sign;
        }
        if exponent >= 0x1f {
            return sign | 0x7c00;
        }
        sign | ((exponent as u16) << 10) | ((mantissa >> 13) as u16)
    }

    fn create_device() -> Option<(ID3D11Device, ID3D11DeviceContext)> {
        let mut device = None;
        let mut context = None;
        let result = unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        };
        result.ok()?;
        Some((device?, context?))
    }

    fn source_pixels() -> Vec<[f32; 4]> {
        vec![
            [0.0, 0.0, 0.0, 1.0],
            [0.25, 0.5, 0.75, 1.0],
            [2.5, 2.5, 2.5, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            [6.0, 1.0, 0.25, 1.0],
            [0.125, 0.125, 0.125, 1.0],
            [-1.0, 0.5, 3.0, 1.0],
            [2.0, 2.0, 0.0, 1.0],
        ]
    }

    #[test]
    fn shader_compiles_and_matches_the_reference_on_a_real_device() {
        let Some((device, context)) = create_device() else {
            eprintln!("skipping: no D3D11 WARP device available");
            return;
        };

        let stage = ToneMapStage::new(&device, WHITE_SCALE).expect("tone map stage");

        let pixels = source_pixels();
        let halves: Vec<u16> = pixels
            .iter()
            .flat_map(|pixel| pixel.iter().map(|channel| f32_to_half(*channel)))
            .collect();

        let source_descriptor = D3D11_TEXTURE2D_DESC {
            Width: SOURCE_WIDTH,
            Height: SOURCE_HEIGHT,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let initial = D3D11_SUBRESOURCE_DATA {
            pSysMem: halves.as_ptr() as *const _,
            SysMemPitch: SOURCE_WIDTH * 8,
            SysMemSlicePitch: 0,
        };
        let mut source = None;
        unsafe {
            device
                .CreateTexture2D(&source_descriptor, Some(&initial), Some(&mut source))
                .expect("source texture");
        }
        let source: ID3D11Texture2D = source.expect("source texture");

        let target_descriptor = D3D11_TEXTURE2D_DESC {
            Width: CROP_WIDTH,
            Height: CROP_HEIGHT,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            ..source_descriptor
        };
        let mut target = None;
        unsafe {
            device
                .CreateTexture2D(&target_descriptor, None, Some(&mut target))
                .expect("target texture");
        }
        let target: ID3D11Texture2D = target.expect("target texture");

        let source_view = super::source_view(&device, &source).expect("source view");
        let target_view = super::target_view(&device, &target).expect("target view");
        stage
            .run(
                &context,
                &source_view,
                &target_view,
                (CROP_X, CROP_Y),
                (CROP_WIDTH, CROP_HEIGHT),
            )
            .expect("tone map dispatch");

        let staging_descriptor = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..target_descriptor
        };
        let mut staging = None;
        unsafe {
            device
                .CreateTexture2D(&staging_descriptor, None, Some(&mut staging))
                .expect("staging texture");
        }
        let staging: ID3D11Texture2D = staging.expect("staging texture");
        let staging_resource: ID3D11Resource = staging.clone().into();
        let target_resource: ID3D11Resource = target.clone().into();
        unsafe {
            context.CopyResource(&staging_resource, &target_resource);
            context.Flush();
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(&staging_resource, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .expect("map staging");
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(
                mapped.pData as *const u8,
                (mapped.RowPitch * CROP_HEIGHT) as usize,
            )
        };

        let mut mismatches = Vec::new();
        for y in 0..CROP_HEIGHT {
            for x in 0..CROP_WIDTH {
                let source_index = ((y + CROP_Y) * SOURCE_WIDTH + (x + CROP_X)) as usize;
                let pixel = pixels[source_index];
                let expected = reference_tone_map(WHITE_SCALE, pixel[0], pixel[1], pixel[2]);
                let offset = (y * mapped.RowPitch + x * 4) as usize;
                let actual = [bytes[offset + 2], bytes[offset + 1], bytes[offset]];

                for channel in 0..3 {
                    if expected[channel].abs_diff(actual[channel]) > 1 {
                        mismatches.push(format!(
                            "({x},{y}) source {pixel:?} expected {expected:?} gpu {actual:?}"
                        ));
                        break;
                    }
                }
            }
        }

        unsafe {
            context.Unmap(&staging_resource, 0);
        }

        assert!(mismatches.is_empty(), "{}", mismatches.join("; "));
    }

    fn bgra_texture(
        device: &ID3D11Device,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> ID3D11Texture2D {
        let descriptor = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let initial = D3D11_SUBRESOURCE_DATA {
            pSysMem: pixels.as_ptr() as *const _,
            SysMemPitch: width * 4,
            SysMemSlicePitch: 0,
        };
        let mut texture = None;
        unsafe {
            device
                .CreateTexture2D(&descriptor, Some(&initial), Some(&mut texture))
                .expect("source texture");
        }
        texture.expect("source texture")
    }

    fn render_target(device: &ID3D11Device, width: u32, height: u32) -> ID3D11Texture2D {
        let descriptor = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe {
            device
                .CreateTexture2D(&descriptor, None, Some(&mut texture))
                .expect("target texture");
        }
        texture.expect("target texture")
    }

    fn read_back(
        device: &ID3D11Device,
        context: &ID3D11DeviceContext,
        texture: &ID3D11Texture2D,
        width: u32,
        height: u32,
    ) -> Vec<[u8; 3]> {
        let mut descriptor = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.GetDesc(&mut descriptor) };
        let staging_descriptor = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..descriptor
        };
        let mut staging = None;
        unsafe {
            device
                .CreateTexture2D(&staging_descriptor, None, Some(&mut staging))
                .expect("staging texture");
        }
        let staging: ID3D11Texture2D = staging.expect("staging texture");
        let staging_resource: ID3D11Resource = staging.clone().into();
        let source_resource: ID3D11Resource = texture.clone().into();
        unsafe {
            context.CopyResource(&staging_resource, &source_resource);
            context.Flush();
        }

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            context
                .Map(&staging_resource, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .expect("map staging");
        }
        let bytes = unsafe {
            std::slice::from_raw_parts(
                mapped.pData as *const u8,
                (mapped.RowPitch * height) as usize,
            )
        };
        let mut read = Vec::with_capacity((width * height) as usize);
        for y in 0..height {
            for x in 0..width {
                let offset = (y * mapped.RowPitch + x * 4) as usize;
                read.push([bytes[offset + 2], bytes[offset + 1], bytes[offset]]);
            }
        }
        unsafe {
            context.Unmap(&staging_resource, 0);
        }
        read
    }

    #[test]
    fn fit_shader_keeps_an_unresized_window_pixel_exact() {
        let Some((device, context)) = create_device() else {
            eprintln!("skipping: no D3D11 WARP device available");
            return;
        };

        let pixels: Vec<u8> = vec![
            10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
        ];
        let source = bgra_texture(&device, 2, 2, &pixels);
        let target = render_target(&device, 2, 2);
        let stage = FitStage::new(&device, None).expect("fit stage");

        stage
            .run(
                &context,
                &super::source_view(&device, &source).expect("source view"),
                &super::target_view(&device, &target).expect("target view"),
                (2, 2),
                (2, 2),
                (2, 2),
            )
            .expect("fit dispatch");

        let read = read_back(&device, &context, &target, 2, 2);
        let expected: Vec<[u8; 3]> = pixels
            .chunks_exact(4)
            .map(|pixel| [pixel[2], pixel[1], pixel[0]])
            .collect();

        assert_eq!(read, expected);
    }

    #[test]
    fn fit_shader_letterboxes_a_window_that_no_longer_fills_the_frame() {
        let Some((device, context)) = create_device() else {
            eprintln!("skipping: no D3D11 WARP device available");
            return;
        };

        let pixels: Vec<u8> = vec![
            10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
        ];
        let source = bgra_texture(&device, 2, 2, &pixels);
        let target = render_target(&device, 4, 2);
        let stage = FitStage::new(&device, None).expect("fit stage");

        stage
            .run(
                &context,
                &super::source_view(&device, &source).expect("source view"),
                &super::target_view(&device, &target).expect("target view"),
                (2, 2),
                (2, 2),
                (4, 2),
            )
            .expect("fit dispatch");

        let read = read_back(&device, &context, &target, 4, 2);
        let black = [0u8, 0, 0];

        assert_eq!(read[0], black);
        assert_eq!(read[3], black);
        assert_eq!(read[4], black);
        assert_eq!(read[7], black);
        assert_eq!(read[1], [30, 20, 10]);
        assert_eq!(read[2], [60, 50, 40]);
        assert_eq!(read[5], [90, 80, 70]);
        assert_eq!(read[6], [120, 110, 100]);
    }

    #[test]
    fn fit_shader_tone_maps_an_hdr_window() {
        let Some((device, context)) = create_device() else {
            eprintln!("skipping: no D3D11 WARP device available");
            return;
        };

        let pixels = source_pixels();
        let halves: Vec<u16> = pixels
            .iter()
            .flat_map(|pixel| pixel.iter().map(|channel| f32_to_half(*channel)))
            .collect();
        let descriptor = D3D11_TEXTURE2D_DESC {
            Width: SOURCE_WIDTH,
            Height: SOURCE_HEIGHT,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let initial = D3D11_SUBRESOURCE_DATA {
            pSysMem: halves.as_ptr() as *const _,
            SysMemPitch: SOURCE_WIDTH * 8,
            SysMemSlicePitch: 0,
        };
        let mut source = None;
        unsafe {
            device
                .CreateTexture2D(&descriptor, Some(&initial), Some(&mut source))
                .expect("source texture");
        }
        let source: ID3D11Texture2D = source.expect("source texture");
        let target = render_target(&device, SOURCE_WIDTH, SOURCE_HEIGHT);
        let stage = FitStage::new(&device, Some(WHITE_SCALE)).expect("fit stage");

        stage
            .run(
                &context,
                &super::source_view(&device, &source).expect("source view"),
                &super::target_view(&device, &target).expect("target view"),
                (SOURCE_WIDTH, SOURCE_HEIGHT),
                (SOURCE_WIDTH, SOURCE_HEIGHT),
                (SOURCE_WIDTH, SOURCE_HEIGHT),
            )
            .expect("fit dispatch");

        let read = read_back(&device, &context, &target, SOURCE_WIDTH, SOURCE_HEIGHT);
        let mut mismatches = Vec::new();
        for (index, pixel) in pixels.iter().enumerate() {
            let expected = reference_tone_map(WHITE_SCALE, pixel[0], pixel[1], pixel[2]);
            let actual = read[index];
            if (0..3).any(|channel| expected[channel].abs_diff(actual[channel]) > 1) {
                mismatches.push(format!("{index}: expected {expected:?} gpu {actual:?}"));
            }
        }

        assert!(mismatches.is_empty(), "{}", mismatches.join("; "));
    }
}

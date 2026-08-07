use crate::display_color::{hdr_white_scale, ToneMapper};
use crate::overlay::{monitors, rect_height, rect_width, to_wide, MonitorEntry};
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::mpsc::channel;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use windows::core::{factory, IInspectable, Interface, PCWSTR};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{
    Direct3D11CaptureFrame, Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::{GENERIC_WRITE, HMODULE, HWND, POINT, RECT};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0,
    D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
    D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ,
    D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::{IDXGIAdapter, IDXGIDevice};
use windows::Win32::Graphics::Gdi::{
    CreateDIBSection, DeleteObject, MonitorFromPoint, MonitorFromWindow, BITMAPINFO,
    BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HMONITOR, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_ContainerFormatPng, GUID_WICPixelFormat32bppBGRA,
    IWICBitmapFrameEncode, IWICImagingFactory, WICBitmapEncoderNoCache,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

const FRAME_POOL_SIZE: i32 = 2;
const FRAME_TIMEOUT: Duration = Duration::from_secs(3);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);
const BYTES_PER_PIXEL: usize = 4;
const HDR_BYTES_PER_PIXEL: usize = 8;

pub struct DesktopFrame {
    pub bounds: RECT,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub fn capture_monitors() -> Vec<DesktopFrame> {
    run_isolated(|| {
        let Ok(device) = CaptureDevice::create() else {
            return Vec::new();
        };

        monitors()
            .iter()
            .filter_map(|monitor| capture_monitor(&device, monitor).ok())
            .collect()
    })
    .unwrap_or_default()
}

pub fn capture_rect(bounds: RECT) -> Result<DesktopFrame, String> {
    run_isolated(move || {
        let device = CaptureDevice::create()?;
        let monitor = monitor_for_rect(bounds)
            .ok_or_else(|| "The capture area is not on a connected display".to_string())?;
        let frame = capture_monitor(&device, &monitor)?;
        crop(&frame, bounds)
            .ok_or_else(|| "The capture area is outside the display bounds".to_string())
    })
    .unwrap_or_else(|| Err("Timed out while capturing the screen".to_string()))
}

pub fn capture_window(window: HWND) -> Result<DesktopFrame, String> {
    let handle = window.0 as isize;

    run_isolated(move || {
        let window = HWND(handle as *mut c_void);
        let device = CaptureDevice::create()?;
        let interop = capture_interop()?;
        let item = unsafe { interop.CreateForWindow(window) }
            .map_err(|error| format!("Failed to open window capture: {error}"))?;
        let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
        let white_scale = monitors()
            .iter()
            .find(|entry| entry.handle == monitor.0 as isize)
            .and_then(|entry| hdr_white_scale(&entry.device));

        capture_item(&device, &item, RECT::default(), white_scale)
    })
    .unwrap_or_else(|| Err("Timed out while capturing the window".to_string()))
}

pub fn store_frozen(frames: Vec<DesktopFrame>) {
    if let Ok(mut frozen) = frozen_frames().lock() {
        *frozen = frames;
    }
}

pub fn clear_frozen() {
    if let Ok(mut frozen) = frozen_frames().lock() {
        frozen.clear();
    }
}

pub fn frozen_rect(bounds: RECT) -> Option<DesktopFrame> {
    let frozen = frozen_frames().lock().ok()?;
    let center = center_of(bounds);

    frozen
        .iter()
        .find(|frame| contains(&frame.bounds, center))
        .and_then(|frame| crop(frame, bounds))
}

pub fn to_hbitmap(frame: &DesktopFrame) -> Option<HBITMAP> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: frame.width as i32,
            biHeight: -(frame.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: frame.pixels.len() as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut bits: *mut c_void = null_mut();
    let bitmap =
        unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) }.ok()?;

    if bits.is_null() {
        unsafe {
            let _ = DeleteObject(bitmap.into());
        }
        return None;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(frame.pixels.as_ptr(), bits as *mut u8, frame.pixels.len());
    }

    Some(bitmap)
}

pub fn write_png(frame: &DesktopFrame, path: &str) -> Result<(), String> {
    let _apartment = ImagingApartment::initialize()?;
    let factory: IWICImagingFactory =
        unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| format!("Failed to open the image encoder: {error}"))?;

    let stream = unsafe { factory.CreateStream() }
        .map_err(|error| format!("Failed to open the image stream: {error}"))?;
    let wide_path = to_wide(path);
    unsafe {
        stream
            .InitializeFromFilename(PCWSTR(wide_path.as_ptr()), GENERIC_WRITE.0)
            .map_err(|error| format!("Failed to create '{path}': {error}"))?;
    }

    let encoder = unsafe { factory.CreateEncoder(&GUID_ContainerFormatPng, null()) }
        .map_err(|error| format!("Failed to create the PNG encoder: {error}"))?;
    unsafe {
        encoder
            .Initialize(&stream, WICBitmapEncoderNoCache)
            .map_err(|error| format!("Failed to initialize the PNG encoder: {error}"))?;
    }

    let mut encoded_frame: Option<IWICBitmapFrameEncode> = None;
    unsafe {
        encoder
            .CreateNewFrame(&mut encoded_frame, null_mut())
            .map_err(|error| format!("Failed to create the PNG frame: {error}"))?;
    }
    let encoded_frame =
        encoded_frame.ok_or_else(|| "The PNG frame was not created".to_string())?;

    let mut format = GUID_WICPixelFormat32bppBGRA;
    unsafe {
        encoded_frame
            .Initialize(None)
            .map_err(|error| format!("Failed to initialize the PNG frame: {error}"))?;
        encoded_frame
            .SetSize(frame.width, frame.height)
            .map_err(|error| format!("Failed to size the PNG frame: {error}"))?;
        encoded_frame
            .SetPixelFormat(&mut format)
            .map_err(|error| format!("Failed to configure the PNG frame: {error}"))?;
        encoded_frame
            .WritePixels(
                frame.height,
                frame.width * BYTES_PER_PIXEL as u32,
                &frame.pixels,
            )
            .map_err(|error| format!("Failed to write the PNG frame: {error}"))?;
        encoded_frame
            .Commit()
            .map_err(|error| format!("Failed to commit the PNG frame: {error}"))?;
        encoder
            .Commit()
            .map_err(|error| format!("Failed to save '{path}': {error}"))?;
    }

    Ok(())
}

struct CaptureDevice {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    winrt: IDirect3DDevice,
}

impl CaptureDevice {
    fn create() -> Result<Self, String> {
        let (device, context) = create_d3d_device(D3D_DRIVER_TYPE_HARDWARE)
            .or_else(|_| create_d3d_device(D3D_DRIVER_TYPE_WARP))?;
        let dxgi_device: IDXGIDevice = device
            .cast()
            .map_err(|error| format!("Failed to access the graphics device: {error}"))?;
        let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
            .map_err(|error| format!("Failed to create the capture device: {error}"))?;
        let winrt: IDirect3DDevice = inspectable
            .cast()
            .map_err(|error| format!("Failed to access the capture device: {error}"))?;

        Ok(CaptureDevice {
            device,
            context,
            winrt,
        })
    }
}

struct ImagingApartment;

impl ImagingApartment {
    fn initialize() -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|error| format!("Failed to initialize the image encoder: {error}"))?;
        }
        Ok(ImagingApartment)
    }
}

impl Drop for ImagingApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

fn frozen_frames() -> &'static Mutex<Vec<DesktopFrame>> {
    static FROZEN: OnceLock<Mutex<Vec<DesktopFrame>>> = OnceLock::new();
    FROZEN.get_or_init(|| Mutex::new(Vec::new()))
}

fn run_isolated<T: Send + 'static>(job: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (sender, receiver) = channel();
    let worker = std::thread::Builder::new()
        .spawn(move || {
            let initialized = unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.is_ok();
            let _ = sender.send(job());
            if initialized {
                unsafe {
                    RoUninitialize();
                }
            }
        })
        .ok()?;

    let result = receiver.recv_timeout(CAPTURE_TIMEOUT).ok();
    drop(worker);
    result
}

fn capture_interop() -> Result<IGraphicsCaptureItemInterop, String> {
    if !GraphicsCaptureSession::IsSupported().unwrap_or(false) {
        return Err("Windows Graphics Capture is not available on this system".to_string());
    }

    factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|error| format!("Failed to open the capture factory: {error}"))
}

fn capture_monitor(
    device: &CaptureDevice,
    monitor: &MonitorEntry,
) -> Result<DesktopFrame, String> {
    let interop = capture_interop()?;
    let handle = HMONITOR(monitor.handle as *mut c_void);
    let item = unsafe { interop.CreateForMonitor(handle) }
        .map_err(|error| format!("Failed to open display capture: {error}"))?;

    capture_item(
        device,
        &item,
        monitor.rect,
        hdr_white_scale(&monitor.device),
    )
}

fn capture_item(
    device: &CaptureDevice,
    item: &GraphicsCaptureItem,
    bounds: RECT,
    white_scale: Option<f32>,
) -> Result<DesktopFrame, String> {
    let size = item
        .Size()
        .map_err(|error| format!("Failed to read the capture size: {error}"))?;
    if size.Width <= 0 || size.Height <= 0 {
        return Err("The capture target has no visible area".to_string());
    }

    let format = match white_scale {
        Some(_) => DirectXPixelFormat::R16G16B16A16Float,
        None => DirectXPixelFormat::B8G8R8A8UIntNormalized,
    };
    let pool =
        Direct3D11CaptureFramePool::CreateFreeThreaded(&device.winrt, format, FRAME_POOL_SIZE, size)
            .map_err(|error| format!("Failed to create the capture frame pool: {error}"))?;
    let session = pool
        .CreateCaptureSession(item)
        .map_err(|error| format!("Failed to create the capture session: {error}"))?;
    let _ = session.SetIsCursorCaptureEnabled(false);
    let _ = session.SetIsBorderRequired(false);

    let (sender, receiver) = channel();
    let handler = TypedEventHandler::<Direct3D11CaptureFramePool, IInspectable>::new(
        move |source, _| {
            let Some(source) = source.as_ref() else {
                return Ok(());
            };
            if let Ok(frame) = source.TryGetNextFrame() {
                let _ = sender.send(frame);
            }
            Ok(())
        },
    );
    let token = pool
        .FrameArrived(&handler)
        .map_err(|error| format!("Failed to subscribe to captured frames: {error}"))?;

    let captured = session
        .StartCapture()
        .map_err(|error| format!("Failed to start the capture: {error}"))
        .and_then(|_| {
            receiver
                .recv_timeout(FRAME_TIMEOUT)
                .map_err(|_| "Timed out while waiting for the screen contents".to_string())
        })
        .and_then(|frame| read_pixels(device, &frame, white_scale));

    let _ = pool.RemoveFrameArrived(token);
    let _ = session.Close();
    let _ = pool.Close();

    let (width, height, pixels) = captured?;
    Ok(DesktopFrame {
        bounds,
        width,
        height,
        pixels,
    })
}

fn read_pixels(
    device: &CaptureDevice,
    frame: &Direct3D11CaptureFrame,
    white_scale: Option<f32>,
) -> Result<(u32, u32, Vec<u8>), String> {
    let surface = frame
        .Surface()
        .map_err(|error| format!("Failed to access the captured surface: {error}"))?;
    let access: IDirect3DDxgiInterfaceAccess = surface
        .cast()
        .map_err(|error| format!("Failed to access the captured surface: {error}"))?;
    let source: ID3D11Texture2D = unsafe { access.GetInterface() }
        .map_err(|error| format!("Failed to access the captured texture: {error}"))?;

    let mut descriptor = D3D11_TEXTURE2D_DESC::default();
    unsafe { source.GetDesc(&mut descriptor) };

    let staging_descriptor = D3D11_TEXTURE2D_DESC {
        Usage: D3D11_USAGE_STAGING,
        BindFlags: 0,
        CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
        MiscFlags: 0,
        ..descriptor
    };
    let mut staging = None;
    unsafe {
        device
            .device
            .CreateTexture2D(&staging_descriptor, None, Some(&mut staging))
            .map_err(|error| format!("Failed to prepare the capture buffer: {error}"))?;
    }
    let staging = staging.ok_or_else(|| "The capture buffer was not created".to_string())?;

    let source_resource: ID3D11Resource = source
        .cast()
        .map_err(|error| format!("Failed to access the captured texture: {error}"))?;
    let target_resource: ID3D11Resource = staging
        .cast()
        .map_err(|error| format!("Failed to access the capture buffer: {error}"))?;
    unsafe {
        device
            .context
            .CopyResource(&target_resource, &source_resource);
    }

    let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
    unsafe {
        device
            .context
            .Map(&target_resource, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|error| format!("Failed to read the capture buffer: {error}"))?;
    }

    let pixels = convert_pixels(
        &mapped,
        descriptor.Width as usize,
        descriptor.Height as usize,
        white_scale,
    );

    unsafe {
        device.context.Unmap(&target_resource, 0);
    }

    pixels.map(|pixels| (descriptor.Width, descriptor.Height, pixels))
}

fn convert_pixels(
    mapped: &D3D11_MAPPED_SUBRESOURCE,
    width: usize,
    height: usize,
    white_scale: Option<f32>,
) -> Result<Vec<u8>, String> {
    let stride = width * BYTES_PER_PIXEL;
    let pitch = mapped.RowPitch as usize;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(stride * height)
        .map_err(|_| "Not enough memory to read the screen contents".to_string())?;
    pixels.resize(stride * height, 0);

    let source = unsafe { std::slice::from_raw_parts(mapped.pData as *const u8, pitch * height) };

    let Some(white_scale) = white_scale else {
        for row in 0..height {
            let source_row = &source[row * pitch..row * pitch + stride];
            let target_row = &mut pixels[row * stride..(row + 1) * stride];
            target_row.copy_from_slice(source_row);
            for pixel in target_row.chunks_exact_mut(BYTES_PER_PIXEL) {
                pixel[3] = u8::MAX;
            }
        }
        return Ok(pixels);
    };

    let mapper = ToneMapper::new(white_scale);
    for row in 0..height {
        let source_row = &source[row * pitch..row * pitch + width * HDR_BYTES_PER_PIXEL];
        let target_row = &mut pixels[row * stride..(row + 1) * stride];

        for (channels, pixel) in source_row
            .chunks_exact(HDR_BYTES_PER_PIXEL)
            .zip(target_row.chunks_exact_mut(BYTES_PER_PIXEL))
        {
            let [red, green, blue] = mapper.map(
                half_to_f32(u16::from_le_bytes([channels[0], channels[1]])),
                half_to_f32(u16::from_le_bytes([channels[2], channels[3]])),
                half_to_f32(u16::from_le_bytes([channels[4], channels[5]])),
            );
            pixel[0] = blue;
            pixel[1] = green;
            pixel[2] = red;
            pixel[3] = u8::MAX;
        }
    }

    Ok(pixels)
}

fn half_to_f32(bits: u16) -> f32 {
    let sign = ((bits >> 15) as u32) << 31;
    let exponent = ((bits >> 10) & 0x1f) as u32;
    let mantissa = (bits & 0x3ff) as u32;

    let value = match exponent {
        0 if mantissa == 0 => 0,
        0 => {
            let mut exponent = 127 - 15 + 1;
            let mut mantissa = mantissa;
            while mantissa & 0x400 == 0 {
                mantissa <<= 1;
                exponent -= 1;
            }
            (exponent << 23) | ((mantissa & 0x3ff) << 13)
        }
        0x1f => 0x7f80_0000 | (mantissa << 13),
        _ => ((exponent + 112) << 23) | (mantissa << 13),
    };

    f32::from_bits(sign | value)
}

fn crop(frame: &DesktopFrame, bounds: RECT) -> Option<DesktopFrame> {
    let source_width = rect_width(&frame.bounds);
    let source_height = rect_height(&frame.bounds);
    if source_width <= 0 || source_height <= 0 {
        return None;
    }

    let scale_x = frame.width as f64 / source_width as f64;
    let scale_y = frame.height as f64 / source_height as f64;
    let left = scaled(bounds.left - frame.bounds.left, scale_x).clamp(0, frame.width as i32);
    let top = scaled(bounds.top - frame.bounds.top, scale_y).clamp(0, frame.height as i32);
    let right = scaled(bounds.right - frame.bounds.left, scale_x).clamp(0, frame.width as i32);
    let bottom = scaled(bounds.bottom - frame.bounds.top, scale_y).clamp(0, frame.height as i32);

    let width = (right - left) as usize;
    let height = (bottom - top) as usize;
    if width == 0 || height == 0 {
        return None;
    }

    let stride = width * BYTES_PER_PIXEL;
    let source_stride = frame.width as usize * BYTES_PER_PIXEL;
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(stride * height).ok()?;

    for row in 0..height {
        let offset = (top as usize + row) * source_stride + left as usize * BYTES_PER_PIXEL;
        pixels.extend_from_slice(&frame.pixels[offset..offset + stride]);
    }

    Some(DesktopFrame {
        bounds,
        width: width as u32,
        height: height as u32,
        pixels,
    })
}

fn scaled(value: i32, scale: f64) -> i32 {
    (value as f64 * scale).round() as i32
}

fn center_of(rect: RECT) -> POINT {
    POINT {
        x: (rect.left + rect.right) / 2,
        y: (rect.top + rect.bottom) / 2,
    }
}

fn contains(rect: &RECT, point: POINT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

fn monitor_for_rect(bounds: RECT) -> Option<MonitorEntry> {
    let handle = unsafe { MonitorFromPoint(center_of(bounds), MONITOR_DEFAULTTONEAREST) };
    monitors()
        .into_iter()
        .find(|entry| entry.handle == handle.0 as isize)
}

fn create_d3d_device(
    driver: D3D_DRIVER_TYPE,
) -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
    let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
    let mut device = None;
    let mut context = None;

    unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            driver,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|error| format!("Failed to create the graphics device: {error}"))?;
    }

    let device = device.ok_or_else(|| "The graphics device was not created".to_string())?;
    let context = context.ok_or_else(|| "The graphics context was not created".to_string())?;
    Ok((device, context))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: u32, height: u32, fill: u8) -> DesktopFrame {
        DesktopFrame {
            bounds: RECT {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            },
            width,
            height,
            pixels: vec![fill; (width * height) as usize * BYTES_PER_PIXEL],
        }
    }

    #[test]
    fn converts_half_floats() {
        assert_eq!(half_to_f32(0x0000), 0.0);
        assert_eq!(half_to_f32(0x3c00), 1.0);
        assert_eq!(half_to_f32(0x4000), 2.0);
        assert_eq!(half_to_f32(0xbc00), -1.0);
    }

    #[test]
    fn crops_inside_the_frame() {
        let source = frame(10, 10, 7);
        let cropped = crop(
            &source,
            RECT {
                left: 2,
                top: 3,
                right: 6,
                bottom: 9,
            },
        )
        .expect("crop");

        assert_eq!(cropped.width, 4);
        assert_eq!(cropped.height, 6);
        assert_eq!(cropped.pixels.len(), 4 * 6 * BYTES_PER_PIXEL);
    }

    #[test]
    fn clamps_crops_that_leave_the_frame() {
        let source = frame(10, 10, 0);
        let cropped = crop(
            &source,
            RECT {
                left: 8,
                top: 8,
                right: 20,
                bottom: 20,
            },
        )
        .expect("crop");

        assert_eq!(cropped.width, 2);
        assert_eq!(cropped.height, 2);
    }

    #[test]
    fn rejects_empty_crops() {
        let source = frame(10, 10, 0);
        assert!(crop(
            &source,
            RECT {
                left: 12,
                top: 12,
                right: 16,
                bottom: 16,
            }
        )
        .is_none());
    }
}

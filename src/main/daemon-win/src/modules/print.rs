use crate::com::retain_process_mta;
use crate::overlay::bitmap_info;
use crate::protocol::{Request, params, respond_error, respond_success};
use crate::router::{Module, Reply, method_not_found};
use base64::engine::general_purpose::STANDARD;
use base64::{Engine, decoded_len_estimate};
use poratake_daemon_common::contract::{
    PNG_SIGNATURE, PRINT_MODULE, PrintImageRequest, PrintMethod,
};
use serde_json::json;
use std::mem::size_of;
use std::ptr::null;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{GlobalFree, HGLOBAL};
use windows::Win32::Graphics::Gdi::{
    DIB_RGB_COLORS, DeleteDC, GDI_ERROR, GetDeviceCaps, HALFTONE, HDC, HORZRES, LOGPIXELSX,
    LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH, SRCCOPY,
    SetBrushOrgEx, SetStretchBltMode, StretchDIBits, VERTRES,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory, IWICPalette,
    WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnLoad,
};
use windows::Win32::Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::UI::Controls::Dialogs::{
    PD_NOPAGENUMS, PD_NOSELECTION, PD_RESULT_PRINT, PD_RETURNDC, PD_USEDEVMODECOPIESANDCOLLATE,
    PRINTDLGEXW, PrintDlgExW, START_PAGE_GENERAL,
};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::core::PCWSTR;

const DEFAULT_IMAGE_DPI: f64 = 96.0;
const MIN_IMAGE_DPI: f64 = 1.0;
const MAX_IMAGE_DPI: f64 = 1200.0;
static PRINT_ACTIVE: AtomicBool = AtomicBool::new(false);

struct PrintGuard;

impl PrintGuard {
    fn acquire() -> Option<Self> {
        PRINT_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for PrintGuard {
    fn drop(&mut self) {
        PRINT_ACTIVE.store(false, Ordering::Release);
    }
}

pub struct PrintModule;

impl Module for PrintModule {
    fn name(&self) -> &'static str {
        PRINT_MODULE
    }

    fn handle(&mut self, request: &Request) -> Reply {
        match PrintMethod::parse(&request.method) {
            Some(PrintMethod::Image) => self.print_image(request),
            None => method_not_found(&request.method),
        }
    }
}

impl PrintModule {
    fn print_image(&self, request: &Request) -> Reply {
        let params: PrintImageRequest = match params(request) {
            Ok(params) => params,
            Err(error) => return Reply::Now(Err(error)),
        };

        let mut image_data = Vec::new();
        if image_data
            .try_reserve_exact(decoded_len_estimate(params.image_base64.len()))
            .is_err()
            || STANDARD
                .decode_vec(params.image_base64, &mut image_data)
                .is_err()
        {
            return Reply::Now(Err((
                "INVALID_IMAGE".to_string(),
                "Failed to decode image data".to_string(),
            )));
        };

        if !image_data.starts_with(PNG_SIGNATURE) {
            return Reply::Now(Err((
                "INVALID_IMAGE".to_string(),
                "Failed to decode image data".to_string(),
            )));
        }

        let Some(guard) = PrintGuard::acquire() else {
            return Reply::Now(Err((
                "PRINT_IN_PROGRESS".to_string(),
                "Another print dialog is already open".to_string(),
            )));
        };
        let request_id = request.id.clone();
        let worker = std::thread::Builder::new()
            .name("poratake-print".to_string())
            .spawn(move || {
                let _guard = guard;
                print_png(&image_data, &request_id);
            });
        if worker.is_err() {
            return Reply::Now(Err((
                "PRINT_FAILED".to_string(),
                "Failed to start printing".to_string(),
            )));
        }

        Reply::Deferred
    }
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self, String> {
        retain_process_mta()
            .map_err(|error| format!("Failed to retain the process COM apartment: {error}"))?;
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|error| format!("Failed to initialize printing: {error}"))?;
        }
        Ok(ComApartment)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

struct DecodedImage {
    width: u32,
    height: u32,
    dpi_x: f64,
    dpi_y: f64,
    pixels: Vec<u8>,
}

fn print_png(image_data: &[u8], request_id: &str) {
    let _apartment = match ComApartment::initialize() {
        Ok(apartment) => apartment,
        Err(message) => {
            respond_error(request_id, "PRINT_FAILED", &message);
            return;
        }
    };
    let Ok(image) = decode_png(image_data) else {
        respond_error(request_id, "INVALID_IMAGE", "Failed to decode image data");
        return;
    };

    respond_success(request_id, json!({ "success": true }));

    let Ok(Some(printer)) = show_print_dialog() else {
        return;
    };
    let _ = print_document(&printer, &image);
}

fn decode_png(image_data: &[u8]) -> Result<DecodedImage, String> {
    let factory: IWICImagingFactory = unsafe {
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
            .map_err(|_| "Failed to decode image data".to_string())?
    };
    let stream = unsafe {
        factory
            .CreateStream()
            .map_err(|_| "Failed to decode image data".to_string())?
    };
    unsafe {
        stream
            .InitializeFromMemory(image_data)
            .map_err(|_| "Failed to decode image data".to_string())?;
    }

    let decoder = unsafe {
        factory
            .CreateDecoderFromStream(&stream, null(), WICDecodeMetadataCacheOnLoad)
            .map_err(|_| "Failed to decode image data".to_string())?
    };
    let frame = unsafe {
        decoder
            .GetFrame(0)
            .map_err(|_| "Failed to decode image data".to_string())?
    };

    let mut width = 0;
    let mut height = 0;
    unsafe {
        frame
            .GetSize(&mut width, &mut height)
            .map_err(|_| "Failed to decode image data".to_string())?;
    }
    if width == 0 || height == 0 {
        return Err("Failed to decode image data".to_string());
    }

    let stride = width
        .checked_mul(4)
        .ok_or_else(|| "Failed to decode image data".to_string())?;
    let buffer_size = stride
        .checked_mul(height)
        .ok_or_else(|| "Failed to decode image data".to_string())?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(buffer_size as usize)
        .map_err(|_| "Failed to decode image data".to_string())?;
    pixels.resize(buffer_size as usize, 0);

    let converter = unsafe {
        factory
            .CreateFormatConverter()
            .map_err(|_| "Failed to decode image data".to_string())?
    };
    unsafe {
        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppBGRA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|_| "Failed to decode image data".to_string())?;
        converter
            .CopyPixels(null(), stride, &mut pixels)
            .map_err(|_| "Failed to decode image data".to_string())?;
    }

    composite_onto_white(&mut pixels);

    let mut dpi_x = DEFAULT_IMAGE_DPI;
    let mut dpi_y = DEFAULT_IMAGE_DPI;
    if unsafe { frame.GetResolution(&mut dpi_x, &mut dpi_y) }.is_err() {
        dpi_x = DEFAULT_IMAGE_DPI;
        dpi_y = DEFAULT_IMAGE_DPI;
    }

    Ok(DecodedImage {
        width,
        height,
        dpi_x: validated_dpi(dpi_x),
        dpi_y: validated_dpi(dpi_y),
        pixels,
    })
}

fn validated_dpi(dpi: f64) -> f64 {
    if dpi.is_finite() && (MIN_IMAGE_DPI..=MAX_IMAGE_DPI).contains(&dpi) {
        return dpi;
    }
    DEFAULT_IMAGE_DPI
}

fn composite_onto_white(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = pixel[3] as u32;
        let inverse_alpha = 255 - alpha;
        for channel in &mut pixel[..3] {
            *channel = (((*channel as u32 * alpha) + (255 * inverse_alpha) + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }
}

struct PrinterContext {
    dc: HDC,
    dev_mode: HGLOBAL,
    dev_names: HGLOBAL,
}

impl Drop for PrinterContext {
    fn drop(&mut self) {
        unsafe {
            if !self.dc.is_invalid() {
                let _ = DeleteDC(self.dc);
            }
            if !self.dev_mode.is_invalid() {
                let _ = GlobalFree(Some(self.dev_mode));
            }
            if !self.dev_names.is_invalid() {
                let _ = GlobalFree(Some(self.dev_names));
            }
        }
    }
}

fn show_print_dialog() -> Result<Option<PrinterContext>, String> {
    let mut dialog = PRINTDLGEXW {
        lStructSize: size_of::<PRINTDLGEXW>() as u32,
        hwndOwner: unsafe { GetForegroundWindow() },
        Flags: PD_RETURNDC | PD_USEDEVMODECOPIESANDCOLLATE | PD_NOPAGENUMS | PD_NOSELECTION,
        nStartPage: START_PAGE_GENERAL,
        ..Default::default()
    };

    let result = unsafe { PrintDlgExW(&mut dialog) };
    let printer = PrinterContext {
        dc: dialog.hDC,
        dev_mode: dialog.hDevMode,
        dev_names: dialog.hDevNames,
    };

    result.map_err(|error| format!("Failed to show print dialog: {error}"))?;
    if dialog.dwResultAction != PD_RESULT_PRINT {
        return Ok(None);
    }
    if printer.dc.is_invalid() {
        return Err("Failed to create printer device context".to_string());
    }

    Ok(Some(printer))
}

struct PrintableArea {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    dpi_x: i32,
    dpi_y: i32,
}

#[derive(Debug, PartialEq)]
struct PageSlice {
    destination_height: i32,
    source_top: i32,
    source_height: i32,
}

fn page_slice(page: u64, total_height: u64, page_height: u64, source_height: u32) -> PageSlice {
    let destination_offset = page * page_height;
    let next_offset = ((page + 1) * page_height).min(total_height);
    let source_top = (((destination_offset * source_height as u64 + total_height / 2)
        / total_height)
        .min(source_height as u64 - 1)) as i32;
    let source_bottom = ((next_offset * source_height as u64 + total_height / 2) / total_height)
        .max(source_top as u64 + 1)
        .min(source_height as u64) as i32;

    PageSlice {
        destination_height: (next_offset - destination_offset) as i32,
        source_top,
        source_height: source_bottom - source_top,
    }
}

fn printable_area(dc: HDC) -> Result<PrintableArea, String> {
    let width = unsafe { GetDeviceCaps(Some(dc), HORZRES) };
    let height = unsafe { GetDeviceCaps(Some(dc), VERTRES) };
    let dpi_x = unsafe { GetDeviceCaps(Some(dc), LOGPIXELSX) };
    let dpi_y = unsafe { GetDeviceCaps(Some(dc), LOGPIXELSY) };
    if width <= 0 || height <= 0 || dpi_x <= 0 || dpi_y <= 0 {
        return Err("Printer reported an invalid printable area".to_string());
    }

    let physical_width = unsafe { GetDeviceCaps(Some(dc), PHYSICALWIDTH) };
    let physical_height = unsafe { GetDeviceCaps(Some(dc), PHYSICALHEIGHT) };
    let offset_x = unsafe { GetDeviceCaps(Some(dc), PHYSICALOFFSETX) }.max(0);
    let offset_y = unsafe { GetDeviceCaps(Some(dc), PHYSICALOFFSETY) }.max(0);
    let margin_x = dpi_x / 2;
    let margin_y = dpi_y / 2;

    let left = (margin_x - offset_x).max(0);
    let top = (margin_y - offset_y).max(0);
    let right = if physical_width > 0 {
        (physical_width - margin_x - offset_x).min(width)
    } else {
        width - margin_x
    };
    let bottom = if physical_height > 0 {
        (physical_height - margin_y - offset_y).min(height)
    } else {
        height - margin_y
    };

    if right <= left || bottom <= top {
        return Ok(PrintableArea {
            x: 0,
            y: 0,
            width,
            height,
            dpi_x,
            dpi_y,
        });
    }

    Ok(PrintableArea {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
        dpi_x,
        dpi_y,
    })
}

fn print_document(printer: &PrinterContext, image: &DecodedImage) -> Result<(), String> {
    let area = printable_area(printer.dc)?;
    let natural_width = image.width as f64 * area.dpi_x as f64 / image.dpi_x;
    let natural_height = image.height as f64 * area.dpi_y as f64 / image.dpi_y;
    let scale = (area.width as f64 / natural_width).min(1.0);
    let destination_width = (natural_width * scale).round().max(1.0) as i32;
    let destination_height = natural_height * scale;
    if !destination_height.is_finite() || destination_height > i32::MAX as f64 {
        return Err("Image is too large to print".to_string());
    }
    let destination_height = destination_height.round().max(1.0) as i32;

    let bitmap_info = bitmap_info(
        image.width as i32,
        image.height as i32,
        image.pixels.len() as u32,
    );

    let title: Vec<u16> = "Poratake Screenshot"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let document_info = DOCINFOW {
        cbSize: size_of::<DOCINFOW>() as i32,
        lpszDocName: PCWSTR(title.as_ptr()),
        ..Default::default()
    };

    if unsafe { StartDocW(printer.dc, &document_info) } <= 0 {
        return Err("Failed to start print job".to_string());
    }

    if unsafe { SetStretchBltMode(printer.dc, HALFTONE) } == 0 {
        unsafe {
            AbortDoc(printer.dc);
        }
        return Err("Failed to configure printer scaling".to_string());
    }
    if !unsafe { SetBrushOrgEx(printer.dc, 0, 0, None) }.as_bool() {
        unsafe {
            AbortDoc(printer.dc);
        }
        return Err("Failed to configure printer scaling".to_string());
    }

    let total_height = destination_height as u64;
    let page_height = area.height as u64;
    let page_count = total_height.div_ceil(page_height);
    let destination_x = area.x + (area.width - destination_width) / 2;

    for page in 0..page_count {
        let slice = page_slice(page, total_height, page_height, image.height);

        if unsafe { StartPage(printer.dc) } <= 0 {
            unsafe {
                AbortDoc(printer.dc);
            }
            return Err("Failed to start printer page".to_string());
        }

        let copied = unsafe {
            StretchDIBits(
                printer.dc,
                destination_x,
                area.y,
                destination_width,
                slice.destination_height,
                0,
                slice.source_top,
                image.width as i32,
                slice.source_height,
                Some(image.pixels.as_ptr().cast()),
                &bitmap_info,
                DIB_RGB_COLORS,
                SRCCOPY,
            )
        };
        if copied == 0 || copied == GDI_ERROR {
            unsafe {
                AbortDoc(printer.dc);
            }
            return Err("Failed to render image to printer".to_string());
        }

        if unsafe { EndPage(printer.dc) } <= 0 {
            unsafe {
                AbortDoc(printer.dc);
            }
            return Err("Failed to finish printer page".to_string());
        }
    }

    if unsafe { EndDoc(printer.dc) } <= 0 {
        unsafe {
            AbortDoc(printer.dc);
        }
        return Err("Failed to finish print job".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::BI_RGB;

    #[test]
    fn composites_straight_alpha_onto_white() {
        let mut pixels = [0, 0, 0, 0, 0, 0, 255, 128, 10, 20, 30, 255];

        composite_onto_white(&mut pixels);

        assert_eq!(
            pixels,
            [255, 255, 255, 255, 127, 127, 255, 255, 10, 20, 30, 255]
        );
    }

    #[test]
    fn creates_top_down_bgra_bitmap_info() {
        let info = bitmap_info(10, 20, 800);

        assert_eq!(info.bmiHeader.biWidth, 10);
        assert_eq!(info.bmiHeader.biHeight, -20);
        assert_eq!(info.bmiHeader.biPlanes, 1);
        assert_eq!(info.bmiHeader.biBitCount, 32);
        assert_eq!(info.bmiHeader.biCompression, BI_RGB.0);
        assert_eq!(info.bmiHeader.biSizeImage, 800);
    }

    #[test]
    fn slices_tall_images_into_contiguous_vertical_pages() {
        let slices: Vec<PageSlice> = (0..4)
            .map(|page| page_slice(page, 10_000, 3_000, 2_000))
            .collect();

        assert_eq!(
            slices,
            [
                PageSlice {
                    destination_height: 3_000,
                    source_top: 0,
                    source_height: 600,
                },
                PageSlice {
                    destination_height: 3_000,
                    source_top: 600,
                    source_height: 600,
                },
                PageSlice {
                    destination_height: 3_000,
                    source_top: 1_200,
                    source_height: 600,
                },
                PageSlice {
                    destination_height: 1_000,
                    source_top: 1_800,
                    source_height: 200,
                },
            ]
        );
    }

    #[test]
    fn print_guard_allows_only_one_active_dialog() {
        let first = PrintGuard::acquire().expect("first print guard");
        assert!(PrintGuard::acquire().is_none());
        drop(first);
        assert!(PrintGuard::acquire().is_some());
    }
}

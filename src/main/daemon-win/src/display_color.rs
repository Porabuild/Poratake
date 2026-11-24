use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes,
    QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::ERROR_SUCCESS;

const SCRGB_WHITE_NITS: f32 = 80.0;
const SDR_WHITE_LEVEL_UNIT: f32 = 1000.0;
const ADVANCED_COLOR_ENABLED: u32 = 0x2;
pub(crate) const TRANSFER_SIZE: usize = 256;
pub(crate) const TRANSFER_LEVELS: usize = 4096;
const LUMA_RED: f32 = 0.2126;
const LUMA_GREEN: f32 = 0.7152;
const LUMA_BLUE: f32 = 0.0722;

#[derive(PartialEq)]
pub struct DisplayTransfer {
    white: f32,
    inverse: [u8; TRANSFER_LEVELS],
}

pub(crate) fn capture_pixel_format(transfer: Option<&DisplayTransfer>) -> DirectXPixelFormat {
    match transfer {
        Some(_) => DirectXPixelFormat::R16G16B16A16Float,
        None => DirectXPixelFormat::B8G8R8A8UIntNormalized,
    }
}

impl DisplayTransfer {
    pub fn from_reported_white(white_scale: f32) -> Self {
        let mut encoded = [0u16; TRANSFER_SIZE];
        for (code, level) in encoded.iter_mut().enumerate() {
            let value = code as f32 / (TRANSFER_SIZE - 1) as f32;
            let linear = srgb_decode(value);
            *level = transfer_index(linear);
        }

        DisplayTransfer {
            white: white_scale.max(1.0),
            inverse: invert(&encoded),
        }
    }

    pub fn from_probe(samples: &[f32; TRANSFER_SIZE]) -> Option<Self> {
        let white = samples[TRANSFER_SIZE - 1];
        if !white.is_finite() || white <= 0.0 {
            return None;
        }

        let mut encoded = [0u16; TRANSFER_SIZE];
        let mut previous = 0.0f32;

        for (code, sample) in samples.iter().enumerate() {
            let normalized = (sample / white).clamp(0.0, 1.0).max(previous);
            previous = normalized;
            encoded[code] = transfer_index(normalized);
        }

        if encoded[0] != 0 || encoded[TRANSFER_SIZE - 1] == 0 {
            return None;
        }

        Some(DisplayTransfer {
            white,
            inverse: invert(&encoded),
        })
    }

    pub fn white(&self) -> f32 {
        self.white
    }

    pub fn inverse(&self) -> &[u8; TRANSFER_LEVELS] {
        &self.inverse
    }

    pub fn with_scaled_white(&self, ratio: f32) -> Option<Self> {
        if !ratio.is_finite() || ratio <= 0.0 {
            return None;
        }

        let white = self.white * ratio;
        if !white.is_finite() || white <= 0.0 {
            return None;
        }

        Some(DisplayTransfer {
            white: white.max(1.0),
            inverse: self.inverse,
        })
    }

    pub fn encode(&self, red: f32, green: f32, blue: f32) -> [u8; 3] {
        let scale = 1.0 / self.white;
        let mut color = [
            (red * scale).max(0.0),
            (green * scale).max(0.0),
            (blue * scale).max(0.0),
        ];
        let luminance = LUMA_RED * color[0] + LUMA_GREEN * color[1] + LUMA_BLUE * color[2];

        if luminance > 1.0 {
            let gain = 1.0 / luminance;
            color[0] *= gain;
            color[1] *= gain;
            color[2] *= gain;
        }

        [
            self.lookup(color[0]),
            self.lookup(color[1]),
            self.lookup(color[2]),
        ]
    }

    fn lookup(&self, value: f32) -> u8 {
        self.inverse[transfer_index(value) as usize]
    }
}

fn invert(encoded: &[u16; TRANSFER_SIZE]) -> [u8; TRANSFER_LEVELS] {
    let mut inverse = [0u8; TRANSFER_LEVELS];
    let mut code = 0usize;

    for (level, slot) in inverse.iter_mut().enumerate() {
        while code + 1 < TRANSFER_SIZE && (encoded[code + 1] as usize) < level {
            code += 1;
        }

        let current = encoded[code] as usize;
        if current >= level || code + 1 >= TRANSFER_SIZE {
            *slot = code as u8;
            continue;
        }

        let next = encoded[code + 1] as usize;
        let span = next - current;
        let offset = usize::from((level - current) * 2 >= span);
        *slot = (code + offset).min(TRANSFER_SIZE - 1) as u8;
    }

    inverse
}

fn transfer_index(value: f32) -> u16 {
    let root = value.clamp(0.0, 1.0).sqrt();
    (root * (TRANSFER_LEVELS - 1) as f32 + 0.5).clamp(0.0, (TRANSFER_LEVELS - 1) as f32) as u16
}

fn srgb_decode(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub fn hdr_white_scale(device_name: &str) -> Option<f32> {
    let paths = display_paths()?;

    for path in paths {
        if !source_matches(&path, device_name) {
            continue;
        }
        if !advanced_color_enabled(&path) {
            return None;
        }

        let nits = sdr_white_level(&path)?;
        let scale = nits / SCRGB_WHITE_NITS;
        return (scale > 1.0).then_some(scale);
    }

    None
}

fn display_paths() -> Option<Vec<DISPLAYCONFIG_PATH_INFO>> {
    let mut path_count = 0;
    let mut mode_count = 0;
    let sizes = unsafe {
        GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
    };
    if sizes != ERROR_SUCCESS {
        return None;
    }

    let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
    let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
    let query = unsafe {
        QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        )
    };
    if query != ERROR_SUCCESS {
        return None;
    }

    paths.truncate(path_count as usize);
    Some(paths)
}

fn source_matches(path: &DISPLAYCONFIG_PATH_INFO, device_name: &str) -> bool {
    let mut request = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
            adapterId: path.sourceInfo.adapterId,
            id: path.sourceInfo.id,
        },
        ..Default::default()
    };

    if unsafe { DisplayConfigGetDeviceInfo(&mut request.header) } != ERROR_SUCCESS.0 as i32 {
        return false;
    }

    String::from_utf16_lossy(&request.viewGdiDeviceName)
        .trim_end_matches('\0')
        .eq_ignore_ascii_case(device_name)
}

fn advanced_color_enabled(path: &DISPLAYCONFIG_PATH_INFO) -> bool {
    let mut request = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
            size: std::mem::size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32,
            adapterId: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        },
        ..Default::default()
    };

    if unsafe { DisplayConfigGetDeviceInfo(&mut request.header) } != ERROR_SUCCESS.0 as i32 {
        return false;
    }

    unsafe { request.Anonymous.value & ADVANCED_COLOR_ENABLED != 0 }
}

fn sdr_white_level(path: &DISPLAYCONFIG_PATH_INFO) -> Option<f32> {
    let mut request = DISPLAYCONFIG_SDR_WHITE_LEVEL {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
            size: std::mem::size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>() as u32,
            adapterId: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        },
        ..Default::default()
    };

    if unsafe { DisplayConfigGetDeviceInfo(&mut request.header) } != ERROR_SUCCESS.0 as i32 {
        return None;
    }

    let nits = request.SDRWhiteLevel as f32 / SDR_WHITE_LEVEL_UNIT * SCRGB_WHITE_NITS;
    (nits > 0.0).then_some(nits)
}

#[cfg(test)]
pub(crate) fn probe_samples(gamma: f32, white: f32) -> [f32; TRANSFER_SIZE] {
    let mut samples = [0f32; TRANSFER_SIZE];
    for (code, sample) in samples.iter_mut().enumerate() {
        *sample = (code as f32 / (TRANSFER_SIZE - 1) as f32).powf(gamma) * white;
    }
    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_sdr_white_to_pure_white() {
        let transfer = DisplayTransfer::from_reported_white(2.5);
        assert_eq!(transfer.encode(2.5, 2.5, 2.5), [255, 255, 255]);
    }

    #[test]
    fn maps_midtones_to_their_srgb_value() {
        let transfer = DisplayTransfer::from_reported_white(2.5);
        let midtone = transfer.encode(0.5358, 0.5358, 0.5358);
        assert!(midtone[0].abs_diff(128) <= 2);
    }

    #[test]
    fn keeps_black_at_black() {
        let transfer = DisplayTransfer::from_reported_white(2.5);
        assert_eq!(transfer.encode(0.0, -0.01, 0.0), [0, 0, 0]);
    }

    #[test]
    fn clamps_content_brighter_than_sdr_white() {
        let transfer = DisplayTransfer::from_reported_white(2.5);
        assert_eq!(transfer.encode(12.0, 12.0, 12.0), [255, 255, 255]);
    }

    #[test]
    fn keeps_the_hue_of_highlights_instead_of_clipping_channels() {
        let transfer = DisplayTransfer::from_reported_white(1.0);
        let highlight = transfer.encode(6.0, 3.0, 0.6);

        assert!(highlight[0] > highlight[1]);
        assert!(highlight[1] > highlight[2]);
    }

    #[test]
    fn leaves_the_whole_sdr_range_untouched() {
        let transfer = DisplayTransfer::from_reported_white(1.0);

        for step in 0..=10 {
            let value = step as f32 / 10.0;
            let direct = transfer.encode(value, value, value);
            assert_eq!(direct[0], direct[1]);
            assert_eq!(direct[1], direct[2]);
        }

        assert_eq!(transfer.encode(1.0, 1.0, 1.0), [255, 255, 255]);
        assert_eq!(transfer.encode(0.2159, 0.2159, 0.2159)[0], 128);
    }

    #[test]
    fn never_scales_below_sdr_white() {
        let transfer = DisplayTransfer::from_reported_white(0.4);
        assert_eq!(transfer.encode(1.0, 1.0, 1.0), [255, 255, 255]);
    }

    fn assert_recovers_codes(transfer: &DisplayTransfer, samples: &[f32; TRANSFER_SIZE]) {
        for (code, sample) in samples.iter().enumerate() {
            let mapped = transfer.encode(*sample, *sample, *sample);
            assert!(
                mapped[0].abs_diff(code as u8) <= 1,
                "code {code} mapped to {mapped:?}"
            );
        }
    }

    #[test]
    fn recovers_srgb_codes_from_an_srgb_probe() {
        let mut samples = [0f32; TRANSFER_SIZE];
        for (code, sample) in samples.iter_mut().enumerate() {
            let value = code as f32 / (TRANSFER_SIZE - 1) as f32;
            *sample = srgb_decode(value) * 6.0;
        }

        let transfer = DisplayTransfer::from_probe(&samples).expect("srgb transfer");
        assert_recovers_codes(&transfer, &samples);
    }

    #[test]
    fn recovers_gamma_codes_from_a_patched_probe() {
        let samples = probe_samples(2.4, 6.0);
        let transfer = DisplayTransfer::from_probe(&samples).expect("gamma transfer");
        assert_recovers_codes(&transfer, &samples);
    }

    #[test]
    fn recovers_codes_when_the_display_white_is_boosted() {
        let samples = probe_samples(2.4, 12.0);
        let transfer = DisplayTransfer::from_probe(&samples).expect("boosted transfer");
        assert_eq!(transfer.white(), 12.0);
        assert_recovers_codes(&transfer, &samples);
    }

    #[test]
    fn scaling_the_white_preserves_the_measured_curve() {
        let samples = probe_samples(2.4, 6.0);
        let transfer = DisplayTransfer::from_probe(&samples).expect("gamma transfer");
        let scaled = transfer.with_scaled_white(2.0).expect("scaled transfer");

        assert_eq!(scaled.white(), 12.0);
        assert_recovers_codes(&scaled, &probe_samples(2.4, 12.0));
    }

    #[test]
    fn rejects_an_unusable_white_scale() {
        let transfer = DisplayTransfer::from_reported_white(6.0);
        assert!(transfer.with_scaled_white(0.0).is_none());
        assert!(transfer.with_scaled_white(f32::NAN).is_none());
    }

    #[test]
    fn keeps_highlight_hue_after_the_calibrated_lookup() {
        let samples = probe_samples(2.4, 6.0);
        let transfer = DisplayTransfer::from_probe(&samples).expect("gamma transfer");
        let highlight = transfer.encode(30.0, 12.0, 3.0);

        assert!(highlight[0] > highlight[1]);
        assert!(highlight[1] > highlight[2]);
    }

    #[test]
    fn rejects_an_empty_probe() {
        let samples = [0f32; TRANSFER_SIZE];
        assert!(DisplayTransfer::from_probe(&samples).is_none());
    }

    #[test]
    fn rejects_a_probe_with_a_raised_black() {
        let mut samples = probe_samples(2.4, 6.0);
        samples[0] = samples[1];
        assert!(DisplayTransfer::from_probe(&samples).is_none());
    }
}

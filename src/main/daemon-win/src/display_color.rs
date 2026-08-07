use std::sync::OnceLock;
use windows::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO,
    DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO, DISPLAYCONFIG_SDR_WHITE_LEVEL,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
};
use windows::Win32::Foundation::ERROR_SUCCESS;

const SCRGB_WHITE_NITS: f32 = 80.0;
const SDR_WHITE_LEVEL_UNIT: f32 = 1000.0;
const ADVANCED_COLOR_ENABLED: u32 = 0x2;
const ENCODE_TABLE_SIZE: usize = 4096;

pub struct ToneMapper {
    white_scale: f32,
    table: &'static [u8],
}

impl ToneMapper {
    pub fn new(white_scale: f32) -> Self {
        ToneMapper {
            white_scale: white_scale.max(1.0),
            table: encode_table(),
        }
    }

    pub fn map(&self, red: f32, green: f32, blue: f32) -> [u8; 3] {
        let scale = 1.0 / self.white_scale;
        [
            self.encode(red * scale),
            self.encode(green * scale),
            self.encode(blue * scale),
        ]
    }

    fn encode(&self, value: f32) -> u8 {
        let index = (value.clamp(0.0, 1.0) * (ENCODE_TABLE_SIZE - 1) as f32) as usize;
        self.table[index.min(ENCODE_TABLE_SIZE - 1)]
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

fn encode_table() -> &'static [u8] {
    static TABLE: OnceLock<Box<[u8]>> = OnceLock::new();

    TABLE.get_or_init(|| {
        (0..ENCODE_TABLE_SIZE)
            .map(|index| {
                let linear = index as f32 / (ENCODE_TABLE_SIZE - 1) as f32;
                let encoded = if linear <= 0.0031308 {
                    linear * 12.92
                } else {
                    1.055 * linear.powf(1.0 / 2.4) - 0.055
                };
                (encoded * 255.0 + 0.5).clamp(0.0, 255.0) as u8
            })
            .collect()
    })
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
mod tests {
    use super::*;

    #[test]
    fn maps_sdr_white_to_pure_white() {
        let mapper = ToneMapper::new(2.5);
        assert_eq!(mapper.map(2.5, 2.5, 2.5), [255, 255, 255]);
    }

    #[test]
    fn maps_midtones_to_their_srgb_value() {
        let mapper = ToneMapper::new(2.5);
        let midtone = mapper.map(0.5358, 0.5358, 0.5358);
        assert!(midtone[0].abs_diff(128) <= 2);
    }

    #[test]
    fn keeps_black_at_black() {
        let mapper = ToneMapper::new(2.5);
        assert_eq!(mapper.map(0.0, -0.01, 0.0), [0, 0, 0]);
    }

    #[test]
    fn clamps_content_brighter_than_sdr_white() {
        let mapper = ToneMapper::new(2.5);
        assert_eq!(mapper.map(12.0, 12.0, 12.0), [255, 255, 255]);
    }

    #[test]
    fn never_scales_below_sdr_white() {
        let mapper = ToneMapper::new(0.4);
        assert_eq!(mapper.map(1.0, 1.0, 1.0), [255, 255, 255]);
    }
}

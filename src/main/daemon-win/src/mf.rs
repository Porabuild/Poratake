use crate::modules::recorder_types::RecorderError;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFMediaType, MF_MT_AVG_BITRATE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MFCreateAttributes, MFCreateMediaType, MFMediaType_Video, MFVideoInterlace_Progressive,
};

pub fn create_attributes(capacity: u32) -> Result<IMFAttributes, RecorderError> {
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, capacity) }.map_err(|error| {
        RecorderError::capture(format!(
            "Failed to create Media Foundation attributes: {error}"
        ))
    })?;
    attributes.ok_or_else(|| RecorderError::capture("Media Foundation attributes were not created"))
}

pub fn attribute_error(error: windows::core::Error) -> RecorderError {
    RecorderError::capture(format!(
        "Failed to configure Media Foundation media type: {error}"
    ))
}

pub fn pack_ratio(numerator: u32, denominator: u32) -> u64 {
    (u64::from(numerator) << 32) | u64::from(denominator)
}

pub fn create_video_type(
    subtype: windows::core::GUID,
    width: u32,
    height: u32,
    frame_rate_numerator: u32,
    frame_rate_denominator: u32,
    bitrate: Option<u32>,
) -> Result<IMFMediaType, RecorderError> {
    let media_type = unsafe { MFCreateMediaType() }.map_err(|error| {
        RecorderError::capture(format!("Failed to create video media type: {error}"))
    })?;

    unsafe {
        media_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
            .map_err(attribute_error)?;
        media_type
            .SetGUID(&MF_MT_SUBTYPE, &subtype)
            .map_err(attribute_error)?;
        media_type
            .SetUINT64(&MF_MT_FRAME_SIZE, pack_ratio(width, height))
            .map_err(attribute_error)?;
        media_type
            .SetUINT64(
                &MF_MT_FRAME_RATE,
                pack_ratio(frame_rate_numerator, frame_rate_denominator),
            )
            .map_err(attribute_error)?;
        media_type
            .SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, pack_ratio(1, 1))
            .map_err(attribute_error)?;
        media_type
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(attribute_error)?;
        if let Some(bitrate) = bitrate {
            media_type
                .SetUINT32(&MF_MT_AVG_BITRATE, bitrate)
                .map_err(attribute_error)?;
        }
    }

    Ok(media_type)
}

pub fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

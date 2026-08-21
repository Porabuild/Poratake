use std::fmt;
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MFCreateAttributes,
    MFEnumDeviceSources,
};
use windows::Win32::System::Com::CoTaskMemFree;

pub struct CameraDevice {
    pub activation: IMFActivate,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum CameraSelectionError {
    NoCamera,
    AmbiguousName,
    AmbiguousPartialName,
    Unavailable,
}

impl fmt::Display for CameraSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NoCamera => "No camera device found",
            Self::AmbiguousName => "Multiple cameras have the selected name; select a device ID",
            Self::AmbiguousPartialName => "The selected camera name matches multiple devices",
            Self::Unavailable => "The selected camera is no longer available",
        };
        formatter.write_str(message)
    }
}

pub fn enumerate_cameras() -> Result<Vec<CameraDevice>, String> {
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, 1) }.map_err(|error| error.to_string())?;
    let attributes =
        attributes.ok_or_else(|| "Media Foundation attributes unavailable".to_string())?;
    unsafe {
        attributes.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )
    }
    .map_err(|error| error.to_string())?;

    let mut raw = std::ptr::null_mut();
    let mut count = 0;
    unsafe { MFEnumDeviceSources(&attributes, &mut raw, &mut count) }
        .map_err(|error| error.to_string())?;
    if raw.is_null() || count == 0 {
        if !raw.is_null() {
            unsafe {
                CoTaskMemFree(Some(raw.cast()));
            }
        }
        return Ok(Vec::new());
    }

    let entries = unsafe { std::slice::from_raw_parts_mut(raw, count as usize) };
    let mut devices = Vec::with_capacity(count as usize);
    for entry in entries {
        let Some(activation) = (unsafe { std::ptr::read(entry) }) else {
            continue;
        };
        let Some(id) = attribute_string(
            &activation,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
        ) else {
            continue;
        };
        let Some(name) = attribute_string(&activation, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
        else {
            continue;
        };
        if id.is_empty() || name.trim().is_empty() {
            continue;
        }
        devices.push(CameraDevice {
            activation,
            id,
            name,
        });
    }
    unsafe {
        CoTaskMemFree(Some(raw.cast()));
    }
    Ok(devices)
}

pub fn select_camera(
    mut devices: Vec<CameraDevice>,
    requested_id: Option<&str>,
    requested_name: Option<&str>,
) -> Result<CameraDevice, CameraSelectionError> {
    let identities = devices
        .iter()
        .map(|device| (device.id.clone(), device.name.clone()))
        .collect::<Vec<_>>();
    let index = select_camera_index(&identities, requested_id, requested_name)?;
    Ok(devices.swap_remove(index))
}

fn select_camera_index(
    devices: &[(String, String)],
    requested_id: Option<&str>,
    requested_name: Option<&str>,
) -> Result<usize, CameraSelectionError> {
    if devices.is_empty() {
        return Err(CameraSelectionError::NoCamera);
    }
    let requested_id = requested_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let requested_name = requested_name
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(requested_id) = requested_id {
        if let Some(index) = devices
            .iter()
            .position(|(id, _)| id.eq_ignore_ascii_case(requested_id))
        {
            return Ok(index);
        }
    }

    if let Some(requested_name) = requested_name {
        let exact = devices
            .iter()
            .enumerate()
            .filter_map(|(index, (_, name))| {
                name.eq_ignore_ascii_case(requested_name).then_some(index)
            })
            .collect::<Vec<_>>();
        if exact.len() == 1 {
            return Ok(exact[0]);
        }
        if exact.len() > 1 {
            return Err(CameraSelectionError::AmbiguousName);
        }

        let needle = requested_name.to_lowercase();
        let partial = devices
            .iter()
            .enumerate()
            .filter_map(|(index, (_, name))| {
                let name = name.to_lowercase();
                (name.contains(&needle) || needle.contains(&name)).then_some(index)
            })
            .collect::<Vec<_>>();
        if partial.len() == 1 {
            return Ok(partial[0]);
        }
        if partial.len() > 1 {
            return Err(CameraSelectionError::AmbiguousPartialName);
        }
    }

    if requested_id.is_some() || requested_name.is_some() {
        return Err(CameraSelectionError::Unavailable);
    }
    Ok(0)
}

fn attribute_string(activation: &IMFActivate, key: &windows::core::GUID) -> Option<String> {
    let length = unsafe { activation.GetStringLength(key) }.ok()?;
    let mut buffer = vec![0; length as usize + 1];
    unsafe { activation.GetString(key, &mut buffer, None) }.ok()?;
    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cameras() -> Vec<(String, String)> {
        vec![
            ("camera-one".to_string(), "Camera".to_string()),
            ("camera-two".to_string(), "Camera Pro".to_string()),
        ]
    }

    #[test]
    fn defaults_only_when_no_identity_was_requested() {
        assert_eq!(select_camera_index(&cameras(), None, None), Ok(0));
        assert_eq!(
            select_camera_index(&cameras(), Some("missing"), None),
            Err(CameraSelectionError::Unavailable)
        );
    }

    #[test]
    fn device_id_is_case_insensitive_and_wins_over_name() {
        assert_eq!(
            select_camera_index(&cameras(), Some("CAMERA-TWO"), Some("Camera")),
            Ok(1)
        );
    }

    #[test]
    fn unique_exact_and_partial_names_are_supported() {
        assert_eq!(select_camera_index(&cameras(), None, Some("camera")), Ok(0));
        assert_eq!(select_camera_index(&cameras(), None, Some("Pro")), Ok(1));
    }

    #[test]
    fn ambiguous_names_do_not_choose_a_camera() {
        let duplicate_names = vec![
            ("camera-one".to_string(), "Camera".to_string()),
            ("camera-two".to_string(), "Camera".to_string()),
        ];

        assert_eq!(
            select_camera_index(&duplicate_names, None, Some("Camera")),
            Err(CameraSelectionError::AmbiguousName)
        );
        assert_eq!(
            select_camera_index(&cameras(), None, Some("Camera Pro Max")),
            Err(CameraSelectionError::AmbiguousPartialName)
        );
    }
}

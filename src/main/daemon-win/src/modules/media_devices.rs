use std::ffi::c_void;
use windows::core::{Result, PWSTR};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::{
    eCapture, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::Media::MediaFoundation::{
    IMFActivate, MFCreateAttributes, MFEnumDeviceSources, MFShutdown, MFStartup, MFSTARTUP_FULL,
    MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
    MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MF_VERSION,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaDevice {
    pub id: String,
    pub label: String,
}

struct ComApartment;

impl ComApartment {
    fn initialize() -> Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

struct MediaFoundation;

impl MediaFoundation {
    fn initialize() -> Result<Self> {
        unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }?;
        Ok(Self)
    }
}

impl Drop for MediaFoundation {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

pub fn enumerate_microphones() -> Result<Vec<MediaDevice>> {
    let _apartment = ComApartment::initialize()?;
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }?;
    let collection = unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }?;
    let count = unsafe { collection.GetCount() }?;
    let mut devices = Vec::with_capacity(count as usize);

    for index in 0..count {
        let Ok(device) = (unsafe { collection.Item(index) }) else {
            continue;
        };
        let Ok(id) = microphone_id(&device) else {
            continue;
        };
        let Ok(label) = microphone_label(&device) else {
            continue;
        };
        if id.is_empty() || label.trim().is_empty() {
            continue;
        }
        devices.push(MediaDevice { id, label });
    }

    Ok(devices)
}

pub fn enumerate_cameras() -> Result<Vec<MediaDevice>> {
    let _apartment = ComApartment::initialize()?;
    let _media_foundation = MediaFoundation::initialize()?;
    let mut attributes = None;
    unsafe { MFCreateAttributes(&mut attributes, 1) }?;
    let Some(attributes) = attributes else {
        return Ok(Vec::new());
    };
    unsafe {
        attributes.SetGUID(
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
        )
    }?;

    let mut raw = std::ptr::null_mut();
    let mut count = 0;
    unsafe { MFEnumDeviceSources(&attributes, &mut raw, &mut count) }?;
    let activations = take_activations(raw, count);
    let mut devices = Vec::with_capacity(activations.len());

    for activation in &activations {
        let Ok(id) = camera_attribute(
            activation,
            &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK,
        ) else {
            continue;
        };
        let Ok(label) = camera_attribute(activation, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME) else {
            continue;
        };
        if id.is_empty() || label.trim().is_empty() {
            continue;
        }
        devices.push(MediaDevice { id, label });
    }

    Ok(devices)
}

fn microphone_id(device: &IMMDevice) -> Result<String> {
    let value = unsafe { device.GetId() }?;
    allocated_string(value, None)
}

fn microphone_label(device: &IMMDevice) -> Result<String> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ) }?;
    let mut value = unsafe { store.GetValue(&PKEY_Device_FriendlyName) }?;
    let text = unsafe { PropVariantToStringAlloc(&value) };
    unsafe {
        let _ = PropVariantClear(&mut value);
    }
    allocated_string(text?, None)
}

fn camera_attribute(activation: &IMFActivate, key: &windows::core::GUID) -> Result<String> {
    let mut value = PWSTR::null();
    let mut length = 0;
    unsafe { activation.GetAllocatedString(key, &mut value, &mut length) }?;
    allocated_string(value, Some(length))
}

fn allocated_string(value: PWSTR, length: Option<u32>) -> Result<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    let result = match length {
        Some(length) => unsafe {
            String::from_utf16_lossy(std::slice::from_raw_parts(value.0, length as usize))
        },
        None => unsafe { value.to_string().unwrap_or_default() },
    };
    unsafe {
        CoTaskMemFree(Some(value.0.cast::<c_void>()));
    }
    Ok(result)
}

fn take_activations(raw: *mut Option<IMFActivate>, count: u32) -> Vec<IMFActivate> {
    if raw.is_null() {
        return Vec::new();
    }
    let activations = unsafe {
        std::slice::from_raw_parts_mut(raw, count as usize)
            .iter_mut()
            .filter_map(Option::take)
            .collect()
    };
    unsafe {
        CoTaskMemFree(Some(raw.cast::<c_void>()));
    }
    activations
}

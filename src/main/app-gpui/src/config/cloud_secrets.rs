use base64::Engine as _;
use hmac::{Hmac, KeyInit as _, Mac as _};
use sha2::{Digest as _, Sha256};

use crate::config::schema::CloudConfig;

pub const SAFE_STORAGE_PREFIX: &str = "poratake-safe-storage:v1:";
pub const GPUI_SECRET_PREFIX: &str = "poratake-gpui-secret:v1:";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedSecret {
    pub stored_value: String,
    pub revealed_value: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetainedCloudSecrets {
    pub s3_secret_access_key: Option<RetainedSecret>,
    pub rest_headers: Vec<Option<(String, RetainedSecret)>>,
}

#[derive(Clone, Copy, Debug)]
pub enum SecretPolicy {
    System,
    InMemory([u8; 32]),
}

impl SecretPolicy {
    pub fn for_runtime() -> Self {
        if cfg!(test)
            || std::env::var_os(crate::config::store::CONFIG_DIR_ENV)
                .is_some_and(|value| !value.is_empty())
            || std::env::var_os("PORATAKE_DISABLE_OS_KEYCHAIN")
                .is_some_and(|value| !value.is_empty())
        {
            return Self::InMemory(*b"poratake-isolated-profile-secret");
        }
        Self::System
    }
}

pub fn has_unprotected_cloud_secrets(cloud: &CloudConfig) -> bool {
    if !cloud.s3.secret_access_key.is_empty() && !is_protected(&cloud.s3.secret_access_key) {
        return true;
    }
    cloud
        .rest
        .headers
        .iter()
        .any(|header| !header.value.is_empty() && !is_protected(&header.value))
}

fn is_protected(value: &str) -> bool {
    value.starts_with(SAFE_STORAGE_PREFIX) || value.starts_with(GPUI_SECRET_PREFIX)
}

pub fn reveal_cloud_secrets(cloud: &mut CloudConfig, policy: SecretPolicy) -> RetainedCloudSecrets {
    let mut retained = RetainedCloudSecrets::default();
    let (revealed_s3, s3_retained) = reveal_value(&cloud.s3.secret_access_key, policy);
    cloud.s3.secret_access_key = revealed_s3;
    retained.s3_secret_access_key = s3_retained;

    retained.rest_headers = cloud
        .rest
        .headers
        .iter_mut()
        .map(|header| {
            let (revealed, entry) = reveal_value(&header.value, policy);
            header.value = revealed;
            entry.map(|secret| (header.key.clone(), secret))
        })
        .collect();

    retained
}

pub fn protect_cloud_secrets(
    cloud: &mut CloudConfig,
    retained: &mut RetainedCloudSecrets,
    policy: SecretPolicy,
) -> bool {
    let mut failed = false;
    let (stored_s3, next_s3, s3_failed) = protect_value(
        &cloud.s3.secret_access_key,
        retained.s3_secret_access_key.as_ref(),
        policy,
    );
    cloud.s3.secret_access_key = stored_s3;
    retained.s3_secret_access_key = next_s3;
    failed |= s3_failed;

    let previous_headers = std::mem::take(&mut retained.rest_headers);
    let mut next_headers = Vec::with_capacity(cloud.rest.headers.len());
    for (index, header) in cloud.rest.headers.iter_mut().enumerate() {
        let previous = previous_headers
            .get(index)
            .and_then(|entry| entry.as_ref())
            .filter(|(key, _)| *key == header.key)
            .map(|(_, secret)| secret);
        let (stored, next_secret, header_failed) = protect_value(&header.value, previous, policy);
        header.value = stored;
        failed |= header_failed;
        next_headers.push(next_secret.map(|secret| (header.key.clone(), secret)));
    }
    retained.rest_headers = next_headers;

    failed
}

fn reveal_value(value: &str, policy: SecretPolicy) -> (String, Option<RetainedSecret>) {
    if value.is_empty() {
        return (String::new(), None);
    }
    if !is_protected(value) {
        return (
            value.to_string(),
            Some(RetainedSecret {
                stored_value: value.to_string(),
                revealed_value: Some(value.to_string()),
            }),
        );
    }

    if let Some(encoded) = value.strip_prefix(GPUI_SECRET_PREFIX) {
        if let Some(plaintext) = decrypt_gpui_secret(encoded, policy) {
            return (
                plaintext.clone(),
                Some(RetainedSecret {
                    stored_value: value.to_string(),
                    revealed_value: Some(plaintext),
                }),
            );
        }
        return (
            String::new(),
            Some(RetainedSecret {
                stored_value: value.to_string(),
                revealed_value: None,
            }),
        );
    }

    if let Some(encoded) = value.strip_prefix(SAFE_STORAGE_PREFIX) {
        if let Some(plaintext) = decrypt_safe_storage(encoded, policy) {
            return (
                plaintext.clone(),
                Some(RetainedSecret {
                    stored_value: value.to_string(),
                    revealed_value: Some(plaintext),
                }),
            );
        }
    }

    (
        String::new(),
        Some(RetainedSecret {
            stored_value: value.to_string(),
            revealed_value: None,
        }),
    )
}

fn protect_value(
    value: &str,
    retained: Option<&RetainedSecret>,
    policy: SecretPolicy,
) -> (String, Option<RetainedSecret>, bool) {
    if value.is_empty() {
        if let Some(retained) = retained.filter(|secret| secret.revealed_value.is_none()) {
            return (retained.stored_value.clone(), Some(retained.clone()), false);
        }
        return (String::new(), None, false);
    }

    if let Some(retained) = retained {
        if retained.revealed_value.as_deref() == Some(value)
            && is_protected(&retained.stored_value)
            && !retained.stored_value.starts_with(SAFE_STORAGE_PREFIX)
        {
            return (retained.stored_value.clone(), Some(retained.clone()), false);
        }
    }

    if let Some(stored) = encrypt_secret(value, policy) {
        let next = RetainedSecret {
            stored_value: stored.clone(),
            revealed_value: Some(value.to_string()),
        };
        return (stored, Some(next), false);
    }

    if let Some(retained) = retained {
        let failed = retained.revealed_value.as_deref() != Some(value);
        return (
            retained.stored_value.clone(),
            Some(retained.clone()),
            failed,
        );
    }

    (String::new(), None, true)
}

fn encrypt_secret(plaintext: &str, policy: SecretPolicy) -> Option<String> {
    match policy {
        SecretPolicy::InMemory(key) => seal_with_key(&key, plaintext.as_bytes()),
        SecretPolicy::System => encrypt_system_secret(plaintext),
    }
}

fn decrypt_gpui_secret(encoded: &str, policy: SecretPolicy) -> Option<String> {
    let payload = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    match policy {
        SecretPolicy::InMemory(key) => open_with_key(&key, &payload),
        SecretPolicy::System => decrypt_system_gpui_secret(&payload),
    }
}

fn decrypt_safe_storage(encoded: &str, policy: SecretPolicy) -> Option<String> {
    let payload = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    match policy {
        SecretPolicy::InMemory(_) => None,
        SecretPolicy::System => decrypt_system_safe_storage(&payload),
    }
}

fn derive_subkeys(master: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let mut enc_hasher = Sha256::new();
    enc_hasher.update(b"poratake-gpui-enc-v1");
    enc_hasher.update(master);
    let enc_key: [u8; 32] = enc_hasher.finalize().into();

    let mut mac_hasher = Sha256::new();
    mac_hasher.update(b"poratake-gpui-mac-v1");
    mac_hasher.update(master);
    let mac_key: [u8; 32] = mac_hasher.finalize().into();

    (enc_key, mac_key)
}

fn seal_with_key(master: &[u8; 32], plaintext: &[u8]) -> Option<String> {
    let iv = random_iv()?;
    let (enc_key, mac_key) = derive_subkeys(master);
    let ciphertext = aes_256_cbc_encrypt(&enc_key, &iv, plaintext)?;

    let mut body = Vec::with_capacity(1 + 16 + ciphertext.len() + 32);
    body.push(1);
    body.extend_from_slice(&iv);
    body.extend_from_slice(&ciphertext);

    let mut mac = Hmac::<Sha256>::new_from_slice(&mac_key).ok()?;
    mac.update(&body);
    let tag = mac.finalize().into_bytes();
    body.extend_from_slice(&tag);

    Some(format!(
        "{GPUI_SECRET_PREFIX}{}",
        base64::engine::general_purpose::STANDARD.encode(body)
    ))
}

fn open_with_key(master: &[u8; 32], payload: &[u8]) -> Option<String> {
    if payload.len() < 1 + 16 + 16 + 32 || payload[0] != 1 {
        return None;
    }
    let mac_offset = payload.len() - 32;
    let body = &payload[..mac_offset];
    let tag = &payload[mac_offset..];
    let (enc_key, mac_key) = derive_subkeys(master);

    let mut mac = Hmac::<Sha256>::new_from_slice(&mac_key).ok()?;
    mac.update(body);
    mac.verify_slice(tag).ok()?;

    let iv: [u8; 16] = body[1..17].try_into().ok()?;
    let ciphertext = &body[17..];
    let plaintext = aes_256_cbc_decrypt(&enc_key, &iv, ciphertext)?;
    String::from_utf8(plaintext).ok()
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{c_char, c_void, CString};

    const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const CC_ENCRYPT: u32 = 0;
    const CC_DECRYPT: u32 = 1;
    const CC_ALGORITHM_AES: u32 = 0;
    const CC_OPTION_PKCS7_PADDING: u32 = 1;
    const CC_SUCCESS: i32 = 0;
    const CC_PBKDF2: u32 = 2;
    const CC_PRF_HMAC_SHA1: u32 = 1;
    const ERR_SEC_SUCCESS: i32 = 0;
    const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
    const KEYCHAIN_SERVICE: &str = "com.porabuild.poratake.gpui.cloud-secrets";
    const KEYCHAIN_ACCOUNT: &str = "master-key";
    const ELECTRON_KEYCHAIN_SERVICE: &str = "Poratake Safe Storage";
    const ELECTRON_KEYCHAIN_ACCOUNT: &str = "Poratake";

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFTypeDictionaryKeyCallBacks: c_void;
        static kCFTypeDictionaryValueCallBacks: c_void;
        static kCFBooleanTrue: *const c_void;
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const c_char,
            encoding: u32,
        ) -> *const c_void;
        fn CFDataCreate(allocator: *const c_void, bytes: *const u8, length: isize)
            -> *const c_void;
        fn CFDataGetLength(the_data: *const c_void) -> isize;
        fn CFDataGetBytePtr(the_data: *const c_void) -> *const u8;
        fn CFDictionaryCreate(
            allocator: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            num_values: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecClass: *const c_void;
        static kSecClassGenericPassword: *const c_void;
        static kSecAttrService: *const c_void;
        static kSecAttrAccount: *const c_void;
        static kSecAttrAccessible: *const c_void;
        static kSecAttrAccessibleAfterFirstUnlock: *const c_void;
        static kSecReturnData: *const c_void;
        static kSecMatchLimit: *const c_void;
        static kSecMatchLimitOne: *const c_void;
        static kSecValueData: *const c_void;
        static kSecUseAuthenticationUI: *const c_void;
        static kSecUseAuthenticationUIFail: *const c_void;
        fn SecRandomCopyBytes(rnd: *const c_void, count: usize, bytes: *mut u8) -> i32;
        fn SecItemCopyMatching(query: *const c_void, result: *mut *const c_void) -> i32;
        fn SecItemAdd(attributes: *const c_void, result: *mut *const c_void) -> i32;
    }

    #[link(name = "System", kind = "dylib")]
    unsafe extern "C" {
        fn CCCrypt(
            op: u32,
            alg: u32,
            options: u32,
            key: *const c_void,
            key_length: usize,
            iv: *const c_void,
            data_in: *const c_void,
            data_in_length: usize,
            data_out: *mut c_void,
            data_out_available: usize,
            data_out_moved: *mut usize,
        ) -> i32;
        fn CCKeyDerivationPBKDF(
            algorithm: u32,
            password: *const c_char,
            password_len: usize,
            salt: *const u8,
            salt_len: usize,
            prf: u32,
            rounds: u32,
            derived_key: *mut u8,
            derived_key_len: usize,
        ) -> i32;
    }

    struct CfRef(*const c_void);

    impl Drop for CfRef {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CFRelease(self.0) };
            }
        }
    }

    fn cf_string(value: &str) -> Option<CfRef> {
        let c_string = CString::new(value).ok()?;
        let ptr = unsafe {
            CFStringCreateWithCString(std::ptr::null(), c_string.as_ptr(), CF_STRING_ENCODING_UTF8)
        };
        (!ptr.is_null()).then_some(CfRef(ptr))
    }

    pub fn random_iv() -> Option<[u8; 16]> {
        let mut iv = [0u8; 16];
        let status = unsafe { SecRandomCopyBytes(std::ptr::null(), iv.len(), iv.as_mut_ptr()) };
        (status == ERR_SEC_SUCCESS).then_some(iv)
    }

    fn random_master_key() -> Option<[u8; 32]> {
        let mut key = [0u8; 32];
        let status = unsafe { SecRandomCopyBytes(std::ptr::null(), key.len(), key.as_mut_ptr()) };
        (status == ERR_SEC_SUCCESS).then_some(key)
    }

    fn read_keychain_item(service: &str, account: &str) -> Result<Option<Vec<u8>>, i32> {
        let Some(service_cf) = cf_string(service) else {
            return Err(-1);
        };
        let Some(account_cf) = cf_string(account) else {
            return Err(-1);
        };

        unsafe {
            let keys = [
                kSecClass,
                kSecAttrService,
                kSecAttrAccount,
                kSecReturnData,
                kSecMatchLimit,
                kSecUseAuthenticationUI,
            ];
            let values = [
                kSecClassGenericPassword,
                service_cf.0,
                account_cf.0,
                kCFBooleanTrue,
                kSecMatchLimitOne,
                kSecUseAuthenticationUIFail,
            ];
            let query = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                keys.len() as isize,
                &raw const kCFTypeDictionaryKeyCallBacks,
                &raw const kCFTypeDictionaryValueCallBacks,
            );
            if query.is_null() {
                return Err(-1);
            }
            let _query = CfRef(query);
            let mut result: *const c_void = std::ptr::null();
            let status = SecItemCopyMatching(query, &mut result);
            if status == ERR_SEC_ITEM_NOT_FOUND {
                return Ok(None);
            }
            if status != ERR_SEC_SUCCESS || result.is_null() {
                return Err(status);
            }
            let _result = CfRef(result);
            let len = CFDataGetLength(result);
            if len <= 0 {
                return Ok(None);
            }
            let ptr = CFDataGetBytePtr(result);
            let bytes = std::slice::from_raw_parts(ptr, len as usize).to_vec();
            Ok(Some(bytes))
        }
    }

    fn write_keychain_item(service: &str, account: &str, bytes: &[u8]) -> bool {
        let Some(service_cf) = cf_string(service) else {
            return false;
        };
        let Some(account_cf) = cf_string(account) else {
            return false;
        };
        unsafe {
            let data = CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize);
            if data.is_null() {
                return false;
            }
            let _data = CfRef(data);
            let keys = [
                kSecClass,
                kSecAttrService,
                kSecAttrAccount,
                kSecAttrAccessible,
                kSecValueData,
            ];
            let values = [
                kSecClassGenericPassword,
                service_cf.0,
                account_cf.0,
                kSecAttrAccessibleAfterFirstUnlock,
                data,
            ];
            let attrs = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                keys.len() as isize,
                &raw const kCFTypeDictionaryKeyCallBacks,
                &raw const kCFTypeDictionaryValueCallBacks,
            );
            if attrs.is_null() {
                return false;
            }
            let _attrs = CfRef(attrs);
            SecItemAdd(attrs, std::ptr::null_mut()) == ERR_SEC_SUCCESS
        }
    }

    fn load_or_create_gpui_key() -> Option<[u8; 32]> {
        match read_keychain_item(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT) {
            Ok(Some(bytes)) if bytes.len() == 32 => bytes.try_into().ok(),
            Ok(None) => {
                let key = random_master_key()?;
                write_keychain_item(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, &key).then_some(key)
            }
            _ => None,
        }
    }

    pub fn aes_256_cbc_encrypt(key: &[u8; 32], iv: &[u8; 16], plaintext: &[u8]) -> Option<Vec<u8>> {
        let mut out = vec![0u8; plaintext.len() + 16];
        let mut moved = 0usize;
        let status = unsafe {
            CCCrypt(
                CC_ENCRYPT,
                CC_ALGORITHM_AES,
                CC_OPTION_PKCS7_PADDING,
                key.as_ptr().cast(),
                key.len(),
                iv.as_ptr().cast(),
                plaintext.as_ptr().cast(),
                plaintext.len(),
                out.as_mut_ptr().cast(),
                out.len(),
                &mut moved,
            )
        };
        if status != CC_SUCCESS {
            return None;
        }
        out.truncate(moved);
        Some(out)
    }

    pub fn aes_256_cbc_decrypt(
        key: &[u8; 32],
        iv: &[u8; 16],
        ciphertext: &[u8],
    ) -> Option<Vec<u8>> {
        let mut out = vec![0u8; ciphertext.len() + 16];
        let mut moved = 0usize;
        let status = unsafe {
            CCCrypt(
                CC_DECRYPT,
                CC_ALGORITHM_AES,
                CC_OPTION_PKCS7_PADDING,
                key.as_ptr().cast(),
                key.len(),
                iv.as_ptr().cast(),
                ciphertext.as_ptr().cast(),
                ciphertext.len(),
                out.as_mut_ptr().cast(),
                out.len(),
                &mut moved,
            )
        };
        if status != CC_SUCCESS {
            return None;
        }
        out.truncate(moved);
        Some(out)
    }

    pub fn decrypt_electron_v10_with_password(password: &[u8], payload: &[u8]) -> Option<String> {
        let ciphertext = payload.strip_prefix(b"v10")?;
        if ciphertext.is_empty() {
            return None;
        }
        let mut derived_key = [0u8; 16];
        let salt = b"saltysalt";
        let kdf_status = unsafe {
            CCKeyDerivationPBKDF(
                CC_PBKDF2,
                password.as_ptr().cast(),
                password.len(),
                salt.as_ptr(),
                salt.len(),
                CC_PRF_HMAC_SHA1,
                1003,
                derived_key.as_mut_ptr(),
                derived_key.len(),
            )
        };
        if kdf_status != CC_SUCCESS {
            return None;
        }
        let iv = [b' '; 16];
        let mut out = vec![0u8; ciphertext.len() + 16];
        let mut moved = 0usize;
        let crypt_status = unsafe {
            CCCrypt(
                CC_DECRYPT,
                CC_ALGORITHM_AES,
                CC_OPTION_PKCS7_PADDING,
                derived_key.as_ptr().cast(),
                derived_key.len(),
                iv.as_ptr().cast(),
                ciphertext.as_ptr().cast(),
                ciphertext.len(),
                out.as_mut_ptr().cast(),
                out.len(),
                &mut moved,
            )
        };
        if crypt_status != CC_SUCCESS {
            return None;
        }
        out.truncate(moved);
        String::from_utf8(out).ok()
    }

    pub fn encrypt_system_secret(plaintext: &str) -> Option<String> {
        let key = load_or_create_gpui_key()?;
        super::seal_with_key(&key, plaintext.as_bytes())
    }

    pub fn decrypt_system_gpui_secret(payload: &[u8]) -> Option<String> {
        let bytes = read_keychain_item(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT)
            .ok()
            .flatten()?;
        let key: [u8; 32] = bytes.try_into().ok()?;
        super::open_with_key(&key, payload)
    }

    pub fn decrypt_system_safe_storage(payload: &[u8]) -> Option<String> {
        let password = read_keychain_item(ELECTRON_KEYCHAIN_SERVICE, ELECTRON_KEYCHAIN_ACCOUNT)
            .ok()
            .flatten()?;
        decrypt_electron_v10_with_password(&password, payload)
    }
}

#[cfg(windows)]
mod platform {
    use base64::Engine as _;

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

    #[repr(C)]
    struct CryptIntegerBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *const CryptIntegerBlob,
            data_descr: *const u16,
            optional_entropy: *const CryptIntegerBlob,
            reserved: *mut core::ffi::c_void,
            prompt_struct: *mut core::ffi::c_void,
            flags: u32,
            data_out: *mut CryptIntegerBlob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const CryptIntegerBlob,
            data_descr: *mut *mut u16,
            optional_entropy: *const CryptIntegerBlob,
            reserved: *mut core::ffi::c_void,
            prompt_struct: *mut core::ffi::c_void,
            flags: u32,
            data_out: *mut CryptIntegerBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(hmem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
    }

    #[allow(dead_code)]
    pub fn random_iv() -> Option<[u8; 16]> {
        None
    }

    pub fn aes_256_cbc_encrypt(
        _key: &[u8; 32],
        _iv: &[u8; 16],
        _plaintext: &[u8],
    ) -> Option<Vec<u8>> {
        None
    }

    pub fn aes_256_cbc_decrypt(
        _key: &[u8; 32],
        _iv: &[u8; 16],
        _ciphertext: &[u8],
    ) -> Option<Vec<u8>> {
        None
    }

    pub fn encrypt_system_secret(plaintext: &str) -> Option<String> {
        let mut bytes = plaintext.as_bytes().to_vec();
        let input = CryptIntegerBlob {
            cb_data: u32::try_from(bytes.len()).ok()?,
            pb_data: bytes.as_mut_ptr(),
        };
        let mut output = CryptIntegerBlob {
            cb_data: 0,
            pb_data: std::ptr::null_mut(),
        };
        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 || output.pb_data.is_null() {
            return None;
        }
        let protected =
            unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize) };
        let mut payload = Vec::with_capacity(3 + protected.len());
        payload.extend_from_slice(b"v10");
        payload.extend_from_slice(protected);
        unsafe { LocalFree(output.pb_data.cast()) };
        Some(format!(
            "{}{}",
            super::SAFE_STORAGE_PREFIX,
            base64::engine::general_purpose::STANDARD.encode(payload)
        ))
    }

    pub fn decrypt_system_gpui_secret(_payload: &[u8]) -> Option<String> {
        None
    }

    pub fn decrypt_system_safe_storage(payload: &[u8]) -> Option<String> {
        let encrypted = payload.strip_prefix(b"v10")?;
        let mut copy = encrypted.to_vec();
        let input = CryptIntegerBlob {
            cb_data: u32::try_from(copy.len()).ok()?,
            pb_data: copy.as_mut_ptr(),
        };
        let mut output = CryptIntegerBlob {
            cb_data: 0,
            pb_data: std::ptr::null_mut(),
        };
        let ok = unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 || output.pb_data.is_null() {
            return None;
        }
        let plain =
            unsafe { std::slice::from_raw_parts(output.pb_data, output.cb_data as usize) }.to_vec();
        unsafe { LocalFree(output.pb_data.cast()) };
        String::from_utf8(plain).ok()
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    #[allow(dead_code)]
    pub fn random_iv() -> Option<[u8; 16]> {
        None
    }

    pub fn aes_256_cbc_encrypt(
        _key: &[u8; 32],
        _iv: &[u8; 16],
        _plaintext: &[u8],
    ) -> Option<Vec<u8>> {
        None
    }

    pub fn aes_256_cbc_decrypt(
        _key: &[u8; 32],
        _iv: &[u8; 16],
        _ciphertext: &[u8],
    ) -> Option<Vec<u8>> {
        None
    }

    pub fn encrypt_system_secret(_plaintext: &str) -> Option<String> {
        None
    }

    pub fn decrypt_system_gpui_secret(_payload: &[u8]) -> Option<String> {
        None
    }

    pub fn decrypt_system_safe_storage(_payload: &[u8]) -> Option<String> {
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn random_iv() -> Option<[u8; 16]> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let mut hasher = Sha256::new();
    hasher.update(nanos.to_le_bytes());
    hasher.update(std::process::id().to_le_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    digest[..16].try_into().ok()
}

#[cfg(target_os = "macos")]
fn random_iv() -> Option<[u8; 16]> {
    platform::random_iv()
}

fn aes_256_cbc_encrypt(key: &[u8; 32], iv: &[u8; 16], plaintext: &[u8]) -> Option<Vec<u8>> {
    if cfg!(target_os = "macos") {
        return platform::aes_256_cbc_encrypt(key, iv, plaintext);
    }
    None
}

fn aes_256_cbc_decrypt(key: &[u8; 32], iv: &[u8; 16], ciphertext: &[u8]) -> Option<Vec<u8>> {
    if cfg!(target_os = "macos") {
        return platform::aes_256_cbc_decrypt(key, iv, ciphertext);
    }
    None
}

fn encrypt_system_secret(plaintext: &str) -> Option<String> {
    platform::encrypt_system_secret(plaintext)
}

fn decrypt_system_gpui_secret(payload: &[u8]) -> Option<String> {
    platform::decrypt_system_gpui_secret(payload)
}

fn decrypt_system_safe_storage(payload: &[u8]) -> Option<String> {
    platform::decrypt_system_safe_storage(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::RestHeader;

    #[test]
    fn round_trips_secrets_without_leaving_plaintext_on_disk() {
        let policy = SecretPolicy::InMemory(*b"0123456789abcdef0123456789abcdef");
        let mut cloud = CloudConfig {
            active_provider: "s3".into(),
            ..CloudConfig::default()
        };
        cloud.s3.secret_access_key = "top-secret-s3".into();
        cloud.rest.headers = vec![RestHeader {
            key: "Authorization".into(),
            value: "Bearer top-secret-token".into(),
        }];

        let mut retained = RetainedCloudSecrets::default();
        let failed = protect_cloud_secrets(&mut cloud, &mut retained, policy);
        assert!(!failed);
        if cfg!(target_os = "macos") {
            assert!(cloud.s3.secret_access_key.starts_with(GPUI_SECRET_PREFIX));
            assert!(cloud.rest.headers[0].value.starts_with(GPUI_SECRET_PREFIX));
            assert!(!cloud.s3.secret_access_key.contains("top-secret-s3"));

            let _ = reveal_cloud_secrets(&mut cloud, policy);
            assert_eq!(cloud.s3.secret_access_key, "top-secret-s3");
            assert_eq!(cloud.rest.headers[0].value, "Bearer top-secret-token");
        }
    }

    #[test]
    fn preserves_locked_electron_safe_storage_tokens_across_unrelated_saves() {
        let policy = SecretPolicy::InMemory(*b"0123456789abcdef0123456789abcdef");
        let locked_s3 = format!("{SAFE_STORAGE_PREFIX}djEwLWVuY3J5cHRlZC1zMw==");
        let locked_header = format!("{SAFE_STORAGE_PREFIX}djEwLWVuY3J5cHRlZC1oZWFkZXI=");
        let mut cloud = CloudConfig::default();
        cloud.s3.secret_access_key = locked_s3.clone();
        cloud.rest.headers = vec![RestHeader {
            key: "Authorization".into(),
            value: locked_header.clone(),
        }];

        let mut retained = reveal_cloud_secrets(&mut cloud, policy);
        assert_eq!(cloud.s3.secret_access_key, "");
        assert_eq!(cloud.rest.headers[0].value, "");

        let failed = protect_cloud_secrets(&mut cloud, &mut retained, policy);
        assert!(!failed);
        assert_eq!(cloud.s3.secret_access_key, locked_s3);
        assert_eq!(cloud.rest.headers[0].value, locked_header);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn decrypts_electron_macos_v10_aes128_cbc_payload() {
        let encrypted_b64 = "djEws+0oG4J3W+J5d0oE4G2k9Q==";
        let _ = base64::engine::general_purpose::STANDARD
            .decode(encrypted_b64)
            .unwrap();

        let password = b"electron-safe-storage-pass";
        let mut derived = [0u8; 16];
        let iv = [b' '; 16];
        let mut out = vec![0u8; 64];
        let mut moved = 0usize;

        #[link(name = "System", kind = "dylib")]
        unsafe extern "C" {
            fn CCKeyDerivationPBKDF(
                algorithm: u32,
                password: *const std::ffi::c_char,
                password_len: usize,
                salt: *const u8,
                salt_len: usize,
                prf: u32,
                rounds: u32,
                derived_key: *mut u8,
                derived_key_len: usize,
            ) -> i32;
            fn CCCrypt(
                op: u32,
                alg: u32,
                options: u32,
                key: *const std::ffi::c_void,
                key_length: usize,
                iv: *const std::ffi::c_void,
                data_in: *const std::ffi::c_void,
                data_in_length: usize,
                data_out: *mut std::ffi::c_void,
                data_out_available: usize,
                data_out_moved: *mut usize,
            ) -> i32;
        }

        let salt = b"saltysalt";
        let plain = b"my-s3-secret-access-key";
        let status_kdf = unsafe {
            CCKeyDerivationPBKDF(
                2,
                password.as_ptr().cast(),
                password.len(),
                salt.as_ptr(),
                salt.len(),
                1,
                1003,
                derived.as_mut_ptr(),
                derived.len(),
            )
        };
        assert_eq!(status_kdf, 0);
        let status_enc = unsafe {
            CCCrypt(
                0,
                0,
                1,
                derived.as_ptr().cast(),
                derived.len(),
                iv.as_ptr().cast(),
                plain.as_ptr().cast(),
                plain.len(),
                out.as_mut_ptr().cast(),
                out.len(),
                &mut moved,
            )
        };
        assert_eq!(status_enc, 0);
        let mut payload = b"v10".to_vec();
        payload.extend_from_slice(&out[..moved]);
        assert_eq!(
            platform::decrypt_electron_v10_with_password(password, &payload).as_deref(),
            Some("my-s3-secret-access-key")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rejects_tampered_gpui_secret_ciphertext() {
        let key = *b"0123456789abcdef0123456789abcdef";
        let sealed = seal_with_key(&key, b"api-token").expect("sealed");
        let encoded = sealed.strip_prefix(GPUI_SECRET_PREFIX).expect("prefix");
        let mut raw = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .expect("b64");
        raw[18] ^= 0xff;
        assert!(open_with_key(&key, &raw).is_none());
    }
}

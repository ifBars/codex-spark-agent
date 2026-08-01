use std::fmt::Debug;

pub(crate) trait KeyProtector: Send + Sync + Debug {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, String>;
    fn unprotect(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String>;
}

#[cfg(any(not(windows), test))]
#[derive(Debug)]
pub(crate) struct UnavailableProtector;

#[cfg(any(not(windows), test))]
impl KeyProtector for UnavailableProtector {
    fn protect(&self, _plaintext: &[u8]) -> Result<Vec<u8>, String> {
        Err("protected key storage is unavailable".into())
    }
    fn unprotect(&self, _ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        Err("protected key storage is unavailable".into())
    }
}

#[cfg(windows)]
#[derive(Debug)]
pub(crate) struct WindowsDpapiProtector;

#[cfg(windows)]
impl WindowsDpapiProtector {
    fn crypt(input: &[u8], protect: bool) -> Result<Vec<u8>, String> {
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::Cryptography::{
                CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
            },
        };
        let input_len =
            u32::try_from(input.len()).map_err(|_| "key material is too large".to_owned())?;
        let input_blob = CRYPT_INTEGER_BLOB {
            cbData: input_len,
            pbData: input.as_ptr() as *mut u8,
        };
        let mut output_blob = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let result = unsafe {
            if protect {
                CryptProtectData(
                    &input_blob,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output_blob,
                )
            } else {
                CryptUnprotectData(
                    &input_blob,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output_blob,
                )
            }
        };
        if result == 0 {
            return Err("Windows DPAPI could not protect Wave 1 ledger material".into());
        }
        let protected = unsafe {
            std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize).to_vec()
        };
        unsafe {
            LocalFree(output_blob.pbData.cast());
        }
        Ok(protected)
    }
}

#[cfg(windows)]
impl KeyProtector for WindowsDpapiProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        Self::crypt(plaintext, true)
    }
    fn unprotect(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        Self::crypt(ciphertext, false)
    }
}

#[cfg(not(windows))]
pub(crate) type WindowsDpapiProtector = UnavailableProtector;

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct TestProtector;

#[cfg(test)]
impl KeyProtector for TestProtector {
    fn protect(&self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        Ok(plaintext.iter().map(|byte| byte ^ 0xA5).collect())
    }
    fn unprotect(&self, ciphertext: &[u8]) -> Result<Vec<u8>, String> {
        Ok(ciphertext.iter().map(|byte| byte ^ 0xA5).collect())
    }
}

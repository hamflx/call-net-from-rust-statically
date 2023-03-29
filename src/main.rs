extern "C" {
    fn wtf_des_encrypt(message: *const u8, key: *const u8) -> *const u8;
    fn wtf_des_decrypt(cipher_text: *const u8, key: *const u8) -> *const u8;
    fn wtf_des_free(ptr: *const u8);
}

fn main() {
    let key = b"key\0";
    let cipher_text = unsafe { wtf_des_encrypt(b"message\0".as_ptr(), key.as_ptr()) };
    let cipher_text = unsafe { std::ffi::CStr::from_ptr(cipher_text as *const i8) };
    let plain_text = unsafe { wtf_des_decrypt(cipher_text.as_ptr() as _, key.as_ptr()) };
    let plain_text = unsafe { std::ffi::CStr::from_ptr(plain_text as *const i8) };
    println!("cipher_text: {}", cipher_text.to_str().unwrap());
    println!("plain_text: {}", plain_text.to_str().unwrap());

    unsafe {
        wtf_des_free(cipher_text.as_ptr() as _);
        wtf_des_free(plain_text.as_ptr() as _);
    }
}

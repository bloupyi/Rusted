//! Native backend for the EXISTING Minecraft JVM process, not a replacement game.
//! The JNI interface owns copies; no Java array or JNIEnv escapes its call/thread.
use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
use jni::{
    objects::{JByteArray, JClass},
    sys::{jbyteArray, jint},
    JNIEnv,
};
use std::{
    cell::RefCell,
    panic::{catch_unwind, AssertUnwindSafe},
};

/// Our native allocation guard. Oversized packets must keep the Java path.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const ABI_VERSION: i32 = 1;
type CodecResult<T> = Result<T, String>;

thread_local! {
    // Reuse the encoder allocation on each caller's own Netty/JVM thread.
    // No shared lock, Java reference, async scheduling, or cross-thread state.
    static ENCODER: RefCell<Compress> = RefCell::new(Compress::new(Compression::default(), true));
}

pub fn compress(input: &[u8]) -> CodecResult<Vec<u8>> {
    if input.len() > MAX_BYTES {
        return Err("Native input limit exceeded".into());
    }
    ENCODER.with(|encoder| {
        let mut encoder = encoder
            .try_borrow_mut()
            .map_err(|_| "Reentrant native encoder call")?;
        encoder.reset();
        // Space for stored-block overhead; no unbounded encoder output growth.
        let mut output = Vec::with_capacity(input.len() + input.len() / 100 + 128);
        let status = encoder
            .compress_vec(input, &mut output, FlushCompress::Finish)
            .map_err(|e| e.to_string())?;
        if status != Status::StreamEnd || encoder.total_in() != input.len() as u64 {
            return Err("Native compression did not finish within output bound".into());
        }
        Ok(output)
    })
}

/// Strict oracle/test decoder. The Minecraft integration currently replaces encode only.
pub fn decompress(input: &[u8], expected: usize) -> CodecResult<Vec<u8>> {
    if expected > MAX_BYTES || input.len() > MAX_BYTES + 65536 {
        return Err("Native output/input limit exceeded".into());
    }
    // One spare byte detects expansion past the declared length; never unbounded.
    let mut output = vec![0; expected + 1];
    let mut decoder = Decompress::new(true);
    let status = decoder
        .decompress(input, &mut output, FlushDecompress::Finish)
        .map_err(|e| e.to_string())?;
    if status != Status::StreamEnd
        || decoder.total_out() != expected as u64
        || decoder.total_in() != input.len() as u64
    {
        return Err("Invalid zlib stream, trailing bytes, or uncompressed length mismatch".into());
    }
    output.truncate(expected);
    Ok(output)
}

fn array_call<'a, F>(env: &mut JNIEnv<'a>, array: JByteArray<'a>, operation: F) -> jbyteArray
where
    F: FnOnce(&[u8]) -> CodecResult<Vec<u8>>,
{
    let result = catch_unwind(AssertUnwindSafe(|| -> CodecResult<jbyteArray> {
        let length = env.get_array_length(&array).map_err(|e| e.to_string())? as usize;
        if length > MAX_BYTES + 65536 {
            return Err("JNI input too large".into());
        }
        let bytes = env.convert_byte_array(array).map_err(|e| e.to_string())?;
        let output = operation(&bytes)?;
        Ok(env
            .byte_array_from_slice(&output)
            .map_err(|e| e.to_string())?
            .into_raw())
    }));
    match result {
        Ok(Ok(array)) => array,
        other => {
            let message = match other {
                Ok(Err(message)) => message,
                Err(_) => "Rust backend panic contained at JNI boundary".into(),
                _ => unreachable!(),
            };
            if !env.exception_check().unwrap_or(true) {
                let _ = env.throw_new("java/lang/IllegalArgumentException", message);
            }
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_rusted_backend_NativeCodec_abiVersion(
    _env: JNIEnv,
    _class: JClass,
) -> jint {
    ABI_VERSION
}

#[no_mangle]
pub extern "system" fn Java_dev_rusted_backend_NativeCodec_compressNative<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    input: JByteArray<'a>,
) -> jbyteArray {
    array_call(&mut env, input, compress)
}

#[no_mangle]
pub extern "system" fn Java_dev_rusted_backend_NativeCodec_decompressNative<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    input: JByteArray<'a>,
    expected: jint,
) -> jbyteArray {
    array_call(&mut env, input, |bytes| {
        if expected < 0 {
            Err("Negative output length".into())
        } else {
            decompress(bytes, expected as usize)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sizes_and_content() {
        for size in [0, 1, 255, 256, 257, 8192, 1_000_000, MAX_BYTES] {
            let bytes: Vec<u8> = (0..size)
                .map(|i| (i.wrapping_mul(31) % 251) as u8)
                .collect();
            let z = compress(&bytes).unwrap();
            assert_eq!(decompress(&z, size).unwrap(), bytes);
        }
    }
    #[test]
    fn invalid_lengths_trailing_and_checksum() {
        let z = compress(b"packet payload").unwrap();
        assert!(decompress(&z, 1).is_err());
        assert!(decompress(&z, 100).is_err());
        let mut trailing = z.clone();
        trailing.push(0);
        assert!(decompress(&trailing, 14).is_err());
        let mut corrupt = z.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(decompress(&corrupt, 14).is_err());
        assert!(decompress(&z[..z.len() - 1], 14).is_err());
        assert!(decompress(&z, MAX_BYTES + 1).is_err());
        assert!(compress(&vec![0; MAX_BYTES + 1]).is_err());
    }
    #[test]
    fn independent_threads() {
        let threads: Vec<_> = (0..8)
            .map(|n| {
                std::thread::spawn(move || {
                    for _ in 0..50 {
                        let data = vec![n; 4096];
                        assert_eq!(
                            decompress(&compress(&data).unwrap(), data.len()).unwrap(),
                            data
                        );
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
    }
}

//! Cross-platform FFI surfaces:
//! - `c_abi`: Windows WinUI 3 (`vexio.h`), macOS Swift / Quick Look (`VexioCoreFFI`), and C/C++
//! - `jni_bridge`: Android & Googlebook OS zero-copy `DirectByteBuffer` + Vulkan `Surface` JNI
//! - `wasm_api`: ChromeOS PWA `wasm-bindgen` SIMD + `SharedArrayBuffer` Worker session

pub mod c_abi;
#[cfg(not(target_arch = "wasm32"))]
pub mod jni_bridge;
#[cfg(target_arch = "wasm32")]
pub mod wasm_api;

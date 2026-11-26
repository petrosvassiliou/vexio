#![no_main]

use libfuzzer_sys::fuzz_target;
use vexio_vsdx::read_vsdx;

fuzz_target!(|data: &[u8]| {
    if data.len() > 2 * 1024 * 1024 {
        return;
    }
    let _ = read_vsdx(data);
});

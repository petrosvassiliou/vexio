#![no_main]

use libfuzzer_sys::fuzz_target;
use vexio_vsd_legacy::read_legacy_auto;

fuzz_target!(|data: &[u8]| {
    if data.len() > 2 * 1024 * 1024 {
        return;
    }
    let _ = read_legacy_auto(data, "vsd");
    let _ = read_legacy_auto(data, "vdx");
    let _ = read_legacy_auto(data, "vdw");
});

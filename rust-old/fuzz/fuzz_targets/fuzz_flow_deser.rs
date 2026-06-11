#![no_main]
use libfuzzer_sys::fuzz_target;
use szal::flow::FlowDef;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        if let Ok(flow) = serde_json::from_str::<FlowDef>(s) {
            let _ = flow.validate();
        }
    }
});

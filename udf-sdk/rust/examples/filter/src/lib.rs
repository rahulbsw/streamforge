wit_bindgen::generate!({
    path: "../../../../wit/streamforge-udf-v1",
    world: "filter-v1",
});

use exports::streamforge::udf::filter_api::Guest;
use exports::streamforge::udf::types::{EnvelopeInput, ErrorCode, UdfError};

struct FilterGuest;

static mut INVOCATIONS: u32 = 0;

impl Guest for FilterGuest {
    fn filter(input: EnvelopeInput) -> Result<bool, UdfError> {
        if contains(&input.value, br#""loop":true"#) {
            loop {
                core::hint::spin_loop();
            }
        }
        if contains(&input.value, br#""stack":true"#) {
            consume_stack(0);
        }
        if contains(&input.value, br#""guest_error":true"#) {
            return Err(UdfError {
                code: ErrorCode::Rejected,
                message: "fixture-requested rejection".to_string(),
            });
        }
        if contains(&input.value, br#""echo_error":true"#) {
            return Err(UdfError {
                code: ErrorCode::Rejected,
                // An untrusted guest may echo the complete input. The host
                // security boundary must not retain or log this message.
                message: String::from_utf8_lossy(&input.value).into_owned(),
            });
        }
        if contains(&input.value, br#""state_probe":true"#) {
            // The host contract creates a fresh instance for every invocation,
            // so this mutable guest global must always begin at zero.
            let previous = unsafe { INVOCATIONS };
            unsafe {
                INVOCATIONS = INVOCATIONS.saturating_add(1);
            }
            return Ok(previous == 0);
        }
        Ok(!contains(&input.value, br#""allow":false"#))
    }
}

#[inline(never)]
#[allow(unconditional_recursion)]
fn consume_stack(depth: u32) -> u32 {
    // Keep the guest's linear-memory stack frame small so Wasmtime's native
    // call-stack ceiling is reached before Rust's own wasm stack guard.
    let frame = [depth as u8; 16];
    core::hint::black_box(&frame);
    let child = consume_stack(depth.wrapping_add(1));
    core::hint::black_box(&frame);
    child.wrapping_add(depth)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}

export!(FilterGuest);

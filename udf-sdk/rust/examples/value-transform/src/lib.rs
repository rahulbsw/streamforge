wit_bindgen::generate!({
    path: "../../../../wit/streamforge-udf-v1",
    world: "value-transform-v1",
});

use exports::streamforge::udf::types::{ErrorCode, UdfError};
use exports::streamforge::udf::value_transform_api::Guest;

struct ValueTransformGuest;

impl Guest for ValueTransformGuest {
    fn transform(value_json: Vec<u8>) -> Result<Vec<u8>, UdfError> {
        if contains(&value_json, br#""guest_error":true"#) {
            return Err(UdfError {
                code: ErrorCode::InvalidInput,
                message: "fixture-requested value error".to_string(),
            });
        }
        if contains(&value_json, br#""large":true"#) {
            return Ok(vec![b'0'; 8 * 1024]);
        }
        Ok(value_json)
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}

export!(ValueTransformGuest);

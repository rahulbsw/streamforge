wit_bindgen::generate!({
    path: "../../../../wit/streamforge-udf-v1",
    world: "envelope-transform-v1",
});

use exports::streamforge::udf::envelope_transform_api::Guest;
use exports::streamforge::udf::types::{EnvelopeInput, EnvelopeOutput, Header, UdfError};

struct EnvelopeTransformGuest;

impl Guest for EnvelopeTransformGuest {
    fn transform(input: EnvelopeInput) -> Result<EnvelopeOutput, UdfError> {
        let mut headers = input.headers;
        headers.push(Header {
            name: "x-streamforge-wasm".to_string(),
            value: b"ok".to_vec(),
        });
        Ok(EnvelopeOutput {
            key: input.key,
            value: input.value,
            headers,
            timestamp: input.timestamp.map(|timestamp| timestamp + 1),
        })
    }
}

export!(EnvelopeTransformGuest);

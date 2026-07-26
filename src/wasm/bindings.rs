pub(crate) mod filter {
    wasmtime::component::bindgen!({
        path: "wit/streamforge-udf-v1",
        world: "filter-v1",
    });
}

pub(crate) mod value_transform {
    wasmtime::component::bindgen!({
        path: "wit/streamforge-udf-v1",
        world: "value-transform-v1",
    });
}

pub(crate) mod envelope_transform {
    wasmtime::component::bindgen!({
        path: "wit/streamforge-udf-v1",
        world: "envelope-transform-v1",
    });
}

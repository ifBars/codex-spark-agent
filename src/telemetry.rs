use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub(crate) fn init() {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("warn"))
        .expect("static tracing filter is valid")
        // Tungstenite traces the serialized upgrade request, including headers.
        // Spark emits its own redacted transport diagnostics instead.
        .add_directive("tungstenite=off".parse().expect("static directive"))
        .add_directive("tokio_tungstenite=off".parse().expect("static directive"));
    let fmt_layer = fmt::layer()
        .compact()
        .with_target(false)
        .with_writer(std::io::stderr);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init();
}

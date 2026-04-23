//! Binary entry point: delegates to [`backend::run`].

#[tokio::main]
async fn main() {
    if let Err(err) = backend::run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

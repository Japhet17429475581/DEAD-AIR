//! Read-only smoke test: never installs packages, captures, or starts benchmarks.
#[tokio::main]
async fn main() {
    let diagnostics = dead_air::diagnostics::check().await;
    println!("{}", serde_json::to_string_pretty(&diagnostics).unwrap());
    match dead_air::process::output("hashcat", &["-I"], 30).await {
        Ok(output) => println!("Hashcat actual device enumeration:\n{output}"),
        Err(error) => println!("Hashcat actual device error: {error}"),
    }
}

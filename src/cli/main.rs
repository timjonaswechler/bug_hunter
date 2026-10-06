fn main() -> std::process::ExitCode {
    match woodpecker::cli::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{}", serde_json::json!({"error":error.to_string()}));
            std::process::ExitCode::FAILURE
        }
    }
}

fn main() {
    // Transcription worker: runs transcribe.cpp in an isolated child process.
    if handy_app_lib::engine_supervisor::is_worker_invocation() {
        std::process::exit(handy_app_lib::engine_supervisor::run_worker());
    }
    println!("handy-server");
}

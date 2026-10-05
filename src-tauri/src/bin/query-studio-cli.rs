#[tokio::main]
async fn main() {
    if let Err(error) = db_lang_lib::automation::run(std::env::args().skip(1).collect()).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

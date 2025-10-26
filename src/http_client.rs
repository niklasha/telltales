use reqwest::blocking::Client;

pub fn build_http_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("telltales-cli/0.1 (+https://github.com/niklasha/telltales)")
        .build()
}

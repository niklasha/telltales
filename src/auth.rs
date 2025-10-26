use crate::config::TelldusCredentials;
use dialoguer::Input;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use reqwest_oauth1::{Error as OAuth1Error, OAuthClientProvider, Secrets};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;
use thiserror::Error;
use url::Url;

const REQUEST_TOKEN_URL: &str = "https://pa-api.telldus.com/oauth/requestToken";
const AUTHORIZE_URL: &str = "https://pa-api.telldus.com/oauth/authorize";
const ACCESS_TOKEN_URL: &str = "https://pa-api.telldus.com/oauth/accessToken";
const PROFILE_URL: &str = "https://pa-api.telldus.com/json/user/profile";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("consumer keys are required before authenticating")]
    MissingConsumerKeys,
    #[error("HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("unable to parse OAuth response: {0}")]
    ParseToken(#[from] serde_urlencoded::de::Error),
    #[error("OAuth handshake failed")]
    OAuth(#[from] OAuth1Error),
    #[error("OAuth authorization was denied")]
    AuthorizationDenied,
    #[error("OAuth response missing field `{0}`")]
    MissingField(&'static str),
    #[error("Telldus Live rejected the request with status {0}")]
    VerificationFailed(String),
    #[error("stored tokens were rejected; please re-authorize")]
    Unauthorized,
    #[error("authorization code or redirect URL is required")]
    MissingVerifier,
    #[error("redirect URL missing oauth_verifier parameter")]
    VerifierNotFound,
    #[error("callback listener failed: {0}")]
    CallbackListener(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("prompt failed")]
    Prompt(#[from] dialoguer::Error),
}

pub struct AuthOutcome {
    pub tokens_refreshed: bool,
    pub account_name: Option<String>,
}

pub fn validate(credentials: &mut TelldusCredentials) -> Result<AuthOutcome, AuthError> {
    if credentials.public_key.trim().is_empty() || credentials.private_key.trim().is_empty() {
        return Err(AuthError::MissingConsumerKeys);
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("telltales-cli/0.1 (+https://github.com/niklasha/telltales)")
        .build()?;

    let mut refreshed = false;

    if credentials.token.trim().is_empty() || credentials.token_secret.trim().is_empty() {
        let (token, secret) = oauth_dance(&client, credentials)?;
        credentials.token = token;
        credentials.token_secret = secret;
        refreshed = true;
    }

    match verify_profile(&client, credentials) {
        Ok(name) => Ok(AuthOutcome {
            tokens_refreshed: refreshed,
            account_name: name,
        }),
        Err(AuthError::Unauthorized) => {
            println!("Stored tokens were rejected by Telldus Live; starting OAuth flow.");
            let (token, secret) = oauth_dance(&client, credentials)?;
            credentials.token = token;
            credentials.token_secret = secret;
            refreshed = true;
            let name = verify_profile(&client, credentials)?;
            Ok(AuthOutcome {
                tokens_refreshed: refreshed,
                account_name: name,
            })
        }
        Err(err) => Err(err),
    }
}

fn oauth_dance(
    client: &Client,
    credentials: &TelldusCredentials,
) -> Result<(String, String), AuthError> {
    let callback = CallbackServer::start()?;
    let temp = request_token(
        client,
        &credentials.public_key,
        &credentials.private_key,
        &callback.callback_url,
    )?;
    let authorize_url = format!("{AUTHORIZE_URL}?oauth_token={}", temp.token);
    println!("Open this URL in your browser to authorize Telldus Live access:");
    println!("{authorize_url}");
    println!(
        "After approving, Telldus Live redirects to {}.\n\
If the CLI captures the redirect automatically, you can close the browser tab.\n\
Otherwise, copy the full redirect URL (or the code shown) and paste it below.",
        callback.callback_url
    );

    let verifier = match callback.wait_for_verifier(Duration::from_secs(300))? {
        Some(code) => code,
        None => {
            let verifier_input: String = Input::new()
                .with_prompt("Verification code or redirect URL")
                .allow_empty(false)
                .interact_text()?;
            extract_verifier(&verifier_input)?
        }
    };

    exchange_access_token(
        client,
        &credentials.public_key,
        &credentials.private_key,
        temp,
        &verifier,
    )
}

fn request_token(
    client: &Client,
    consumer_key: &str,
    consumer_secret: &str,
    callback_url: &str,
) -> Result<TempToken, AuthError> {
    let secrets = Secrets::new(consumer_key, consumer_secret);
    let response = client
        .clone()
        .oauth1(secrets)
        .post(REQUEST_TOKEN_URL)
        .query(&[("oauth_callback", callback_url)])
        .send()?
        .error_for_status()?
        .text()?;

    parse_token_response(&response)
}

fn exchange_access_token(
    client: &Client,
    consumer_key: &str,
    consumer_secret: &str,
    temp: TempToken,
    verifier: &str,
) -> Result<(String, String), AuthError> {
    let secrets = Secrets::new(consumer_key, consumer_secret).token(&temp.token, &temp.secret);
    let response = client
        .clone()
        .oauth1(secrets)
        .post(ACCESS_TOKEN_URL)
        .query(&[("oauth_verifier", verifier)])
        .send()?
        .error_for_status()?
        .text()?;

    let token = parse_token_response(&response)?;
    Ok((token.token, token.secret))
}

fn verify_profile(
    client: &Client,
    credentials: &TelldusCredentials,
) -> Result<Option<String>, AuthError> {
    let secrets = Secrets::new(&credentials.public_key, &credentials.private_key)
        .token(&credentials.token, &credentials.token_secret);
    let response = client.clone().oauth1(secrets).get(PROFILE_URL).send()?;

    if response.status() == StatusCode::UNAUTHORIZED {
        return Err(AuthError::Unauthorized);
    }

    let value: Value = response.error_for_status()?.json()?;
    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    if status != "success" {
        return Err(AuthError::VerificationFailed(status.to_string()));
    }

    let account = value.get("user").and_then(|user| {
        let first = user
            .get("firstname")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim();
        let last = user
            .get("lastname")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim();
        let mut composed = String::new();
        if !first.is_empty() {
            composed.push_str(first);
        }
        if !last.is_empty() {
            if !composed.is_empty() {
                composed.push(' ');
            }
            composed.push_str(last);
        }
        if composed.is_empty() {
            user.get("username")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            Some(composed)
        }
    });

    Ok(account)
}

#[derive(Clone)]
struct TempToken {
    token: String,
    secret: String,
}

struct CallbackServer {
    callback_url: String,
    receiver: mpsc::Receiver<Result<String, AuthError>>,
}

impl CallbackServer {
    fn start() -> Result<Self, AuthError> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let callback_url = format!("http://127.0.0.1:{port}/telltales/callback");
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            if let Err(err) = wait_for_callback(listener, tx.clone()) {
                let _ = tx.send(Err(err));
            }
        });

        Ok(Self {
            callback_url,
            receiver: rx,
        })
    }

    fn wait_for_verifier(self, timeout: Duration) -> Result<Option<String>, AuthError> {
        match self.receiver.recv_timeout(timeout) {
            Ok(Ok(v)) => Ok(Some(v)),
            Ok(Err(e)) => Err(e),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => {
                Err(AuthError::CallbackListener("channel closed".into()))
            }
        }
    }
}

fn wait_for_callback(
    listener: TcpListener,
    tx: Sender<Result<String, AuthError>>,
) -> Result<(), AuthError> {
    match listener.accept() {
        Ok((mut stream, _)) => {
            let mut buffer = [0u8; 4096];
            let read = stream.read(&mut buffer)?;
            let request = String::from_utf8_lossy(&buffer[..read]);
            let first_line = request.lines().next().unwrap_or_default();
            let path = first_line.split_whitespace().nth(1).unwrap_or("/");
            let parsed_url = Url::parse(&format!("http://localhost{path}")).map_err(|err| {
                AuthError::CallbackListener(format!("failed to parse redirect URL: {err}"))
            })?;

            if let Some((_, value)) = parsed_url
                .query_pairs()
                .find(|(key, _)| key == "oauth_verifier")
            {
                let verifier = value.into_owned();
                let response_body = "<html><body><h2>Telldus Live authorization complete.</h2>\
<p>You can close this window and return to the Telltales CLI.</p></body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream.write_all(response.as_bytes())?;
                stream.flush()?;
                let _ = tx.send(Ok(verifier));
                Ok(())
            } else {
                let response_body = "<html><body><h2>Authorization error</h2>\
<p>Missing oauth_verifier parameter. Please return to the CLI and try again.</p></body></html>";
                let response = format!(
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream.write_all(response.as_bytes())?;
                stream.flush()?;
                Err(AuthError::VerifierNotFound)
            }
        }
        Err(err) => Err(AuthError::CallbackListener(format!(
            "failed to accept connection: {err}"
        ))),
    }
}

fn extract_verifier(input: &str) -> Result<String, AuthError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AuthError::MissingVerifier);
    }

    if let Ok(url) = Url::parse(trimmed) {
        if let Some(query) = url.query() {
            for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
                if key == "oauth_verifier" {
                    if value.is_empty() {
                        return Err(AuthError::MissingVerifier);
                    }
                    return Ok(value.into_owned());
                }
            }
            return Err(AuthError::VerifierNotFound);
        }
        return Err(AuthError::VerifierNotFound);
    }

    Ok(trimmed.to_owned())
}

fn parse_token_response(body: &str) -> Result<TempToken, AuthError> {
    let data: HashMap<String, String> = serde_urlencoded::from_str(body)?;
    if let Some(problem) = data.get("oauth_problem") {
        if problem == "user_refused" {
            return Err(AuthError::AuthorizationDenied);
        }
        return Err(AuthError::VerificationFailed(problem.clone()));
    }

    let token = data
        .get("oauth_token")
        .cloned()
        .ok_or(AuthError::MissingField("oauth_token"))?;
    let secret = data
        .get("oauth_token_secret")
        .cloned()
        .ok_or(AuthError::MissingField("oauth_token_secret"))?;

    Ok(TempToken { token, secret })
}

use anyhow::Result;
use clap::{App, Arg, SubCommand};
use log::debug;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs::File, io::Write, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    user: Option<User>,
    auth_token: Option<AuthToken>,
    portfolio: Portfolio,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostUserResponse {
    user: User,
    auth_token: AuthToken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetExchangeRateResponse {
    quote: String,
    base: String,
    rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiError {
    code: u16,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Portfolio {
    currencies: Vec<Currency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Currency {
    code: String,
    amount: f64,
}

#[tokio::main]
async fn main() {
    let matches = App::new("pfm")
        .version("0.1.0")
        .author("Igor Bubelov <igor@bubelov.com>")
        .about("Command line client for pfd")
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .subcommand(
            SubCommand::with_name("signup")
                .about("Creates a new user")
                .arg(
                    Arg::with_name("username")
                        .help("Should be unique")
                        .required(true),
                )
                .arg(
                    Arg::with_name("password")
                        .help("Use strong passwords")
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("set")
                .about("Sets asset")
                .arg(
                    Arg::with_name("symbol")
                        .help("Ticker symbol or currency code")
                        .required(true),
                )
                .arg(
                    Arg::with_name("amount")
                        .help("How much units you have")
                        .required(true),
                ),
        )
        .get_matches();

    match matches.occurrences_of("verbose") {
        0 => {}
        1 => env::set_var("RUST_LOG", "info"),
        2 => env::set_var("RUST_LOG", "debug"),
        3 | _ => env::set_var("RUST_LOG", "trace"),
    }

    env_logger::init();

    if env::var("RUST_BACKTRACE").is_err() {
        debug!("RUST_BACKTRACE isn't set, defaulting to \"1\"");
        env::set_var("RUST_BACKTRACE", "1");
    }

    match matches.subcommand() {
        ("signup", Some(matches)) => {
            let username = matches.value_of("username").unwrap();
            let password = matches.value_of("password").unwrap();
            signup(username, password).await.unwrap();
        }
        ("set", Some(matches)) => {
            let symbol = matches.value_of("symbol").unwrap();
            let amount = matches.value_of("amount").unwrap();
            set_currency(symbol, amount).unwrap();
        }
        _ => show_total().await.unwrap(),
    }
}

async fn signup(username: &str, password: &str) -> Result<()> {
    let mut args = HashMap::new();
    args.insert("username", username);
    args.insert("password", password);

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.easyportfol.io/users/")
        .json(&args)
        .send()
        .await?;

    if res.status().is_success() {
        let res: PostUserResponse = res.json().await?;
        println!("Signed up as {}", res.user.username);
        let mut state = load_state()?;
        state.user = Some(res.user.clone());
        state.auth_token = Some(res.auth_token.clone());
        save_state(&state)?;
    } else {
        let error: ApiError = res.json().await?;
        println!("{}", error.message);
    }

    Ok(())
}

fn set_currency(code: &str, amount: &str) -> Result<()> {
    debug!("Setting {} to {}", code, amount);
    let amount = amount.parse::<f64>()?;

    let mut state = load_state()?;
    let currency = Currency {
        code: code.to_string(),
        amount: amount,
    };
    state.portfolio.currencies.push(currency);
    save_state(&state)?;

    Ok(())
}

async fn show_total() -> Result<()> {
    let client = reqwest::Client::new();
    let state = load_state()?;
    let mut total = 0.0;

    println!("Currencies");
    println!("---");

    for currency in state.portfolio.currencies {
        if currency.code.to_lowercase() == "btc".to_string() {
            println!("{}: {:.8}", currency.code, currency.amount);
        } else {
            println!("{}: {:.2}", currency.code, currency.amount);
        }

        let mut headers = HeaderMap::new();
        let header_value =
            HeaderValue::from_str(&format!("Bearer {}", &state.auth_token.clone().unwrap().id))?;
        headers.insert(AUTHORIZATION, header_value);
        let builder = client
            .get(format!(
                "https://api.easyportfol.io/exchange_rates?quote={}&base=USD",
                &currency.code
            ))
            .headers(headers);

        let res = client.execute(builder.build()?).await?;

        if res.status().is_success() {
            let res: GetExchangeRateResponse = res.json().await?;
            total += res.rate * currency.amount;
        } else {
            let error: ApiError = res.json().await?;
            println!("{}", error.message);
        }
    }

    println!("---");
    println!("Total: ${:.2}", total);

    Ok(())
}

fn save_state(state: &State) -> Result<()> {
    let mut file = File::create("state.json")?;
    let json = serde_json::to_string_pretty(state)?;
    write!(file, "{}", json)?;
    Ok(())
}

fn load_state() -> Result<State> {
    let file_path = Path::new("state.json");

    return if file_path.exists() {
        let file = File::open(file_path)?;
        Ok(serde_json::from_reader(file)?)
    } else {
        Ok(State {
            user: None,
            auth_token: None,
            portfolio: Portfolio { currencies: vec![] },
        })
    };
}

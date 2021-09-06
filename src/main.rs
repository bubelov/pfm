use anyhow::Result;
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
    env_logger::init();

    if env::var("RUST_BACKTRACE").is_err() {
        debug!("RUST_BACKTRACE isn't set, defaulting to \"1\"");
        env::set_var("RUST_BACKTRACE", "1");
    }

    let args: Vec<String> = env::args().collect();
    let args = &args[1..];

    match args.first().map(|arg| arg.as_str()) {
        Some("signup") => signup(&args[1..]).await.unwrap(),
        Some("set") => match args.get(1).unwrap_or(&String::new()).as_str() {
            "currency" => set_currency(&args[2..]).await.unwrap(),
            _ => println!("Unknown asset class"),
        },
        Some(_) => println!("Unknown argument"),
        None => show_total().await.unwrap(),
    };
}

async fn signup(args: &[String]) -> Result<()> {
    let username = args.get(0).unwrap();
    let password = args.get(1).unwrap();

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

async fn set_currency(args: &[String]) -> Result<()> {
    let code = args.get(0).unwrap();
    let amount = args.get(1).unwrap().parse::<f64>()?;

    let mut state = load_state()?;
    let currency = Currency {
        code: code.clone(),
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

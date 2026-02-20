pub mod auth;
pub mod env;
pub mod request;
pub mod shared;
pub mod validators;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    Env(env::EnvCommand),
    Auth(auth::AuthCommand),
    Request(request::RequestCommand),
}

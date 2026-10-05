pub mod auth;
pub mod check;
pub mod env;
pub mod request;
pub mod shared;
pub mod validators;
pub mod var;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    Env(env::EnvCommand),
    Auth(auth::AuthCommand),
    Check(check::CheckArgs),
    Request(request::RequestCommand),
    Var(var::VarCommand),
}

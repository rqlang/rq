pub mod auth;
pub mod env;
pub mod ep;
pub mod request;
pub mod shared;
pub mod validators;
pub mod var;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum Commands {
    Env(env::EnvCommand),
    Auth(auth::AuthCommand),
    Ep(ep::EpCommand),
    Request(request::RequestCommand),
    Var(var::VarCommand),
}

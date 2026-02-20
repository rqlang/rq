use super::super::traits::{FunctionContext, RqFunction};
use uuid::Uuid;

pub struct RandomGuid;

impl RqFunction for RandomGuid {
    fn namespace(&self) -> &str {
        "random"
    }

    fn name(&self) -> &str {
        "guid"
    }

    fn execute(&self, _args: &[String], _ctx: &FunctionContext) -> Result<String, String> {
        Ok(Uuid::new_v4().to_string())
    }
}

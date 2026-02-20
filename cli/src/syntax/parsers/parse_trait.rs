use crate::syntax::{error::SyntaxError, parse_result::ParseResult, reader::TokenReader};

pub trait Parse {
    fn can_parse(&self, r: &TokenReader) -> bool;
    fn parse(&self, r: &mut TokenReader, result: &mut ParseResult) -> Result<(), SyntaxError>;
}

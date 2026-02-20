pub const KW_LET: &str = "let";
pub const KW_RQ: &str = "rq";
pub const KW_EP: &str = "ep";
pub const KW_ENV: &str = "env";
pub const KW_AUTH: &str = "auth";
pub const KW_IMPORT: &str = "import";
pub const ALL_KEYWORDS: &[&str] = &[KW_LET, KW_RQ, KW_EP, KW_ENV, KW_AUTH, KW_IMPORT];

pub const PUNC_LBRACE: &str = "{";
pub const PUNC_RBRACE: &str = "}";
#[allow(dead_code)]
pub const PUNC_LPAREN: &str = "(";
#[allow(dead_code)]
pub const PUNC_RPAREN: &str = ")";
pub const PUNC_LBRACKET: &str = "[";
pub const PUNC_RBRACKET: &str = "]";
pub const PUNC_COLON: &str = ":";
pub const PUNC_SEMI: &str = ";";
pub const PUNC_COMMA: &str = ",";
pub const PUNC_DOT: &str = ".";
pub const PUNC_DOLLAR: &str = "$";

pub const OP_ASSIGN: &str = "=";
#[allow(dead_code)]
pub const OP_EQ: &str = "==";
#[allow(dead_code)]
pub const OP_NEQ: &str = "!=";
#[allow(dead_code)]
pub const OP_LT: &str = "<";
#[allow(dead_code)]
pub const OP_LTE: &str = "<=";
#[allow(dead_code)]
pub const OP_GT: &str = ">";
#[allow(dead_code)]
pub const OP_GTE: &str = ">=";

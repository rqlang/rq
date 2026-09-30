#![no_main]

use libfuzzer_sys::fuzz_target;
use rq_lib::syntax::tokenize;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };

    let tokens = match tokenize(source) {
        Ok(tokens) => tokens,
        Err(error) => {
            assert!(
                error.span.start <= error.span.end,
                "inverted error span on {source:?}"
            );
            assert!(
                error.span.end <= source.len(),
                "error span past the end of the input on {source:?}"
            );
            assert!(
                source.is_char_boundary(error.span.start)
                    && source.is_char_boundary(error.span.end),
                "error span off a char boundary on {source:?}"
            );
            assert!(error.line >= 1 && error.column >= 1, "zero-based position");
            return;
        }
    };

    let mut cursor = 0;
    for token in &tokens {
        assert_eq!(
            token.span.start, cursor,
            "gap or overlap between token spans on {source:?}"
        );
        assert!(
            source.is_char_boundary(token.span.start) && source.is_char_boundary(token.span.end),
            "token span off a char boundary on {source:?}"
        );
        assert_eq!(
            &source[token.span.clone()],
            token.value,
            "token value does not match its span on {source:?}"
        );
        cursor = token.span.end;
    }
    assert_eq!(
        cursor,
        source.len(),
        "tokens do not cover the whole input on {source:?}"
    );
});

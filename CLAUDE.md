# RQ Project — Claude Instructions

## Rust Code Style

- Public methods first in impl blocks: constructors → public → private
- Do NOT add comments (`//`) or doc comments (`///`) — names should be self-explanatory
- Keep methods short and focused (~50 lines max); extract complex logic into well-named helpers
- Each method does one thing

### Error Handling
- Use `Result<T, Box<dyn std::error::Error>>` for public APIs with descriptive error messages

### Imports
- Group: `std` → external crates → internal modules
- Import types directly with `use`; avoid fully qualified paths in signatures

## TypeScript Code Style

- Do NOT add comments, JSDoc, or docstrings
- camelCase for functions/variables; PascalCase for classes, interfaces, types, enums

## Testing

- The instance under test must be named `target`

## After Modifying Rust Code in `cli/`

Always run in order:
1. `cargo fmt`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test`

## Architecture

- Separate parsing (syntax layer) from execution (client layer)
- Parse once, reuse results

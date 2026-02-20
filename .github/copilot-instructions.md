# Code Organization Rules for RQ Project

## Rust Code Style

### Method Organization
- Public methods ALWAYS at the top of impl blocks
- Order: constructors → public methods → private methods
- Group related methods together with comments

### Naming Conventions
- Use snake_case for functions and variables
- Use PascalCase for types and traits
- Prefix private helper methods with underscore if they're truly internal utilities

### Documentation
- Do NOT add doc comments (`///`) or regular comments (`//`)
- Method names and parameter names should be self-explanatory
- Code should be self-documenting through clear naming and structure

### Method Length
- Keep methods short and focused (max ~50 lines)
- Extract complex logic into private helper methods
- Each method should do one thing well
- If a method is too long, break it into smaller, well-named functions

### Error Handling
- Use `Result<T, Box<dyn std::error::Error>>` for public APIs
- Provide descriptive error messages with context

### Imports
- Group imports: std → external crates → internal modules
- Remove unused imports
- Prefer importing types directly and using short names
- Avoid fully qualified paths like `crate::module::Type` in function signatures
- Add `use` statements at the top and reference types by their simple name
- Example: prefer `use crate::syntax::variable::VariableContext;` and use `VariableContext` instead of `crate::syntax::variable::VariableContext`

## TypeScript Code Style

### Documentation
- Do NOT add comments in code
- Code should be self-documenting through clear naming and structure
- Do NOT add JSDoc or docstring comments to functions, methods, or classes

### Naming Conventions
- Use camelCase for functions and variables
- Use PascalCase for classes, interfaces, types, and enums

## Architecture Principles
- Separate parsing (syntax layer) from execution (client layer)
- Parse files once, use results multiple times
- Public APIs should be clean and intuitive
- Private helpers should have focused responsibilities

## Testing
- When writing tests, the instance of the class under test must be named `target`.

## CLI Development Workflow
- After modifying Rust code in `cli/`, ALWAYS run the following commands to ensure quality:
  1. `cargo fmt`
  2. `cargo clippy --all-targets --all-features -- -D warnings`
  3. `cargo test`

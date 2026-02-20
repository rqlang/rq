pub mod datetime;
pub mod io;
pub mod random;
pub mod traits;

use lazy_static::lazy_static;
use std::collections::HashMap;
use traits::RqFunction;

lazy_static! {
    static ref FUNCTIONS: HashMap<String, Box<dyn RqFunction>> = {
        let mut m = HashMap::new();
        register(io::read_file::IoReadFile, &mut m);
        register(random::guid::RandomGuid, &mut m);
        register(datetime::now::DateTimeNow, &mut m);
        m
    };
}

fn register<F: RqFunction + 'static>(f: F, m: &mut HashMap<String, Box<dyn RqFunction>>) {
    m.insert(f.full_name(), Box::new(f));
}

pub fn get_function(namespace: &str, name: &str) -> Option<&'static dyn RqFunction> {
    FUNCTIONS
        .get(&format!("{namespace}.{name}"))
        .map(|f| f.as_ref())
}

pub fn is_known_namespace(namespace: &str) -> bool {
    matches!(namespace, "random" | "datetime" | "io")
}

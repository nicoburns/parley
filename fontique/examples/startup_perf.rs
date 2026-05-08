//! Measure fontique collection startup time

use std::time::Instant;

use fontique::{Collection, CollectionOptions};

fn main() {
    let start = Instant::now();

    let collection = Collection::new(CollectionOptions {
        system_fonts: true,
        shared: false,
    });
    std::hint::black_box(collection);

    let time = Instant::now().duration_since(start).as_millis();

    println!("Created collection in {time}ms");
}

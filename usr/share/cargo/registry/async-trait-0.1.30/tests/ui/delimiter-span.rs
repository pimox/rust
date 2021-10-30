use async_trait::async_trait;

macro_rules! picky {
    (ident) => {};
}

#[async_trait]
trait Trait {
    async fn method();
}

struct Struct;

#[async_trait]
impl Trait for Struct {
    async fn method() {
        picky!({ 123 });
    }
}

fn main() {}

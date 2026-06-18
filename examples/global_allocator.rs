use mechanical_mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    let mut values = Vec::with_capacity(16);
    values.extend(0..16);

    let boxed = Box::new(values.iter().sum::<usize>());
    assert_eq!(*boxed, 120);
}

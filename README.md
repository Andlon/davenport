# davenport

`davenport` is a Rust microcrate that provides ergonomic thread-local workspaces for intermediate data.

```rust
use davenport::{define_thread_local_workspace, with_thread_local_workspace};

#[derive(Default)]
pub struct MyWorkspace {
    index_buffer: Vec<usize>
}

define_thread_local_workspace!(WORKSPACE);

fn median_floor(indices: &[usize]) -> Option<usize> {
    with_thread_local_workspace(&WORKSPACE, |workspace: &mut MyWorkspace| {
        // Re-use buffer from previous call to this function
        let buffer = &mut workspace.index_buffer;
        buffer.clear();
        buffer.copy_from_slice(&indices);
        buffer.sort_unstable();
        buffer.get(indices.len() / 2).copied()
    })
}
```

See the [documentation](https://docs.rs/davenport) for an in-depth explanation
of the crate.

# License

Licensed under the terms of both MIT and Apache 2.0 at your option. See `LICENSE-MIT` and `LICENSE-APACHE` for the detailed license text.
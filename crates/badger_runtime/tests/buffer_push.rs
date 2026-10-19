//! Even trusted/native callers must not append to an already-frozen array.
use badger_runtime::{Adapter, Kind, TaskCtx, TypeDesc, buffer, gc, task};

fn entry(adapter: Adapter) -> u64 {
    let desc =
        TypeDesc::new("buffer-push-closure", Kind::Opaque { trace: None, finalize: None, args: vec![] })
            .leak();
    let word = gc::alloc_immortal(desc, 16);
    unsafe {
        *(gc::obj(word).payload().add(8) as *mut usize) = adapter as usize;
    }
    word
}

extern "C" fn mutate_frozen(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    let roots = gc::Roots::new();
    let word = roots.add(buffer::new(badger_runtime::desc::builtin::int(), 1));
    buffer::push(word, 10);
    buffer::freeze(word);
    buffer::push(word, 20);
    0
}

#[test]
fn frozen_append_fails_before_growth_or_mutation() {
    let start = entry(mutate_frozen);
    match task::run_main(start) {
        task::Exit::Failed(info) => assert_eq!(info.message, "append to a frozen buffer"),
        other => panic!("expected frozen-buffer failure: {other:?}"),
    }
}

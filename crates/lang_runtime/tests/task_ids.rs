//! Task ids are never reused: numbering continues past 32 bits (a program
//! that has spawned four billion tasks still tells its tasks apart, and
//! joins the one it means).
use lang_runtime::{Adapter, Kind, TaskCtx, TypeDesc, gc, task};

fn entry(adapter: Adapter) -> u64 {
    let desc =
        TypeDesc::new("ids-closure", Kind::Opaque { trace: None, finalize: None, args: vec![] }).leak();
    let w = gc::alloc_immortal(desc, 16);
    unsafe {
        *(gc::obj(w).payload().add(8) as *mut usize) = adapter as usize;
    }
    w
}

extern "C" fn child(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    task::current_id()
}

extern "C" fn main_task(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    task::set_next_task_id(u32::MAX as u64 - 1);
    let start = entry(child);
    let ids: Vec<_> = (0..4).map(|_| task::spawn_typed(start, None, "child".into(), false)).collect();
    assert_eq!(ids, vec![u32::MAX as u64 - 1, u32::MAX as u64, 1 << 32, (1 << 32) + 1]);
    for &id in &ids {
        task::await_task(id, false);
        assert!(matches!(task::exit_of(id), Some(task::Exit::Returned(r)) if r == id));
    }
    0
}

#[test]
fn ids_continue_past_32_bits() {
    let start = entry(main_task);
    assert!(matches!(task::run_main(start), task::Exit::Returned(0)));
}

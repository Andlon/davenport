use davenport::{define_thread_local_workspace, with_thread_local_workspace, Workspace};

#[derive(Default)]
struct A(usize);

#[derive(Default)]
struct B(usize);

#[test]
fn workspace_consistency() {
    let mut ws = Workspace::default();

    // Request several workspaces and check that results are consistent at all times

    {
        assert!(ws.try_get::<A>().is_none());
        assert!(ws.try_get::<B>().is_none());
        assert!(ws.try_get_mut::<A>().is_none());
        assert!(ws.try_get_mut::<B>().is_none());
        let a_ws: &mut A = ws.get_or_default();
        assert_eq!(a_ws.0, 0);
        a_ws.0 = 2;
        assert_eq!(ws.try_get::<A>().unwrap().0, 2);
        assert_eq!(ws.try_get_mut::<A>().unwrap().0, 2);
    }

    {
        let a_ws: &mut A = ws.get_or_default();
        assert_eq!(a_ws.0, 2);
    }

    {
        let b_ws: &mut B = ws.get_or_default();
        assert_eq!(b_ws.0, 0);
        b_ws.0 = 3;
        assert_eq!(ws.try_get::<B>().unwrap().0, 3);
        assert_eq!(ws.try_get_mut::<B>().unwrap().0, 3);
    }

    {
        let a_ws: &mut A = ws.get_or_default();
        assert_eq!(a_ws.0, 2);
    }

    {
        let b_ws: &mut B = ws.get_or_default();
        assert_eq!(b_ws.0, 3);
    }
}

#[test]
fn workspace_try_insert() {
    {
        // Basic test
        let mut ws = Workspace::default();
        let a = A(3);
        assert_eq!(ws.try_insert(a).unwrap().0, 3);
        assert!(ws.try_insert(A(5)).is_none());
        assert_eq!(ws.get_or_default::<A>().0, 3);
    }

    {
        // Check proper interplay with get_or_* alternative means of insertion
        let mut ws = Workspace::default();
        let _ = ws.get_or_insert_with(|| A(3));
        assert!(ws.try_insert(A(4)).is_none());
        assert_eq!(ws.get_or_default::<A>().0, 3);
    }
}

define_thread_local_workspace!(WORKSPACE);

#[test]
fn with_thread_local_workspace_consistency() {
    let retval = with_thread_local_workspace(&WORKSPACE, |a: &mut A| {
        assert_eq!(a.0, 0);
        a.0 = 1;
        0
    });
    assert_eq!(retval, 0);

    let retval = with_thread_local_workspace(&WORKSPACE, |b: &mut B| {
        assert_eq!(b.0, 0);
        b.0 = 2;
        0
    });
    assert_eq!(retval, 0);

    let retval = with_thread_local_workspace(&WORKSPACE, |a: &mut A| {
        assert_eq!(a.0, 1);
        1
    });
    assert_eq!(retval, 1);

    let retval = with_thread_local_workspace(&WORKSPACE, |b: &mut B| {
        assert_eq!(b.0, 2);
        2
    });
    assert_eq!(retval, 2);
}

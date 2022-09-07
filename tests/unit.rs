use davenport::{define_thread_local_workspace, with_thread_local_workspace, Workspace};

#[test]
fn workspace_consistency() {
    #[derive(Default)]
    struct A(usize);

    #[derive(Default)]
    struct B(usize);
    let mut ws = Workspace::default();

    // Request several workspaces and check that results are consistent at all times

    {
        let a_ws: &mut A = ws.get_or_default();
        assert_eq!(a_ws.0, 0);
        a_ws.0 = 2;
    }

    {
        let a_ws: &mut A = ws.get_or_default();
        assert_eq!(a_ws.0, 2);
    }

    {
        let b_ws: &mut B = ws.get_or_default();
        assert_eq!(b_ws.0, 0);
        b_ws.0 = 3;
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

define_thread_local_workspace!(WORKSPACE);

#[test]
fn with_thread_local_workspace_consistency() {
    #[derive(Default)]
    struct A(usize);

    #[derive(Default)]
    struct B(usize);

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

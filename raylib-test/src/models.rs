#[cfg(test)]
mod model_test {
    use crate::tests::*;
    use raylib::prelude::*;

    ray_test!(test_load_model);
    fn test_load_model(thread: &RaylibThread) {
        let mut handle = TEST_HANDLE.write().unwrap();
        let rl = handle.as_mut().unwrap();
        let _ = rl.load_model(thread, "resources/cube.obj");
        let _ = rl.load_model(thread, "resources/pbr/trooper.obj");
    }

    ray_test!(test_load_meshes);
    fn test_load_meshes(_thread: &RaylibThread) {
        // TODO run this test when Raysan implements LoadMeshes
        // let m = Mesh::load_meshes(thread, "resources/cube.obj").expect("couldn't load any meshes");
    }

    // ray_test!(test_load_anims);

    ray_test!(test_load_anims);
    fn test_load_anims(thread: &RaylibThread) {
        let mut handle = TEST_HANDLE.write().unwrap();
        let rl = handle.as_mut().unwrap();

        let _ = rl
            .load_model_animations(&thread, "resources/guy/guyanim.iqm")
            .expect("could not load model animations");
    }

    ray_test!(test_model_from_generated_mesh);
    fn test_model_from_generated_mesh(thread: &RaylibThread) {
        let mut handle = TEST_HANDLE.write().unwrap();
        let rl = handle.as_mut().unwrap();

        let mesh = Mesh::try_gen_mesh_cube(&thread, 1.0, 1.0, 1.0).unwrap();
        let model = rl.load_model_from_mesh(&thread, mesh).unwrap();

        let zero = Vector3::ZERO;

        let camera = Camera3D::perspective(zero, zero, zero, 10.0);

        let mut d = rl.begin_drawing(&thread);
        let mut world = d.begin_mode3D(&camera);

        world.draw_model(&model, zero, 1.0, Color::RED);
    }
}
// TODO: figure out about the Send and safety stuff that cant happen still
// ray_test!(can_build_raw_mesh_without_thread_and_drop_elsewhere);
// fn can_build_raw_mesh_without_thread_and_drop_elsewhere(_thread: &RaylibThread) {
//     let mesh = Mesh::init_mesh(&[
//         Vector3::new(0.0, 0.0, 0.0),
//         Vector3::new(1.0, 0.0, 0.0),
//         Vector3::new(0.0, 1.0, 0.0),
//     ])
//         .colors(&[Color::RED, Color::GREEN, Color::BLUE])
//         .build_raw()
//         .expect("builder produced an invalid mesh");
//
//     assert_eq!(mesh.as_ref().vaoId, 0);
//     assert!(!mesh.uploaded());
//     assert!(mesh.validate_invariants().is_ok());
//
//     std::thread::spawn(move || drop(mesh)).join().unwrap();
// }
//
// ray_test!(uploaded_mesh_unload_on_owner_thread_via_weak);
// fn uploaded_mesh_unload_on_owner_thread_via_weak(thread: &RaylibThread) {
//     let mut mesh = Mesh::init_mesh(&[
//         Vector3::new(0.0, 0.0, 0.0),
//         Vector3::new(1.0, 0.0, 0.0),
//         Vector3::new(0.0, 1.0, 0.0),
//     ])
//         .build_raw()
//         .unwrap();
//
//     mesh.try_upload_valid(false, thread).unwrap();
//     assert!(mesh.uploaded());
//
//     let mut handle = TEST_HANDLE.write().unwrap();
//     let rl = handle.as_mut().expect("global RaylibHandle not initialized");
//     let weak = unsafe { mesh.make_weak() };
//     let (tx, rx) = mpsc::channel();
//     std::thread::spawn(move || {
//         tx.send(weak).unwrap();
//     })
//         .join()
//         .unwrap();
//
//     let weak_back = rx.recv().unwrap();
//     unsafe { rl.unload_mesh(thread, weak_back) };
// }
//
// ray_test!(build_convenience_does_upload);
// fn build_convenience_does_upload(thread: &RaylibThread) {
//     let mesh = Mesh::init_mesh(&[
//         Vector3::new(0.0, 0.0, 0.0),
//         Vector3::new(1.0, 0.0, 0.0),
//         Vector3::new(0.0, 1.0, 0.0),
//     ])
//         .build(thread)
//         .unwrap();
//     assert!(mesh.uploaded());
//
//     drop(mesh);
// }
//
// ray_test!(invalid_builder_is_rejected);
// fn invalid_builder_is_rejected(_thread: &RaylibThread) {
//     let verts = &[
//         Vector3::new(0.0, 0.0, 0.0),
//         Vector3::new(1.0, 0.0, 0.0),
//         Vector3::new(0.0, 1.0, 0.0),
//         Vector3::new(1.0, 1.0, 0.0),
//     ];
//     let err = Mesh::init_mesh(verts).build_raw().unwrap_err();
//     let _ = err;
//
//     let verts3 = &[
//         Vector3::new(0.0, 0.0, 0.0),
//         Vector3::new(1.0, 0.0, 0.0),
//         Vector3::new(0.0, 1.0, 0.0),
//     ];
//  cdcd rayliqesfasdcd cdcdcdasdf   let bad_indices = &[0u16, 1, 3]; // 3 is OOB
//     let err2 = Mesh::init_mesh(verts3).indices(bad_indices).build_raw().unwrap_err();
//
// }
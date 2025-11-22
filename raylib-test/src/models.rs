#[cfg(test)]
mod model_test {
    use crate::tests::*;
    use raylib::prelude::*;
    use raylib::error::{GenMeshError, InvalidMeshError};

    ray_test!(test_load_model);
    fn test_load_model(thread: &RaylibThread) {
        println!("\nTEST 1 ======= test_load_model ==");
        let mut handle = TEST_HANDLE.write().unwrap();
        let rl = handle.as_mut().unwrap();
        let _ = rl.load_model(thread, "resources/cube.obj");
        let _ = rl.load_model(thread, "resources/pbr/trooper.obj");
        println!("TEST 1 PASSED");
    }

    ray_test!(test_load_meshes);
    fn test_load_meshes(_thread: &RaylibThread) {
        println!("\nTEST 2 ======= test_load_meshes ==");
        // TODO run this test when Raysan implements LoadMeshes
        // let m = Mesh::load_meshes(thread, "resources/cube.obj").expect("couldn't load any meshes");
        println!("TEST 2 PASSED");
    }

    // ray_test!(test_load_anims);

    ray_test!(test_load_anims);
    fn test_load_anims(thread: &RaylibThread) {
        println!("\nTEST 3 ======= test_load_anims ==");
        let mut handle = TEST_HANDLE.write().unwrap();
        let rl = handle.as_mut().unwrap();

        let _ = rl
            .load_model_animations(&thread, "resources/guy/guyanim.iqm")
            .expect("could not load model animations");
        println!("TEST 3 PASSED");
    }

    ray_test!(test_model_from_generated_mesh);
    fn test_model_from_generated_mesh(thread: &RaylibThread) {
        println!("\nTEST 4 ======= test_model_from_generated_mesh ==");
        let mut handle = TEST_HANDLE.write().unwrap();
        let rl = handle.as_mut().unwrap();

        let mesh = unsafe { Mesh::gen_mesh_cube(&thread, 1.0, 1.0, 1.0) };
        let model = rl.load_model_from_mesh(&thread, mesh).unwrap();

        let zero = Vector3::ZERO;

        let camera = Camera3D::perspective(zero, zero, zero, 10.0);

        let mut d = rl.begin_drawing(&thread);
        let mut world = d.begin_mode3D(&camera);

        world.draw_model(&model, zero, 1.0, Color::RED);
        println!("TEST 4 PASSED");
    }

    ray_test!(test_indexed_mesh_triangle_iteration);
    fn test_indexed_mesh_triangle_iteration(thread: &RaylibThread) {
        println!("\nTEST 5 ======= test_indexed_mesh_triangle_iteration ==");
        let vertices = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
        ];
        let indices = vec![0, 1, 2, 1, 3, 2];
        let mesh = Mesh::init_mesh(&vertices)
            .indices(&indices)
            .build(&thread)
            .unwrap();

        let mut triangle_count = 0;
        for [a, b, c] in mesh.triangles() {
            let _v1 = mesh.vertices()[a];
            let _v2 = mesh.vertices()[b];
            let _v3 = mesh.vertices()[c];
            triangle_count += 1;
        }
        const EXPECTED_TRIANGLES: usize = 2;
        println!("Triangle count: {} (expected {})", triangle_count, EXPECTED_TRIANGLES);
        assert_eq!(triangle_count, EXPECTED_TRIANGLES, "Expected {} triangles", EXPECTED_TRIANGLES);
        println!("TEST 5 PASSED");
    }

    ray_test!(test_unindexed_mesh_triangle_iteration);
    fn test_unindexed_mesh_triangle_iteration(thread: &RaylibThread) {
        println!("\nTEST 6 ======= test_unindexed_mesh_triangle_iteration ==");
        let vertices = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ];

        let mesh = Mesh::init_mesh(&vertices)
            .build(&thread)
            .unwrap();

        println!("Has indices: {}", mesh.indices().is_some());
        assert!(
            mesh.indices().is_none(),
            "Unindexed mesh has no indices"
        );
        let triangles: Vec<_> = mesh.triangles().collect();
        const EXPECTED_TRIANGLES: usize = 2;
        const EXPECTED_TRI0: [usize; 3] = [0, 1, 2];
        const EXPECTED_TRI1: [usize; 3] = [3, 4, 5];
        println!("Triangle count: {} (expected {})", triangles.len(), EXPECTED_TRIANGLES);
        println!("Triangle[0]: {:?} (expected {:?})", triangles[0], EXPECTED_TRI0);
        println!("Triangle[1]: {:?} (expected {:?})", triangles[1], EXPECTED_TRI1);
        assert_eq!(triangles.len(), EXPECTED_TRIANGLES, "Expected {} triangles", EXPECTED_TRIANGLES);
        assert_eq!(triangles[0], EXPECTED_TRI0, "First triangle");
        assert_eq!(triangles[1], EXPECTED_TRI1, "Second triangle");
        println!("TEST 6 PASSED");
    }

    ray_test!(test_indexed_mesh_indices_out_of_bounds);
    fn test_indexed_mesh_indices_out_of_bounds(thread: &RaylibThread) {
        println!("\nTEST 7 ======= test_indexed_mesh_indices_out_of_bounds ==");
        let vertices = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        ];
        const BAD_INDEX: u16 = 999;
        let bad_indices = vec![0, 1, BAD_INDEX];

        let result = Mesh::init_mesh(&vertices)
            .indices(&bad_indices)
            .build(&thread);

        println!("Result: {:?}", result);
        assert!(
            matches!(
                result,
                Err(GenMeshError::InvalidMesh(
                    InvalidMeshError::IndexOutOfBounds
                ))
            ),
            "Expected IndexOutOfBounds error got: {:?}",
            result
        );
        println!("TEST 7 PASSED");
    }

    ray_test!(test_unindexed_mesh_triangle_alignment);
    fn test_unindexed_mesh_triangle_alignment(thread: &RaylibThread) {
        println!("\nTEST 8 ======= test_unindexed_mesh_triangle_alignment ==");
        const VERTEX_COUNT: i32 = 12;
        const TRIANGLE_COUNT: i32 = 1;
        let mut vertices: Box<[Vector3]> = (0..VERTEX_COUNT)
            .map(|i| Vector3::new(i as f32, 0.0, 0.0))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        let raw = ffi::Mesh {
            vertexCount: VERTEX_COUNT,
            triangleCount: TRIANGLE_COUNT,
            vertices: vertices.as_mut_ptr().cast(),
            ..Default::default()
        };

        let mesh = unsafe { Mesh::from_raw_unchecked(raw).make_weak() };

        println!("Unindexed mesh: {} vertices, triangleCount={}", VERTEX_COUNT, TRIANGLE_COUNT);
        println!("Triangle iterator length: {}", mesh.triangles().len());
        assert_eq!(mesh.triangles().len(), TRIANGLE_COUNT as usize);

        for [a, b, c] in mesh.triangles() {
            println!("Triangle indices: [{}, {}, {}]", a, b, c);
            let _ = mesh.vertices()[a];
            let _ = mesh.vertices()[b];
            let _ = mesh.vertices()[c];
        }
        println!("TEST 8 PASSED");
    }

    ray_test!(test_indexed_mesh_triangle_alignment);
    fn test_indexed_mesh_triangle_alignment(thread: &RaylibThread) {
        println!("\nTEST 9 ======= test_indexed_mesh_triangle_alignment ==");
        const VERTEX_COUNT: i32 = 8;
        const INDEX_COUNT: usize = 3;
        const TRIANGLE_COUNT: i32 = 1;
        let mut vertices: Box<[Vector3]> = (0..VERTEX_COUNT)
            .map(|i| Vector3::new(i as f32, 0.0, 0.0))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let mut indices: Box<[u16]> = Box::new([0u16, 1, 2]);

        let raw = ffi::Mesh {
            vertexCount: VERTEX_COUNT,
            triangleCount: TRIANGLE_COUNT,
            vertices: vertices.as_mut_ptr().cast(),
            indices: indices.as_mut_ptr(),
            ..Default::default()
        };

        let mesh = unsafe { Mesh::from_raw_unchecked(raw).make_weak() };

        println!("Indexed mesh: {} vertices, {} indices, triangleCount={}", VERTEX_COUNT, INDEX_COUNT, TRIANGLE_COUNT);
        println!("Triangle iterator length: {}", mesh.triangles().len());
        assert_eq!(mesh.triangles().len(), TRIANGLE_COUNT as usize);

        for [a, b, c] in mesh.triangles() {
            println!("Triangle indices: [{}, {}, {}]", a, b, c);
            let _ = mesh.vertices()[a];
            let _ = mesh.vertices()[b];
            let _ = mesh.vertices()[c];
        }
        println!("TEST 9 PASSED");
    }

    ray_test!(test_validate_catches_invalid_patterns);
    fn test_validate_catches_invalid_patterns(thread: &RaylibThread) {
        println!("\n[VALIDATION] validate_mesh() catches all invalid patterns");

        // Pattern 1: Out of bounds index
        const PATTERN1_VERTEX_COUNT: i32 = 3;
        const PATTERN1_TRIANGLE_COUNT: i32 = 1;
        const PATTERN1_BAD_INDEX: u16 = 999;
        let mut verts1 = vec![Vector3::ZERO, Vector3::ZERO, Vector3::ZERO];
        let mut indices1 = vec![0u16, 1, PATTERN1_BAD_INDEX];
        let mesh1 = ffi::Mesh {
            vertexCount: PATTERN1_VERTEX_COUNT,
            triangleCount: PATTERN1_TRIANGLE_COUNT,
            vertices: verts1.as_mut_ptr().cast(),
            indices: indices1.as_mut_ptr(),
            ..Default::default()
        };
        assert!(matches!(
            validate_mesh(&mesh1),
            Err(InvalidMeshError::IndexOutOfBounds)
        ));
        println!("IndexOutOfBounds detected");

        // Pattern 2: vertices Null with positive count
        const PATTERN2_VERTEX_COUNT: i32 = 3;
        const PATTERN2_TRIANGLE_COUNT: i32 = 1;
        let mesh2 = ffi::Mesh {
            vertexCount: PATTERN2_VERTEX_COUNT,
            triangleCount: PATTERN2_TRIANGLE_COUNT,
            vertices: std::ptr::null_mut(),
            ..Default::default()
        };
        assert!(matches!(
            validate_mesh(&mesh2),
            Err(InvalidMeshError::VerticesPointerNull)
        ));
        println!("VerticesPointerNull detected");

        // Pattern 3: Insufficient vertices
        const PATTERN3_VERTEX_COUNT: i32 = 2;
        const PATTERN3_TRIANGLE_COUNT: i32 = 1; // Needs 3 for 1 triangle
        let mut verts3 = vec![Vector3::ZERO, Vector3::ZERO];
        let mesh3 = ffi::Mesh {
            vertexCount: PATTERN3_VERTEX_COUNT,
            triangleCount: PATTERN3_TRIANGLE_COUNT,
            vertices: verts3.as_mut_ptr().cast(),
            indices: std::ptr::null_mut(),
            ..Default::default()
        };
        assert!(matches!(
            validate_mesh(&mesh3),
            Err(InvalidMeshError::VertexCountInsufficient)
        ));
        println!("VertexCountInsufficient detected");

        // Pattern 4: Negative counts?????
        const PATTERN4_VERTEX_COUNT: i32 = -5;
        const PATTERN4_TRIANGLE_COUNT: i32 = -2;
        let mesh4 = ffi::Mesh {
            vertexCount: PATTERN4_VERTEX_COUNT,
            triangleCount: PATTERN4_TRIANGLE_COUNT,
            ..Default::default()
        };
        assert!(matches!(
            validate_mesh(&mesh4),
            Err(InvalidMeshError::NegativeCount)
        ));
        println!("NegativeCount detected");
    }

    ray_test!(test_all_entry_points_validate);
    fn test_all_entry_points_validate(thread: &RaylibThread) {
        println!("\n[VALIDATION] All mesh entry points validate");
        let mut handle = TEST_HANDLE.write().unwrap();
        let rl = handle.as_mut().unwrap();

        // Builder path
        const BAD_INDEX: u16 = 999;
        let builder_result = Mesh::init_mesh(&[Vector3::ZERO; 3])
            .indices(&[0, 1, BAD_INDEX])
            .build(thread);
        assert!(builder_result.is_err(), "builder validates");
        println!("MESH BUILDER VALIDATED");

        let gen_mesh = Mesh::try_gen_mesh_cube(thread, 1.0, 1.0, 1.0)
            .unwrap();
        println!("GEN MESH VALIDATED");

        // File loading path
        let model = rl
            .load_model(thread, "resources/cube.obj")
            .unwrap();
        println!("FILE LOAD MESH VALIDATED");
    }
}
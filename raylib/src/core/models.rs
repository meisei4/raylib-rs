//! 3D Model, Mesh, and Animation

use crate::MintVec3;
use crate::core::databuf::DataBuf;
use crate::core::math::BoundingBox;
use crate::core::math::Matrix;
use crate::core::math::Transform;
use crate::core::math::{Vector2, Vector3, Vector4};
use crate::core::shaders::WeakShader;
use crate::core::texture::{Image, WeakTexture2D};
use crate::core::{RaylibHandle, RaylibThread};
use crate::ffi::Color;
use crate::{
    consts,
    error::{
        AllocationError, GenMeshError, InvalidMeshError, LoadMaterialError, LoadModelAnimError,
        LoadModelError, SetMaterialError,
    },
    ffi,
};
use std::ffi::CString;
use std::os::raw::c_void;
use raylib_sys::{
    RL_DEFAULT_SHADER_ATTRIB_LOCATION_POSITION,
    RL_DEFAULT_SHADER_ATTRIB_LOCATION_TEXCOORD,
    RL_DEFAULT_SHADER_ATTRIB_LOCATION_COLOR,
};
use crate::shaders::Shader;
use crate::texture::Texture2D;

fn no_drop<T>(_thing: T) {}
make_thick_wrapper! {
    /// Model, meshes, materials and animation data
    pub struct Model {
        /// Local transform matrix
        pub transform: Matrix,

        /// Number of meshes
        meshCount: i32,
        /// Number of materials
        materialCount: i32,
        /// Meshes array
        meshes: *mut WeakMesh,
        /// Materials array
        materials: *mut WeakMaterial,
        /// Mesh material number
        meshMaterial: *mut i32,

        // Animation data
        /// Number of bones
        boneCount: i32,
        /// Bones information (skeleton)
        bones: *mut BoneInfo,
        /// Bones base transformation (pose)
        bindPose: *mut Transform,
    }
    weak = WeakModel,
    raw = ffi::Model,
    drop = ffi::UnloadModel,
}
make_thick_wrapper! {
    /// Mesh, vertex data and vao/vbo
    pub struct Mesh {
        /// Number of vertices stored in arrays
        vertexCount: i32,
        /// Number of triangles stored (indexed or not)
        triangleCount: i32,

        // Vertex attributes data
        /// Vertex position (XYZ - 3 components per vertex) (shader-location = 0)
        vertices: *mut Vector3,
        /// Vertex texture coordinates (UV - 2 components per vertex) (shader-location = 1)
        texcoords: *mut Vector2,
        /// Vertex texture second coordinates (UV - 2 components per vertex) (shader-location = 5)
        texcoords2: *mut Vector2,
        /// Vertex normals (XYZ - 3 components per vertex) (shader-location = 2)
        normals: *mut Vector3,
        /// Vertex tangents (XYZW - 4 components per vertex) (shader-location = 4)
        tangents: *mut Vector4,
        /// Vertex colors (RGBA - 4 components per vertex) (shader-location = 3)
        colors: *mut Color,
        /// Vertex indices (in case vertex data comes indexed)
        indices: *mut u16,

        // Animation vertex data
        /// Animated vertex positions (after bones transformations)
        animVertices: *mut Vector3,
        /// Animated normals (after bones transformations)
        animNormals: *mut Vector3,
        /// Vertex bone ids, max 255 bone ids, up to 4 bones influence by vertex (skinning) (shader-location = 6)
        boneIds: *mut u8,
        /// Vertex bone weight, up to 4 bones influence by vertex (skinning) (shader-location = 7)
        boneWeights: *mut f32,
        /// Bones animated transformation matrices
        boneMatrices: *mut Matrix,
        /// Number of bones
        boneCount: i32,

        // OpenGL identifiers
        /// OpenGL Vertex Array Object id
        vaoId: u32,
        /// OpenGL Vertex Buffer Objects id (default vertex data)
        vboId: *mut u32,
    }
    weak = WeakMesh,
    raw = ffi::Mesh,
    drop = ffi::UnloadMesh,
}
make_thick_wrapper! {
    /// Material, includes shader and maps
    pub struct Material {
        /// Material shader
        pub shader: WeakShader,
        /// Material maps array (MAX_MATERIAL_MAPS)
        maps: *mut MaterialMap,
        /// Material generic parameters (if required)
        pub params: [f32; 4],
    }
    weak = WeakMaterial,
    raw = ffi::Material,
    drop = ffi::UnloadMaterial,
}
make_thick_wrapper! {
    /// ModelAnimation
    pub struct ModelAnimation {
        /// Number of bones
        boneCount: i32,
        /// Number of animation frames
        frameCount: i32,
        /// Bones information (skeleton)
        bones: *mut BoneInfo,
        /// Poses array by frame
        framePoses: *mut *mut Transform,
        /// Animation name
        pub name: [::std::os::raw::c_char; 32],
    }
    weak = WeakModelAnimation,
    raw = ffi::ModelAnimation,
    drop = ffi::UnloadModelAnimation,
}
make_thin_wrapper!(
    /// Bone, skeletal animation bone
    BoneInfo,
    ffi::BoneInfo,
    no_drop
);
make_thick_wrapper! {
    /// MaterialMap
    pub struct MaterialMap {
        /// Material map texture
        pub texture: WeakTexture2D,
        /// Material map color
        pub color: Color,
        /// Material map value
        pub value: f32,
    }
    raw = ffi::MaterialMap
}

impl RaylibHandle {
    #[must_use]
    /// Loads model from files (mesh and material).
    // #[inline]
    pub fn load_model(
        &mut self,
        _: &RaylibThread,
        filename: &str,
    ) -> Result<Model, LoadModelError> {
        let c_filename = CString::new(filename).unwrap();
        let m = unsafe { ffi::LoadModel(c_filename.as_ptr()) };
        if m.meshes.is_null() && m.materials.is_null() && m.bones.is_null() && m.bindPose.is_null()
        {
            return Err(LoadModelError::LoadFromFileFailed {
                path: filename.into(),
            });
        }
        // TODO check if null pointer checks are necessary.
        let meshes = unsafe { std::slice::from_raw_parts(m.meshes, m.meshCount as usize) };
        for mesh in meshes {
            validate_mesh(mesh).map_err(|source| {
                LoadModelError::InvalidMeshFromFile {
                    path: filename.into(),
                    source,
                }
            })?;
        }
        Ok(unsafe { Model::from_raw_unchecked(m) })
    }

    #[must_use]
    /// Loads model from a generated mesh
    pub fn load_model_from_mesh(
        &mut self,
        _: &RaylibThread,
        mesh: Mesh,
    ) -> Result<Model, LoadModelError> {
        validate_mesh(mesh.as_raw_ref())?; //TODO: overkill? NEEDS REASONABLE TEST
        let weak_mesh = unsafe { mesh.make_weak() }; //TODO: I would like to consider if asking the user to always call make_weak() for these function calls is ergonomic?
        let m = unsafe { ffi::LoadModelFromMesh(weak_mesh.clone_raw()) };

        if m.meshes.is_null() || m.materials.is_null() || m.meshCount != 1 {
            return Err(LoadModelError::LoadFromMeshFailed);
        }

        let meshes = unsafe { std::slice::from_raw_parts(m.meshes, 1) };
        validate_mesh(&meshes[0])?;
        Ok(unsafe { Model::from_raw_unchecked(m) })
    }

    #[must_use]
    /// Load model animations from file
    pub fn load_model_animations(
        &mut self,
        _: &RaylibThread,
        filename: &str,
    ) -> Result<Vec<ModelAnimation>, LoadModelAnimError> {
        let c_filename = CString::new(filename).unwrap();
        let mut m_size = 0;
        let m_ptr = unsafe { ffi::LoadModelAnimations(c_filename.as_ptr(), &mut m_size) };
        if m_size <= 0 {
            return Err(LoadModelAnimError::NoAnimationsLoaded {
                path: filename.into(),
            });
        }
        let mut m_vec = Vec::with_capacity(m_size as usize);
        for i in 0..m_size {
            unsafe {
                m_vec.push(ModelAnimation::from_raw_unchecked(
                    *m_ptr.offset(i as isize),
                ));
            }
        }
        unsafe {
            ffi::MemFree(m_ptr as *mut ::std::os::raw::c_void);
        }
        Ok(m_vec)
    }

    /// Update model animation pose (CPU)
    #[inline]
    pub fn update_model_animation(
        &mut self,
        _: &RaylibThread,
        model: &mut Model,
        anim: &ModelAnimation,
        frame: i32,
    ) {
        unsafe {
            ffi::UpdateModelAnimation(model.clone_raw(), anim.clone_raw(), frame);
        }
    }

    /// Update model animation mesh bone matrices (GPU skinning)
    #[inline]
    pub fn update_model_animation_bones(
        &mut self,
        _: &RaylibThread,
        model: &mut Model,
        anim: &ModelAnimation,
        frame: i32,
    ) {
        unsafe {
            ffi::UpdateModelAnimationBones(model.clone_raw(), anim.clone_raw(), frame);
        }
    }
}

impl Model {
    #[inline]
    #[must_use]
    /// Local transform matrix
    fn transform(&self) -> &Matrix {
        &self.transform
    }

    #[inline]
    pub fn set_transform(&mut self, mat: &Matrix) {
        self.transform.clone_from(mat);
    }

    /// Meshes array
    #[inline]
    #[must_use]
    pub fn meshes(&self) -> &[WeakMesh] {
        unsafe { std::slice::from_raw_parts(self.meshes, self.meshCount as usize) }
    }
    // Meshes array
    #[inline]
    #[must_use]
    fn meshes_mut(&mut self) -> &mut [WeakMesh] {
        unsafe { std::slice::from_raw_parts_mut(self.meshes, self.meshCount as usize) }
    }
    /// Materials array
    #[inline]
    #[must_use]
    fn materials(&self) -> &[WeakMaterial] {
        unsafe { std::slice::from_raw_parts(self.materials, self.materialCount as usize) }
    }
    /// Materials array
    #[inline]
    #[must_use]
    pub fn materials_mut(&mut self) -> &mut [WeakMaterial] {
        unsafe { std::slice::from_raw_parts_mut(self.materials, self.materialCount as usize) }
    }
    #[inline]
    #[must_use]
    /// Bones information (skeleton)
    fn bones(&self) -> Option<&[BoneInfo]> {
        if self.bones.is_null() {
            return None;
        }

        Some(unsafe { std::slice::from_raw_parts(self.bones, self.boneCount as usize) })
    }
    #[inline]
    #[must_use]
    /// Bones information (skeleton)
    fn bones_mut(&mut self) -> Option<&mut [BoneInfo]> {
        if self.bones.is_null() {
            return None;
        }

        Some(unsafe { std::slice::from_raw_parts_mut(self.bones, self.boneCount as usize) })
    }
    #[inline]
    #[must_use]
    /// Bones base transformation (pose)
    fn bind_pose(&self) -> Option<&Transform> {
        if self.bindPose.is_null() {
            return None;
        }
        Some(unsafe { &*self.bindPose })
    }
    #[inline]
    #[must_use]
    /// Bones base transformation (pose)
    fn bind_pose_mut(&mut self) -> Option<&mut Transform> {
        if self.bindPose.is_null() {
            return None;
        }
        Some(unsafe { &mut *self.bindPose })
    }
    #[inline]
    #[must_use]
    /// Check model animation skeleton match
    fn is_model_animation_valid(&self, anim: &ModelAnimation) -> bool {
        unsafe { ffi::IsModelAnimationValid(self.clone_raw(), anim.clone_raw()) }
    }

    /// Check if a model is ready
    #[inline]
    #[must_use]
    fn is_model_valid(&self) -> bool {
        unsafe { ffi::IsModelValid(self.clone_raw()) }
    }

    /// Compute model bounding box limits (considers all meshes)
    #[inline]
    #[must_use]
    fn get_model_bounding_box(&self) -> BoundingBox {
        unsafe { BoundingBox::from(ffi::GetModelBoundingBox(self.clone_raw())) }
    }
    #[inline]
    /// Set material for a mesh
    fn set_model_mesh_material(
        &mut self,
        mesh_id: i32,
        material_id: i32,
    ) -> Result<(), SetMaterialError> {
        // should this be an assertion?
        if mesh_id >= self.meshCount {
            Err(SetMaterialError::MeshIdOutOfBounds)
        } else if material_id >= self.materialCount {
            Err(SetMaterialError::MaterialIdOutOfBounds)
        } else {
            unsafe { ffi::SetModelMeshMaterial(self.as_raw_mut(), mesh_id, material_id) };
            Ok(())
        }
    }
}

pub struct Triangles<'a> {
    grouping: TriangleGrouping<'a>,
}

enum TriangleGrouping<'a> {
    Indexed(std::slice::ChunksExact<'a, u16>),
    Unindexed { next: usize, last: usize },
}

impl<'a> Iterator for Triangles<'a> {
    type Item = [usize; 3];
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.grouping {
            TriangleGrouping::Indexed(chunk) => chunk
                .next()
                .map(|chunk| [chunk[0] as usize, chunk[1] as usize, chunk[2] as usize]),
            TriangleGrouping::Unindexed { next, last } => {
                if *next + 2 < *last {
                    let triangle = [*next, *next + 1, *next + 2];
                    *next += 3;
                    Some(triangle)
                } else {
                    None
                }
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = match &self.grouping {
            TriangleGrouping::Indexed(chunk) => chunk.len(),
            TriangleGrouping::Unindexed { next, last } => (*last - *next) / 3,
        };
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for Triangles<'a> {
    #[inline]
    fn len(&self) -> usize {
        match &self.grouping {
            TriangleGrouping::Indexed(chunk) => chunk.len(),
            TriangleGrouping::Unindexed { next, last } => (*last - *next) / 3,
        }
    }
}

impl Mesh {
    /// Upload mesh vertex data in GPU and provide VAO/VBO ids
    #[inline]
    pub unsafe fn upload(&mut self, dynamic: bool) {
        unsafe { ffi::UploadMesh(self.as_raw_mut(), dynamic) };
    }
    #[inline]
    fn try_upload_valid(
        &mut self,
        dynamic: bool,
        _t: &RaylibThread,
    ) -> Result<(), InvalidMeshError> {
        validate_mesh(self.as_raw_ref())?;
        unsafe { self.upload(dynamic) };
        Ok(())
    }
    /// Update mesh vertex data in GPU for a specific buffer index
    ///
    /// # Safety
    ///
    /// - The mesh **must** have been uploaded on a backend that supports VBO/VAO (e.g. non OPENGL_11 versions)
    /// - if not uploaded this will usually crashe on OPENGL_11, which is "intentional"? for now to surface incorrect usage -> callers are `unsafe`?
    /// TODO: find a non-misleading way to make this and UploadMesh *informative* no-ops under OPENGL_11 -> remove all the `unsafe`s
    #[inline]
    pub unsafe fn update_buffer(&mut self, index: i32, data: &[u8], offset: i32) {
        if data.is_empty() {
            return;
        }
        unsafe {
            ffi::UpdateMeshBuffer(
                self.clone_raw(),
                index,
                data.as_ptr() as *const c_void,
                data.len() as i32,
                offset,
            )
        };
    }
    #[inline]
    unsafe fn update_position_buffer(&mut self, _: &RaylibThread) {
        let vertices = self.vertices();
        let vertex_count = self.vertexCount as usize; //TODO: this cannot rely on the topology I dont think
        let bytes = unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr() as *const u8,
                vertex_count * std::mem::size_of::<Vector3>(),
            )
        };
        unsafe { self.update_buffer(RL_DEFAULT_SHADER_ATTRIB_LOCATION_POSITION as i32, bytes, 0); }
    }
    #[inline]
    unsafe fn update_texcoord_buffer(&mut self, _: &RaylibThread) {
        if let Some(texcoords) = self.texcoords() {
            let vertex_count = self.vertexCount as usize;
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    texcoords.as_ptr() as *const u8,
                    vertex_count * std::mem::size_of::<Vector2>(),
                )
            };
            unsafe { self.update_buffer(RL_DEFAULT_SHADER_ATTRIB_LOCATION_TEXCOORD as i32, bytes, 0); }
        }
    }
    #[inline]
    unsafe fn update_color_buffer(&mut self, _: &RaylibThread) {
        if let Some(colors) = self.colors() {
            let vertex_count = self.vertexCount as usize;
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    colors.as_ptr() as *const u8,
                    vertex_count * std::mem::size_of::<Color>(),
                )
            };
            unsafe { self.update_buffer(RL_DEFAULT_SHADER_ATTRIB_LOCATION_COLOR as i32, bytes, 0); }
        }
    }
    //TODO: I know this is bad, it was an idea for lifetime scope of validity for mutable  meshes
    // needs to be fixed but i will keep it for reference for my struggles
    /// Vertex count - topologically derived vertex count to ensure bounds safety in mesh attribute slice construction/access
    /// - Does not allow for processing trailing vertices (e.g. vertices outside a multiple of 3)
    #[inline]
    fn vertex_count(&self) -> usize {
        if let Some(indices) = self.indices() {
            // NOTE: if caching initial vertex count (for resize), potentially cache stuff here as well
            indices.iter().max().map(|&m| (m + 1) as usize).unwrap_or(0)
        } else {
            (self.triangleCount as usize) * 3
        }
    }
    /// Safely update the vertex count - validates the mesh after updating to ensure consistency with
    /// triangle count and index buffer
    //TODO: STUDY stdlib Vec!!! shrink and capacity and such
    #[inline]
    fn resize(&mut self, count: usize) -> Result<(), InvalidMeshError> {
        //TODO: this needs more safety with InvalidMeshError::IllegalExpansion, unsure if worth it though
        self.vertexCount = i32::try_from(count)?;
        if !self.is_indexed() {
            self.triangleCount = (count / 3).try_into().map_err(|_| InvalidMeshError::TriangleCountInconsistent)?;
        }
        validate_mesh(self.as_raw_ref())
    }
    #[inline]
    fn resize_sync(&mut self, _t: &RaylibThread, count: usize) -> Result<(), InvalidMeshError> {
        self.resize(count)?;
        unsafe { self.update_position_buffer(_t) };
        Ok(())
    }
    /// Vertex position (XYZ - 3 components per vertex) (shader-location = 0)
    #[inline]
    #[must_use]
    pub fn vertices(&self) -> &[Vector3] {
        //TODO: https://github.com/raylib-rs/raylib-rs/pull/257/files i.e. git diff official-raylib-rs/unstable..amy/mesh-accessor-nulls
        if self.vertexCount == 0 {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.vertices, self.vertexCount as usize) }
    }
    /// Vertex position (XYZ - 3 components per vertex) (shader-location = 0)
    ///
    /// # Safety
    ///
    /// After modifying vertices, the **caller** must ensure:
    /// - If the mesh is indexed: All indices remain valid (< vertexCount)
    /// - If the mesh has been uploaded to GPU: Call `update_position_buffer()` to sync
    /// - Vertex count invariants are maintained
    ///
    /// Violating these requirements may cause undefined behavior in subsequent mesh operations.
    #[inline]
    #[must_use]
    pub unsafe fn vertices_mut(&mut self) -> &mut [Vector3] {
        unsafe {
            std::slice::from_raw_parts_mut(self.vertices, self.vertexCount as usize)
        }
    }
    /// Texture Coordinates (UV (or ST) - 2 components per vertex) (shader-location = 1)
    #[inline]
    #[must_use]
    pub fn texcoords(&self) -> Option<&[Vector2]> {
        if self.texcoords.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts(self.texcoords, self.vertexCount as usize)) }
    }
    /// Texture Coordinates (UV (or ST) - 2 components per vertex) (shader-location = 1)
    #[inline]
    #[must_use]
    pub unsafe fn texcoords_mut(&mut self) -> Option<&mut [Vector2]> {
        if self.texcoords.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts_mut(self.texcoords, self.vertexCount as usize)) }
    }
    pub unsafe fn init_texcoords_mut(&mut self) -> Result<&mut [Vector2], AllocationError> { //TODO: mut is just silly other than a quick way to init for immediate mutability...
        if self.texcoords.is_null() {
            let default_texcoords =
                slice_to_rl_ptr::<Vector2, Vector2>(Some(&vec![Vector2::default(); self.vertexCount as usize]))?;
            self.texcoords = default_texcoords.cast(); //TODO: probably not this AT ALL
        }
        Ok(unsafe { self.texcoords_mut().expect("texcoords must be set") }) }
    /// Vertex normals (XYZ - 3 components per vertex) (shader-location = 2)
    #[inline]
    #[must_use]
    pub fn normals(&self) -> Option<&[Vector3]> {
        if self.normals.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts(self.normals, self.vertexCount as usize)) }
    }
    /// Vertex normals (XYZ - 3 components per vertex) (shader-location = 2)
    #[inline]
    #[must_use]
    pub unsafe fn normals_mut(&mut self) -> Option<&mut [Vector3]> {
        if self.normals.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts_mut(self.normals, self.vertexCount as usize)) }
    }
    /// Vertex colors (RGBA - 4 components per vertex) (shader-location = 3)
    #[inline]
    #[must_use]
    pub fn colors(&self) -> Option<&[Color]> {
        if self.colors.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts(self.colors, self.vertexCount as usize)) }
    }
    /// Vertex colors (RGBA - 4 components per vertex) (shader-location = 3)
    #[inline]
    #[must_use]
    pub unsafe fn colors_mut(&mut self) -> Option<&mut [Color]> {
        if self.colors.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts_mut(self.colors, self.vertexCount as usize)) }
    }
    pub unsafe fn init_colors_mut(&mut self) -> Result<&mut [Color], AllocationError> {
        if self.colors.is_null() {
            let default_colors = slice_to_rl_ptr::<Color, Color>(Some(&vec![Color::WHITE; self.vertexCount as usize]))?;
            self.colors = default_colors.cast(); //TODO again not ideal to even have this function
        }
        Ok(unsafe { self.colors_mut().expect("colors must be set") }) }
    /// Vertex tangents (XYZW - 4 components per vertex) (shader-location = 4)
    #[inline]
    #[must_use]
    pub fn tangents(&self) -> Option<&[Vector4]> {
        if self.tangents.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts(self.tangents, self.vertexCount as usize)) }
    }
    /// Vertex tangents (XYZW - 4 components per vertex) (shader-location = 4)
    #[inline]
    #[must_use]
    pub unsafe fn tangents_mut(&mut self) -> Option<&mut [Vector4]> {
        if self.tangents.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts_mut(self.tangents, self.vertexCount as usize)) }
    }
    /// Texture Coordinates 2 (UV (or ST) - 2 components per vertex) (shader-location = 5)
    #[inline]
    #[must_use]
    pub fn texcoords2(&self) -> Option<&[Vector2]> {
        if self.texcoords2.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts(self.texcoords2, self.vertexCount as usize)) }
    }
    /// Texture Coordinates 2 (UV (or ST) - 2 components per vertex) (shader-location = 5)
    #[inline]
    #[must_use]
    pub unsafe fn texcoords2_mut(&mut self) -> Option<&mut [Vector2]> {
        if self.texcoords2.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts_mut(self.texcoords2, self.vertexCount as usize)) }
    }
    /// Vertex indices (in case vertex data comes indexed) (shader-location = 6)
    #[inline]
    #[must_use]
    pub fn indices(&self) -> Option<&[u16]> {
        if self.indices.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts(self.indices, self.triangleCount as usize * 3)) }
    }
    /// Vertex indices (in case vertex data comes indexed) (shader-location = 6)
    ///
    /// # Safety
    ///
    /// This is **dangerous** if modifying indices, ensure:
    ///  TODO: WIP list of ideas for later!!
    /// - All index values are less than `vertexCount`
    /// - Index buffer length remains `triangleCount * 3`
    /// - If the mesh has been uploaded to GPU: Call `update_index_buffer()` to sync <- TODO MAKE THESE FUNCTIONS later
    ///
    /// **Invalid indices can undefined behavior** when:
    /// - Iterating triangles (e.g. accessing vertices here
    /// - Drawing mesh?
    #[inline]
    #[must_use]
    pub unsafe fn indices_mut(&mut self) -> Option<&mut [u16]> {
        if self.indices.is_null() {
            return None;
        }
        unsafe { Some(std::slice::from_raw_parts_mut(self.indices, self.triangleCount as usize * 3)) }
    }

    #[inline]
    pub fn triangles(&self) -> Triangles<'_> {
        if let Some(indices) = self.indices() {
            Triangles {
                grouping: TriangleGrouping::Indexed(indices.chunks_exact(3)),
            }
        } else {
            let clamped = (self.triangleCount as usize) * 3;
            Triangles {
                grouping: TriangleGrouping::Unindexed {
                    next: 0,
                    last: clamped,
                },
            }
        }
    }

    #[inline]
    fn is_indexed(&self) -> bool {
        !self.indices.is_null()
    }

    #[inline]
    fn triangle_count(&self) -> usize {
        self.triangleCount as usize
    }
    /// Generate polygonal mesh
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_poly")]
    pub fn gen_mesh_poly(_: &RaylibThread, sides: i32, radius: f32) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshPoly(sides, radius)) }
    }

    #[inline]
    fn try_gen_mesh_poly(_: &RaylibThread, sides: i32, radius: f32) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshPoly(sides, radius) };
        try_from_raw(raw_mesh)
    }

    /// Generates plane mesh (with subdivisions).
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_plane")]
    pub fn gen_mesh_plane(
        _: &RaylibThread,
        width: f32,
        length: f32,
        res_x: i32,
        res_z: i32,
    ) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshPlane(width, length, res_x, res_z)) }
    }

    #[inline]
    fn try_gen_mesh_plane(
        _: &RaylibThread,
        width: f32,
        length: f32,
        res_x: i32,
        res_z: i32,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshPlane(width, length, res_x, res_z) };
        try_from_raw(raw_mesh)
    }

    /// Generates cuboid mesh.
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_cube")] //TODO: figure this out if its appropriate even
    pub fn gen_mesh_cube(_: &RaylibThread, width: f32, height: f32, length: f32) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshCube(width, height, length)) }
    }
    #[inline]
    pub fn try_gen_mesh_cube(
        _: &RaylibThread,
        width: f32,
        height: f32,
        length: f32,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshCube(width, height, length) };
        try_from_raw(raw_mesh)
    }

    /// Generates sphere mesh (standard sphere).
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_sphere")]
    pub fn gen_mesh_sphere(_: &RaylibThread, radius: f32, rings: i32, slices: i32) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshSphere(radius, rings, slices)) }
    }

    #[inline]
    pub fn try_gen_mesh_sphere(
        _: &RaylibThread,
        radius: f32,
        rings: i32,
        slices: i32,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshSphere(radius, rings, slices) };
        try_from_raw(raw_mesh)
    }

    /// Generates half-sphere mesh (no bottom cap).
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_hemisphere")]
    pub fn gen_mesh_hemisphere(_: &RaylibThread, radius: f32, rings: i32, slices: i32) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshHemiSphere(radius, rings, slices)) }
    }

    #[inline]
    fn try_gen_mesh_hemisphere(
        _: &RaylibThread,
        radius: f32,
        rings: i32,
        slices: i32,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshHemiSphere(radius, rings, slices) };
        try_from_raw(raw_mesh)
    }

    /// Generates cylinder mesh.
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_cylinder")]
    pub fn gen_mesh_cylinder(_: &RaylibThread, radius: f32, height: f32, slices: i32) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshCylinder(radius, height, slices)) }
    }
    #[inline]
    fn try_gen_mesh_cylinder(
        _: &RaylibThread,
        radius: f32,
        height: f32,
        slices: i32,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshCylinder(radius, height, slices) };
        try_from_raw(raw_mesh)
    }

    /// Generates torus mesh.
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_torus")]
    pub fn gen_mesh_torus(
        _: &RaylibThread,
        radius: f32,
        size: f32,
        rad_seg: i32,
        sides: i32,
    ) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshTorus(radius, size, rad_seg, sides)) }
    }
    #[inline]
    fn try_gen_mesh_torus(
        _: &RaylibThread,
        radius: f32,
        size: f32,
        rad_seg: i32,
        sides: i32,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshTorus(radius, size, rad_seg, sides) };
        try_from_raw(raw_mesh)
    }

    /// Generates trefoil knot mesh.
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_knot")]
    pub fn gen_mesh_knot(
        _: &RaylibThread,
        radius: f32,
        size: f32,
        rad_seg: i32,
        sides: i32,
    ) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshKnot(radius, size, rad_seg, sides)) }
    }
    #[inline]
    fn try_gen_mesh_knot(
        _: &RaylibThread,
        radius: f32,
        size: f32,
        rad_seg: i32,
        sides: i32,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshKnot(radius, size, rad_seg, sides) };
        try_from_raw(raw_mesh)
    }

    /// Generates heightmap mesh from image data.
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_heightmap")]
    pub fn gen_mesh_heightmap(
        _: &RaylibThread,
        heightmap: &Image,
        size: impl Into<MintVec3>,
    ) -> Mesh {
        unsafe {
            Mesh::from_raw_unchecked(ffi::GenMeshHeightmap(heightmap.clone_raw(), size.into()))
        }
    }

    #[inline]
    fn try_gen_mesh_heightmap(
        _: &RaylibThread,
        heightmap: &Image,
        size: impl Into<MintVec3>,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshHeightmap(heightmap.clone_raw(), size.into()) };
        try_from_raw(raw_mesh)
    }

    /// Generates cubes-based map mesh from image data.
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_cubicmap")]
    pub fn gen_mesh_cubicmap(
        _: &RaylibThread,
        cubicmap: &Image,
        cube_size: impl Into<MintVec3>,
    ) -> Mesh {
        unsafe {
            Mesh::from_raw_unchecked(ffi::GenMeshCubicmap(cubicmap.clone_raw(), cube_size.into()))
        }
    }

    #[inline]
    fn try_gen_mesh_cubicmap(
        _: &RaylibThread,
        cubicmap: &Image,
        cube_size: impl Into<MintVec3>,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshCubicmap(cubicmap.clone_raw(), cube_size.into()) };
        try_from_raw(raw_mesh)
    }

    /// Generate cone/pyramid mesh
    #[inline]
    #[must_use]
    #[deprecated(note = "unsound, use try_gen_mesh_cone")]
    pub fn gen_mesh_cone(_: &RaylibThread, radius: f32, height: f32, slices: i32) -> Mesh {
        unsafe { Mesh::from_raw_unchecked(ffi::GenMeshCone(radius, height, slices)) }
    }

    #[inline]
    fn try_gen_mesh_cone(
        _: &RaylibThread,
        radius: f32,
        height: f32,
        slices: i32,
    ) -> Result<Mesh, GenMeshError> {
        let raw_mesh = unsafe { ffi::GenMeshCone(radius, height, slices) };
        try_from_raw(raw_mesh)
    }
    /// Computes mesh bounding box limits.
    #[inline]
    #[must_use]
    pub fn get_mesh_bounding_box(&self) -> BoundingBox {
        unsafe { ffi::GetMeshBoundingBox(self.clone_raw()).into() }
    }

    /// Computes mesh tangents.
    // NOTE: New VBO for tangents is generated at default location and also binded to mesh VAO
    #[inline]
    pub fn gen_mesh_tangents(&mut self, _: &RaylibThread) {
        unsafe {
            ffi::GenMeshTangents(self.as_raw_mut());
        }
    }

    /// Exports mesh as an OBJ file.
    #[inline]
    pub fn export(&self, filename: &str) {
        let c_filename = CString::new(filename).unwrap();
        unsafe {
            ffi::ExportMesh(self.clone_raw(), c_filename.as_ptr());
        }
    }

    /// Export mesh as code file (.h) defining multiple arrays of vertex attributes
    #[inline]
    pub fn export_as_code(&self, filename: &str) {
        let c_filename = CString::new(filename).unwrap();
        unsafe {
            ffi::ExportMeshAsCode(self.clone_raw(), c_filename.as_ptr());
        }
    }
}

fn try_from_raw(raw: ffi::Mesh) -> Result<Mesh, GenMeshError> {
    validate_mesh(&raw)?;
    Ok(unsafe { Mesh::from_raw_unchecked(raw) })
}

#[inline]
pub fn validate_mesh(mesh: &ffi::Mesh) -> Result<(), InvalidMeshError> {
    if mesh.vertexCount < 0 || mesh.triangleCount < 0 {
        return Err(InvalidMeshError::NegativeCount);
    }

    // NOTE: this allows for vertices to be NULL as long as vertexCount is not greater than 0
    if mesh.vertexCount > 0 && mesh.vertices.is_null() {
        return Err(InvalidMeshError::VerticesPointerNull);
    }

    if !mesh.indices.is_null() {
        // INDEXED CASE
        if mesh.triangleCount == 0 {
            return Ok(());
        }

        let index_count = mesh.triangleCount.checked_mul(3).ok_or(InvalidMeshError::TriangleCountInconsistent)?;
        let indices = unsafe { std::slice::from_raw_parts(mesh.indices, index_count as usize) };
        let max_index = indices.iter()
            .max()
            .copied()
            .ok_or(InvalidMeshError::TriangleCountInconsistent)?;

        if max_index as i32 >= mesh.vertexCount {
            return Err(InvalidMeshError::IndexOutOfBounds);
        }
    } else {
        // UNINDEXED CASE
        if mesh.triangleCount > 0 {
            let required_vertices = mesh.triangleCount * 3;
            if mesh.vertexCount < required_vertices {
                return Err(InvalidMeshError::VertexCountInsufficient);
            }
        }
    }

    Ok(())
}

impl Material {
    /// Load materials from model file
    #[must_use]
    pub fn load_materials(filename: &str) -> Result<Vec<Material>, LoadMaterialError> {
        let c_filename = CString::new(filename).unwrap();
        let mut m_size = 0;
        let m_ptr = unsafe { ffi::LoadMaterials(c_filename.as_ptr(), &mut m_size) };
        if m_size <= 0 {
            return Err(LoadMaterialError::NoneLoaded {
                path: filename.into(),
            });
        }
        let mut m_vec = Vec::with_capacity(m_size as usize);
        for i in 0..m_size {
            unsafe {
                m_vec.push(Material::from_raw_unchecked(*m_ptr.offset(i as isize)));
            }
        }
        unsafe {
            ffi::MemFree(m_ptr as *mut ::std::os::raw::c_void);
        }
        Ok(m_vec)
    }

    /// Material shader
    #[must_use]
    #[inline]
    fn shader(&self) -> &crate::shaders::WeakShader {
        unsafe { std::mem::transmute(&self.shader) }
    }
    #[must_use]
    #[inline]
    /// Material shader
    fn shader_mut(&mut self) -> &mut crate::shaders::WeakShader {
        unsafe { std::mem::transmute(&mut self.shader) }
    }
    #[inline]
    pub fn set_shader(&mut self, shader: Shader) {
        let weak_shader = unsafe { shader.make_weak() };
        self.shader = weak_shader;
    }
    #[must_use]
    #[inline]
    /// Material maps array (MAX_MATERIAL_MAPS)
    fn maps(&self) -> &[MaterialMap] {
        unsafe { std::slice::from_raw_parts(self.maps, consts::MAX_MATERIAL_MAPS as usize) }
    }
    #[must_use]
    #[inline]
    /// Material maps array (MAX_MATERIAL_MAPS)
    fn maps_mut(&mut self) -> &mut [MaterialMap] {
        unsafe { std::slice::from_raw_parts_mut(self.maps, consts::MAX_MATERIAL_MAPS as usize) }
    }

    /// Set texture for a material map type (MATERIAL_MAP_DIFFUSE, MATERIAL_MAP_SPECULAR...)
    #[inline]
    pub fn set_material_texture(
        &mut self,
        map_type: crate::consts::MaterialMapIndex,
        texture: &Texture2D,
    ) {
        unsafe {
            ffi::SetMaterialTexture(
                self.as_raw_mut(),
                (map_type as u32) as i32,
                texture.clone_raw(),
            )
        }
    }

    /// Check if a material is valid (shader assigned, map textures loaded in GPU)
    #[inline]
    #[must_use]
    fn is_material_valid(&mut self) -> bool {
        unsafe { ffi::IsMaterialValid(self.clone_raw()) }
    }
}

#[derive(Debug, Clone)]
pub struct FramePoseIter<'a> {
    iter: std::slice::Iter<'a, Option<&'a [Transform]>>,
    bone_count: usize,
}
impl<'a> FramePoseIter<'a> {
    #[must_use]
    unsafe fn new(
        frame_poses: *mut *mut ffi::Transform,
        frame_count: usize,
        bone_count: usize,
    ) -> Self {
        // No new items are being created that get dropped here, these are just changes in perspective of how to borrow-check the pointers.
        assert!(!frame_poses.is_null(), "frame pose array cannot be null");
        assert!(frame_poses.is_aligned(), "frame pose array must be aligned");
        let frame_poses = frame_poses.cast::<Option<&'a [Transform]>>();
        let iter = unsafe { std::slice::from_raw_parts(frame_poses, frame_count) }.iter();
        Self { iter, bone_count }
    }
    fn func(tf: &Option<&'a [Transform]>, bone_count: usize) -> &'a [Transform] {
        unsafe {
            std::slice::from_raw_parts(
                tf.expect("frame pose transform cannot be null").as_ptr(),
                bone_count,
            )
        }
    }
}
impl<'a> Iterator for FramePoseIter<'a> {
    type Item = &'a [Transform];

    fn next(&mut self) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter.next().map(move |tf| Self::func(tf, bone_count))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    fn last(self) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter.last().map(move |tf| Self::func(tf, bone_count))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter.nth(n).map(move |tf| Self::func(tf, bone_count))
    }
}
impl<'a> DoubleEndedIterator for FramePoseIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter
            .next_back()
            .map(move |tf| Self::func(tf, bone_count))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter
            .nth_back(n)
            .map(move |tf| Self::func(tf, bone_count))
    }
}
impl<'a> ExactSizeIterator for FramePoseIter<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}
#[derive(Debug)]
pub struct FramePoseIterMut<'a> {
    iter: std::slice::IterMut<'a, Option<&'a mut [Transform]>>,
    bone_count: usize,
}
impl<'a> FramePoseIterMut<'a> {
    unsafe fn new(
        frame_poses: *mut *mut ffi::Transform,
        frame_count: usize,
        bone_count: usize,
    ) -> Self {
        // No new items are being created that get dropped here, these are just changes in perspective of how to borrow-check the pointers.
        assert!(!frame_poses.is_null(), "frame pose array cannot be null");
        assert!(frame_poses.is_aligned(), "frame pose array must be aligned");
        let frame_poses = frame_poses.cast::<Option<&'a mut [Transform]>>();
        let iter = unsafe { std::slice::from_raw_parts_mut(frame_poses, frame_count) }.iter_mut();
        Self { iter, bone_count }
    }
    fn func(tf: &mut Option<&'a mut [Transform]>, bone_count: usize) -> &'a mut [Transform] {
        unsafe {
            std::slice::from_raw_parts_mut(
                tf.as_mut()
                    .expect("frame pose transform cannot be null")
                    .as_mut_ptr(),
                bone_count,
            )
        }
    }
}
impl<'a> Iterator for FramePoseIterMut<'a> {
    type Item = &'a mut [Transform];

    fn next(&mut self) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter.next().map(move |tf| Self::func(tf, bone_count))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    fn last(self) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter.last().map(move |tf| Self::func(tf, bone_count))
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter.nth(n).map(move |tf| Self::func(tf, bone_count))
    }
}
impl<'a> DoubleEndedIterator for FramePoseIterMut<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter
            .next_back()
            .map(move |tf| Self::func(tf, bone_count))
    }

    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let bone_count = self.bone_count;
        self.iter
            .nth_back(n)
            .map(move |tf| Self::func(tf, bone_count))
    }
}
impl<'a> ExactSizeIterator for FramePoseIterMut<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl ModelAnimation {
    /// Bones information (skeleton)
    #[inline]
    #[must_use]
    fn bones(&self) -> &[BoneInfo] {
        unsafe {
            std::slice::from_raw_parts(self.bones as *const BoneInfo, self.boneCount as usize)
        }
    }

    /// Bones information (skeleton)
    #[inline]
    #[must_use]
    fn bones_mut(&mut self) -> &mut [BoneInfo] {
        unsafe {
            std::slice::from_raw_parts_mut(self.bones as *mut BoneInfo, self.boneCount as usize)
        }
    }

    #[must_use]
    /// Poses array by frame
    fn frame_poses(&self) -> Vec<&[Transform]> {
        let anim = self;
        let mut top = Vec::with_capacity(anim.frameCount as usize);

        for i in 0..anim.frameCount {
            top.push(unsafe {
                std::slice::from_raw_parts(
                    *(anim.framePoses.offset(i as isize) as *const *const Transform),
                    anim.boneCount as usize,
                )
            });
        }

        top
    }
    #[must_use]
    fn frame_poses_iter<'a>(&'a self) -> FramePoseIter<'a> {
        let anim = self;
        unsafe {
            FramePoseIter::new(
                anim.framePoses.cast(),
                anim.frameCount as usize,
                anim.boneCount as usize,
            )
        }
    }

    #[must_use]
    /// Poses array by frame
    fn frame_poses_mut(&mut self) -> Vec<&mut [Transform]> {
        let anim = self;
        let mut top = Vec::with_capacity(anim.frameCount as usize);

        for i in 0..anim.frameCount {
            top.push(unsafe {
                std::slice::from_raw_parts_mut(
                    *(anim.framePoses.offset(i as isize) as *mut *mut Transform),
                    anim.boneCount as usize,
                )
            });
        }

        top
    }
    #[must_use]
    fn frame_poses_iter_mut<'a>(&'a mut self) -> FramePoseIterMut<'a> {
        let anim = self;
        unsafe {
            FramePoseIterMut::new(
                anim.framePoses.cast(),
                anim.frameCount as usize,
                anim.boneCount as usize,
            )
        }
    }
}

impl MaterialMap {
    /// Material map texture
    #[inline]
    #[must_use]
    pub fn texture(&self) -> &crate::texture::WeakTexture2D {
        unsafe { std::mem::transmute(&self.texture) }
    }
    /// Material map texture
    #[inline]
    #[must_use]
    pub fn texture_mut(&mut self) -> &mut crate::texture::WeakTexture2D {
        unsafe { std::mem::transmute(&mut self.texture) }
    }

    /// Material map color
    #[inline]
    #[must_use]
    pub fn color(&self) -> &Color {
        unsafe { std::mem::transmute(&self.color) }
    }
    /// Material map color
    #[inline]
    #[must_use]
    pub fn color_mut(&mut self) -> &mut Color {
        unsafe { std::mem::transmute(&mut self.color) }
    }

    /// Material map value
    #[inline]
    #[must_use]
    pub fn value(&self) -> &f32 {
        unsafe { std::mem::transmute(&self.value) }
    }
    /// Material map value
    #[inline]
    #[must_use]
    pub fn value_mut(&mut self) -> &mut f32 {
        unsafe { std::mem::transmute(&mut self.value) }
    }
}

impl RaylibHandle {
    /// Load default material (Supports: DIFFUSE, SPECULAR, NORMAL maps)
    #[inline]
    #[must_use]
    pub fn load_material_default(&self, _: &RaylibThread) -> WeakMaterial {
        unsafe { Material::from_raw_unchecked(ffi::LoadMaterialDefault()).make_weak() }
    }

    /// Weak materials will leak memory if they are not unlaoded
    /// Unload material from GPU memory (VRAM)
    #[inline]
    pub unsafe fn unload_material(&mut self, _: &RaylibThread, material: WeakMaterial) {
        unsafe { _ = Material::from_weak(material) }
    }

    /// Weak models will leak memory if they are not unlaoded
    /// Unload model from GPU memory (VRAM)
    #[inline]
    pub unsafe fn unload_model(&mut self, _: &RaylibThread, model: WeakModel) {
        unsafe { _ = Model::from_weak(model) }
    }

    /// Weak model_animations will leak memory if they are not unlaoded
    /// Unload model_animation from GPU memory (VRAM)
    #[inline]
    pub unsafe fn unload_model_animation(
        &mut self,
        _: &RaylibThread,
        model_animation: WeakModelAnimation,
    ) {
        unsafe { _ = ModelAnimation::from_weak(model_animation) }
    }

    /// Weak meshs will leak memory if they are not unlaoded
    /// Unload mesh from GPU memory (VRAM)
    #[inline]
    pub unsafe fn unload_mesh(&mut self, _: &RaylibThread, mesh: WeakMesh) {
        unsafe { _ = Mesh::from_weak(mesh) }
    }
}

#[derive(Debug, Clone)]
#[must_use]
pub struct MeshBuilder<'a> {
    /// Vertex position (XYZ - 3 components per vertex)
    vertices: &'a [Vector3],
    /// Vertex texture coordinates (UV - 2 components per vertex)
    texcoords: Option<&'a [Vector2]>,
    /// Vertex texture second coordinates (UV - 2 components per vertex)
    texcoords2: Option<&'a [Vector2]>,
    /// Vertex normals (XYZ - 3 components per vertex)
    normals: Option<&'a [Vector3]>,
    /// Vertex tangents (XYZW - 4 components per vertex)
    tangents: Option<&'a [Vector4]>,
    /// Vertex colors (RGBA - 4 components per vertex)
    colors: Option<&'a [Color]>,
    /// Vertex indices (in case vertex data comes indexed)
    indices: Option<&'a [u16]>,
}

impl Mesh {
    /// Create a new [`MeshBuilder`] to begin generating a custom [`Mesh`].
    /// # Example
    /// ```
    /// # use raylib::prelude::*;
    /// let mesh = Mesh::init_mesh(&[
    ///     Vector3::new(0.0, 0.0, 0.0),
    ///     Vector3::new(1.0, 0.0, 0.0),
    ///     Vector3::new(1.0, 0.0, 1.0),
    /// ])
    /// .texcoords(&[
    ///     Vector2::new(0.0, 0.0),
    ///     Vector2::new(1.0, 0.0),
    ///     Vector2::new(1.0, 1.0),
    /// ])
    /// .normals(&[
    ///     Vector3::new(0.0, 1.0, 0.0),
    ///     Vector3::new(0.0, 1.0, 0.0),
    ///     Vector3::new(0.0, 1.0, 0.0),
    /// ])
    /// .colors(&[
    ///     Color::RED,
    ///     Color::GREEN,
    ///     Color::BLUE,
    /// ])
    /// .build_cpu();
    /// ```
    #[inline]
    pub fn init_mesh<'a>(vertices: &'a [Vector3]) -> MeshBuilder<'a> {
        MeshBuilder::new(vertices)
    }
}

/// Allocate a Raylib-managed pointer to a copy of `[T]` cast to `U` for use in [`ffi::Mesh`].
///
/// This function is safe, but dereferencing the returned pointer may not be.
/// The caller must ensure that `*mut [T]` is safe to dereference as `*mut U`.
fn slice_to_rl_ptr<'a, T: Copy + 'a, U: 'a>(
    data: Option<&'a [T]>,
) -> Result<*mut U, AllocationError> {
    Ok(match data {
        Some(data) => {
            // ok:  {AAAA} -> {AAAA}
            // ok:  {AAAA} -> {AA}{AA}
            // bad: {AAAA} -> {AAAA????}
            assert!(
                std::mem::size_of_val(data) >= std::mem::size_of::<U>(),
                "should not cast to a larger type",
            );
            // ok:  {AAAA} -> {AAAA}
            // ok:  {AAAA} -> {AA}{AA}
            // bad: {AAAA} -> {AAA}{A??}
            assert!(
                (std::mem::size_of_val(data) % std::mem::size_of::<U>()) == 0,
                "should not cast to a type whose size does not evenly divide the source",
            );
            // ok:  {AAAA|BBBB} -> {AA|AA|BB|BB}
            // ok:  {AAAA|BBBB} -> {A|A|A|A|B|B|B|B}
            // bad: {AAAA|BBBB} -> {AAAABBBB|????????}
            assert!(
                (std::mem::align_of::<T>() >= std::mem::align_of::<U>()),
                "should not cast to a type with wider alignment than that of the source",
            );
            // ok:  {AAAA|BBBB} -> {AA|AA}{BB|BB}
            // ok:  {AAAA|BBBB} -> {AA}{AA}{BB}{BB}
            // bad: {AAAA|BBBB} -> {AAA|ABB}{BB?|???}
            assert!(
                (std::mem::align_of::<T>() % std::mem::align_of::<U>()) == 0,
                "should not cast to a type whose alignment does not evenly divide the source alignment",
            );
            DataBuf::<[T]>::alloc_from_copy(data)?
                .into_inner()
                .into_inner()
                .as_ptr()
                .cast::<U>()
        }
        // Raylib accepts null for optional pointer values, so it's ok to provide `null_mut`.
        None => std::ptr::null_mut(),
    })
}

impl<'a> MeshBuilder<'a> {
    /// Construct a [`MeshBuilder`] from its required fields.
    pub fn new(vertices: &'a [Vector3]) -> Self {
        Self {
            vertices,
            texcoords: None,
            texcoords2: None,
            normals: None,
            tangents: None,
            colors: None,
            indices: None,
        }
    }

    /// Give the mesh custom secondary texture coordinates.
    ///
    /// NOTE: `texcoords` should have the same number of elements as `self.vertices`.
    #[inline]
    pub fn texcoords(mut self, texcoords: &'a [Vector2]) -> Self {
        assert!(
            self.texcoords.is_none(),
            "texcoords() should be called no more than once on the same MeshBuilder",
        );
        self.texcoords = Some(texcoords);
        self
    }
    #[inline]
    pub fn texcoords_opt<I>(mut self, texcoords: I) -> Self where I: Into<Option<&'a [Vector2]>> {
        assert!(
            self.texcoords.is_none(),
            "texcoords_opt() should be called no more than once on the same MeshBuilder",
        );
        self.texcoords = texcoords.into();
        self
    }

    /// Give the mesh custom secondary texture coordinates.
    ///
    /// NOTE: `texcoords2` should have the same number of elements as `self.vertices`.
    #[inline]
    pub fn texcoords2(mut self, texcoords2: &'a [Vector2]) -> Self {
        assert!(
            self.texcoords2.is_none(),
            "texcoords2() should be called no more than once on the same MeshBuilder",
        );
        self.texcoords2 = Some(texcoords2);
        self
    }

    /// Give the mesh custom vertex normals.
    ///
    /// NOTE: `normals` should have the same number of elements as `self.vertices`.
    #[inline]
    pub fn normals(mut self, normals: &'a [Vector3]) -> Self {
        assert!(
            self.normals.is_none(),
            "normals() should be called no more than once on the same MeshBuilder",
        );
        self.normals = Some(normals);
        self
    }

    /// Give the mesh custom tangent vectors.
    ///
    /// NOTE: `tangents` should have the same number of elements as `self.vertices`.
    #[inline]
    pub fn tangents(mut self, tangents: &'a [Vector4]) -> Self {
        assert!(
            self.tangents.is_none(),
            "tangents() should be called no more than once on the same MeshBuilder",
        );
        self.tangents = Some(tangents);
        self
    }

    /// Give the mesh custom vertex colors.
    ///
    /// NOTE: `colors` should have the same number of elements as `self.vertices`.
    #[inline]
    pub fn colors(mut self, colors: &'a [Color]) -> Self {
        assert!(
            self.colors.is_none(),
            "colors() should be called no more than once on the same MeshBuilder",
        );
        self.colors = Some(colors);
        self
    }
    #[inline]
    pub fn colors_opt<I>(mut self, colors: I) -> Self where I: Into<Option<&'a [Color]>> {
        assert!(
            self.colors.is_none(),
            "colors_opt() should be called no more than once on the same MeshBuilder",
        );
        self.colors = colors.into();
        self
    }

    /// Give the mesh custom triangle indices.
    ///
    /// NOTE: `indices` should have 3x as many elements as `self.triangle_count`.
    #[inline]
    pub fn indices(mut self, indices: &'a [u16]) -> Self {
        assert!(
            self.indices.is_none(),
            "indices() should be called no more than once on the same MeshBuilder",
        );
        self.indices = Some(indices);
        self
    }
    #[inline]
    pub fn indices_opt<I>(mut self, indices: I) -> Self where I: Into<Option<&'a [u16]>> {
        assert!(
            self.indices.is_none(),
            "indices_opt() should be called no more than once on the same MeshBuilder",
        );
        self.indices = indices.into();
        self
    }

    fn validate_mesh_for_build(&self) -> Result<(usize, usize), InvalidMeshError> {
        let vertex_count = self.vertices.len();
        let triangle_vertex_count = self.indices.map_or(vertex_count, <[_]>::len);
        let triangle_count = triangle_vertex_count / 3;
        let triangle_count_rem = triangle_vertex_count % 3;
        if triangle_count_rem != 0 {
            Err(InvalidMeshError::TriangleNotMultipleOf3)
        } else if self.texcoords.is_some_and(|x| x.len() != vertex_count) {
            Err(InvalidMeshError::TexcoordCountMismatch)
        } else if self.texcoords2.is_some_and(|x| x.len() != vertex_count) {
            Err(InvalidMeshError::Texcoord2CountMismatch)
        } else if self.normals.is_some_and(|x| x.len() != vertex_count) {
            Err(InvalidMeshError::NormalCountMismatch)
        } else if self.tangents.is_some_and(|x| x.len() != vertex_count) {
            Err(InvalidMeshError::TangentCountMismatch)
        } else if self.colors.is_some_and(|x| x.len() != vertex_count) {
            Err(InvalidMeshError::ColorCountMismatch)
        } else if match self.indices {
            Some(indices) => {
                let vertex_count = vertex_count
                    .try_into()
                    .map_err(InvalidMeshError::VertexCountOverflow)?;
                indices.iter().any(|&x| x >= vertex_count)
            }
            None => false,
        } {
            Err(InvalidMeshError::IndexOutOfBounds)
        } else {
            Ok((vertex_count, triangle_count))
        }
    }

    /// Complete the [`Mesh`]
    pub fn build_cpu(self) -> Result<Mesh, GenMeshError> {
        let (vertex_count, triangle_count) = self.validate_mesh_for_build()?;
        let cpu_mesh = ffi::Mesh {
            vertexCount: vertex_count.try_into().unwrap(),
            triangleCount: triangle_count.try_into().unwrap(),
            vertices: slice_to_rl_ptr(Some(self.vertices))?,
            texcoords: slice_to_rl_ptr(self.texcoords)?,
            texcoords2: slice_to_rl_ptr(self.texcoords2)?,
            normals: slice_to_rl_ptr(self.normals)?,
            tangents: slice_to_rl_ptr(self.tangents)?,
            colors: slice_to_rl_ptr(self.colors)?,
            indices: slice_to_rl_ptr(self.indices)?,
            ..Default::default()
        };
        // TODO: remove these comments before merge and make it more clear instead
        // NOTE:
        // - here mesh is constructed entirely CPU-side only
        // - i.e. for GL2.2~3.3 vaoId = 0 and all vboId = 0, so no GL objects exist yet
        // - thus UnloadMesh will only free CPU arrays (DataBuf allocated stuff, no GL/gpu stuff)
        // - Therefore this doesn't depend on the raylib init thread from my understanding:
        let mesh = unsafe { Mesh::from_raw_unchecked(cpu_mesh) };
        Ok(mesh)
    }
    /// build and upload the [`Mesh`].
    pub fn build(self, _: &RaylibThread) -> Result<Mesh, GenMeshError> {
        let mut mesh = self.build_cpu()?;
        // SAFETY: Borrowing `RaylibThread` guarantees this is the thread the resourece was created from,
        // and raw_mesh has no duplicates because it was just created.
        // SAFETY: mesh.vertices are valid, initialized, unique, and safe to dereference.
        unsafe { mesh.upload(false) }
        Ok(mesh)
    }

    pub fn build_dynamic(self, _: &RaylibThread) -> Result<Mesh, GenMeshError> {
        let mut mesh = self.build_cpu()?;
        unsafe { mesh.upload(true) }
        Ok(mesh)
    }
}

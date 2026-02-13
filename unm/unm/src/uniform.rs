#[derive(Debug, PartialEq, Clone, Copy)] // Eq for OrderedFloat, Copy for simple types
pub enum UniformDef {
    F32, // 使用 OrderedFloat
    Vec2, // 需要为数组元素也包一层
    Vec3,
    Vec4,
    Mat4,
}

// ====================================================================
// 新增：UniformLayout 和 UBO 布局计算函数
// = ==================================================================
use std::collections::HashMap;
use std::mem;

// 用于存储每个 Uniform 的偏移量和大小
pub type UniformLayout = HashMap<String, (usize, usize)>; // (offset, size)

/// 根据 WGSL Uniform 缓冲区布局规则，计算给定 `UniformDef` 的大小和对齐要求。
/// 返回 (size_in_bytes, alignment_in_bytes)
pub fn get_uniform_type_info(def: &UniformDef) -> (usize, usize) {
    match def {
        UniformDef::F32 => {
            // f32: size=4, align=4
            (mem::size_of::<f32>(), 4) // WGSL F32 requires 4-byte alignment
        }
        UniformDef::Vec2 => {
            // vec2<f32>: size=8, align=8
            (mem::size_of::<[f32; 2]>(), 8) // WGSL Vec2 requires 8-byte alignment
        }
        UniformDef::Vec3 => {
            // WGSL vec3<f32> 内部通常填充到 vec4<f32> 的大小和对齐
            // 所以，在 UBO 中它占用 16 字节，并需要 16 字节对齐
            (16, 16) // WGSL Vec3 UBO size is 16 bytes, aligned to 16 bytes
        }
        UniformDef::Vec4 => {
            // vec4<f32>: size=16, align=16
            (mem::size_of::<[f32; 4]>(), 16) // WGSL Vec4 requires 16-byte alignment
        }
        UniformDef::Mat4 => {
            // mat4x4<f32>: size=64 (4 * vec4f), align=16 (每列是一个 vec4<f32>)
            (mem::size_of::<[[f32; 4]; 4]>(), 16) // WGSL Mat4 requires 16-byte alignment
        }
    }
}

/// 计算用户所有Uniforms在UBO中的总大小，以及每个Uniform的偏移量和大小。
/// 它会按照统一的规则处理对齐，确保UBO兼容WGSL。
/// 返回 (UniformLayout, total_ubo_size_in_bytes)
pub fn calculate_uniform_offsets_and_total_size(
    uniform_defs: &HashMap<String, UniformDef>,
) -> (UniformLayout, usize) {
    let mut current_offset = 0;
    let mut uniform_offsets = HashMap::new();

    // 为了确保一致的布局，对 uniform_defs 进行排序
    let mut sorted_uniform_names: Vec<&String> = uniform_defs.keys().collect();
    sorted_uniform_names.sort_unstable(); // 按名称排序

    for name in sorted_uniform_names {
        if let Some(def) = uniform_defs.get(name) {
            let (uniform_size, uniform_alignment) = get_uniform_type_info(def);

            // 计算对齐后的偏移量
            let aligned_offset = (current_offset + uniform_alignment - 1) / uniform_alignment * uniform_alignment;

            uniform_offsets.insert(name.clone(), (aligned_offset, uniform_size));
            current_offset = aligned_offset + uniform_size;
        }
    }

    (uniform_offsets, current_offset)
}


// 你可能还需要一个 `Uniform` 枚举来表示实际的 Uniform 值
// 在 `src/uniform.rs` 或一个专门的文件中定义
#[derive(Debug, PartialEq, Clone)]
pub enum Uniform {
    F32(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Mat4([[f32; 4]; 4]),
}

// 帮助函数：将 Uniform 类型转换为字节切片
pub fn uniform_to_bytes(uniform: &Uniform) -> Vec<u8> {
    match uniform {
         Uniform::F32(val) => {
            let temp_array = [*val]; // 创建一个拥有所有权的数组 [f32; 1]
            bytemuck::cast_slice(&temp_array).to_vec() // 现在 `bytemuck::cast_slice` 借用的是 `temp_array`
        },
        Uniform::Vec2(val) => bytemuck::cast_slice(val).to_vec(), // 转换为 Vec<u8>
        Uniform::Vec3(val) => {
            let mut padded = [0.0; 4];
            padded[0..3].copy_from_slice(val);
            bytemuck::cast_slice(&padded).to_vec() // 转换为 Vec<u8>
        },
        Uniform::Vec4(val) => bytemuck::cast_slice(val).to_vec(), // 转换为 Vec<u8>
        Uniform::Mat4(val) => bytemuck::cast_slice(val).to_vec(), // 转换为 Vec<u8>
    }
}

impl UniformDef {
    pub(crate) fn to_uniform_value(&self) -> Uniform {
        match self {
            UniformDef::F32 => Uniform::F32(0.0),
            UniformDef::Vec2 => Uniform::Vec2([0.0; 2]),
            UniformDef::Vec3 => Uniform::Vec3([0.0; 3]),
            UniformDef::Vec4 => Uniform::Vec4([0.0; 4]),
            UniformDef::Mat4 => Uniform::Mat4([[0.0; 4]; 4]),
        }
    }
}
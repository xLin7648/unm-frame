use std::collections::HashMap;
use unm_tools::id_map::IdMap;
use crate::clip::{ClipMap, SfxHandle};

/// 原始解码后的素材，始终保持其物理原始状态，不随设备改变
pub struct RawSource {
    pub data: Box<[f32]>,
    pub sample_rate: u32,
    pub frames_count: usize,
}

pub struct SoundAtlas(Box<[f32]>);

impl SoundAtlas {
    pub fn build_from_sources(
        sources: &IdMap<RawSource, SfxHandle>,
        device_sample_rate: u32
    ) -> (Self, HashMap<SfxHandle, ClipMap>) { // 修改返回类型为 HashMap
        let mut central_data: Vec<f32> = Vec::new();
        let mut clips_temp = Vec::new(); // 临时存储，用于构建 HashMap

        for (handle, source) in sources.iter() {
            // 1. 执行重采样逻辑
            let processed_samples = if source.sample_rate != device_sample_rate {
                Self::perform_resample(source, device_sample_rate)
            } else {
                source.data.to_vec()
            };

            // 2. 内存对齐 (对齐到 16 个 f32 = 64 字节，对 Cache 友好)
            while central_data.len() % 16 != 0 {
                central_data.push(0.0);
            }

            let current_offset = central_data.len();
            let frames = processed_samples.len() / 2;

            // 3. 填入大池子
            central_data.extend(processed_samples);

            // 记录偏移量
            clips_temp.push((handle, current_offset, frames)); // 存储在临时 Vec 中
        }

        // 4. 转换内存所有权至 Box，地址自此固定
        let final_buffer = central_data.into_boxed_slice();
        let base_ptr = final_buffer.as_ptr();

        // 5. 将偏移量转换为原始指针，并构建 HashMap
        let final_clips: HashMap<SfxHandle, ClipMap> = clips_temp // 从临时 Vec 转换
            .into_iter()
            .map(|(handle, offset, frames)| (
                handle, // SfxHandle 作为键
                ClipMap {
                    data_ptr: unsafe { base_ptr.add(offset) },
                    frames_count: frames,
                }
            ))
            .collect(); // 使用 collect 方法直接收集到 HashMap 中

        (SoundAtlas(final_buffer), final_clips)
    }


    /// 重采样逻辑：利用插值计算将 RawSource 转换为 TargetRate 对应的采样序列
    fn perform_resample(source: &RawSource, target_rate: u32) -> Vec<f32> {
        let duration = source.frames_count as f32 / source.sample_rate as f32;
        let target_frames_count = (duration * target_rate as f32).ceil() as usize;
        let mut new_data = Vec::with_capacity(target_frames_count * 2);

        for i in 0..target_frames_count {
            let time = i as f32 / target_rate as f32;
            let (l, r) = Self::lerp_sample_from_raw(source, time);
            new_data.push(l);
            new_data.push(r);
        }
        new_data
    }

    /// 静态采样函数：根据时间点在原始数据中线性插值
    fn lerp_sample_from_raw(source: &RawSource, time: f32) -> (f32, f32) {
        let idxf32 = time * source.sample_rate as f32;
        let idx = idxf32 as usize;
        let fract = idxf32 - idx as f32;

        let curr = Self::get_raw_frame(source, idx);
        let next = Self::get_raw_frame(source, idx + 1);

        // 线性插值: lerp(a, b, t) = a + t * (b - a)
        (
            curr.0 + fract * (next.0 - curr.0),
            curr.1 + fract * (next.1 - curr.1),
        )
    }

    #[inline(always)]
    fn get_raw_frame(source: &RawSource, frame_idx: usize) -> (f32, f32) {
        if frame_idx < source.frames_count {
            let base = frame_idx * 2;
            (source.data[base], source.data[base + 1])
        } else {
            (0.0, 0.0)
        }
    }
}
use std::collections::HashMap;
use unm_tools::id_map::IdMap;
use crate::clip::{ClipMap, SfxHandle};

/// 原始解码后的素材，始终保持其物理原始状态，不随设备改变。
/// 注意：现在data中存储的是单声道数据。
pub struct RawSource {
    pub data: Box<[f32]>,
    pub sample_rate: u32,
    pub frames_count: usize, // 现在每一帧包含1个f32 (单声道)
}

pub struct SoundAtlas(Box<[f32]>);

impl SoundAtlas {
    pub fn build_from_sources(
        sources: &IdMap<RawSource, SfxHandle>,
        device_sample_rate: u32
    ) -> (Self, HashMap<SfxHandle, ClipMap>) {
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
            // 单声道数据，对齐逻辑保持不变，确保块的起始地址是对齐的
            while central_data.len() % 16 != 0 {
                central_data.push(0.0);
            }

            let current_offset = central_data.len();
            // 现在 processed_samples.len() 就是单声道帧数
            let frames = processed_samples.len();

            // 3. 填入大池子
            central_data.extend(processed_samples);

            // 记录偏移量
            clips_temp.push((handle, current_offset, frames)); // 存储在临时 Vec 中
        }

        // 4. 转换内存所有权至 Box，地址自此固定
        let final_buffer = central_data.into_boxed_slice();
        let base_ptr = final_buffer.as_ptr();

        // 5. 将偏移量转换为原始指针，并构建 HashMap
        let final_clips: HashMap<SfxHandle, ClipMap> = clips_temp
            .into_iter()
            .map(|(handle, offset, frames)| (
                handle,
                ClipMap {
                    data_ptr: unsafe { base_ptr.add(offset) },
                    frames_count: frames,
                }
            ))
            .collect();

        (SoundAtlas(final_buffer), final_clips)
    }


    /// 重采样逻辑：利用插值计算将 RawSource 转换为 TargetRate 对应的采样序列 for mono
    fn perform_resample(source: &RawSource, target_rate: u32) -> Vec<f32> {
        let duration = source.frames_count as f32 / source.sample_rate as f32;
        let target_frames_count = (duration * target_rate as f32).ceil() as usize;

        // 因为现在是单声道，所以容量就是 target_frames_count
        let mut new_data = Vec::with_capacity(target_frames_count);

        for i in 0..target_frames_count {
            let time = i as f32 / target_rate as f32;
            let sample = Self::lerp_sample_from_raw(source, time); // 获取单个采样
            new_data.push(sample);
        }
        new_data
    }

    /// 静态采样函数：根据时间点在原始单声道数据中线性插值
    fn lerp_sample_from_raw(source: &RawSource, time: f32) -> f32 {
        let idxf32 = time * source.sample_rate as f32;
        let idx = idxf32 as usize;
        let fract = idxf32 - idx as f32;

        let curr = Self::get_raw_frame(source, idx);
        let next = Self::get_raw_frame(source, idx + 1);

        // 线性插值: lerp(a, b, t) = a + t * (b - a)
        curr + fract * (next - curr)
    }

    #[inline(always)]
    /// 从单声道 RawSource 中获取指定帧的采样值
    fn get_raw_frame(source: &RawSource, frame_idx: usize) -> f32 {
        if frame_idx < source.frames_count {
            // 现在每一帧只包含一个 f32
            source.data[frame_idx]
        } else {
            0.0
        }
    }
}
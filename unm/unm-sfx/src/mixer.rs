use crate::clip::ClipMap;

struct SoundState {
    clip: ClipMap,
    cursor: usize,
}

pub(crate) struct Mixer(Vec<SoundState>);

impl Mixer {
    pub(crate) fn new() -> Self {
        Self(Vec::with_capacity(128))
    }

    pub(crate) fn add_sound(&mut self, clip: ClipMap) {
        self.0.push(SoundState { clip, cursor: 0 });
    }

    pub(crate) fn mix(&mut self, channels: usize, out_data: &mut [f32]) {
        let sounds = &mut self.0;
        if sounds.is_empty() {
            return;
        }

        let out_frames = out_data.len() / channels;
        let out_ptr = out_data.as_mut_ptr();
        let mut i = 0;

        while i < sounds.len() {
            let sound = unsafe { sounds.get_unchecked_mut(i) };
            let mix_frames = out_frames.min(sound.clip.frames_count - sound.cursor);

            if mix_frames == 0 {
                sounds.swap_remove(i);
                continue;
            }

            unsafe {
                // src_ptr 现在直接指向单声道数据
                let src_ptr = sound.clip.data_ptr.add(sound.cursor);

                // 使用 match 优化常见的 channels 数量，兼顾缓存命中率
                match channels {
                    1 => {
                        // 输出单声道：直接将源单声道数据拷贝到目标单声道缓冲区
                        for j in 0..mix_frames {
                            *out_ptr.add(j) += *src_ptr.add(j);
                        }
                    }
                    2 => {
                        // 输出双声道：将源单声道数据拷贝到左右两个声道
                        // 这样访问 out_ptr 是连续的 (L, R, L, R...)
                        for j in 0..mix_frames {
                            let mono_sample = *src_ptr.add(j);
                            let out_base_idx = j * 2;
                            *out_ptr.add(out_base_idx) += mono_sample;     // 左声道
                            *out_ptr.add(out_base_idx + 1) += mono_sample; // 右声道
                        }
                    }
                    // 默认情况：通用处理，可能会有缓存损失，但适用于所有其他声道数
                    _ => {
                        for j in 0..mix_frames {
                            let mono_sample = *src_ptr.add(j);
                            // 确保内层循环是连续访问 out_ptr
                            let out_frame_base_idx = j * channels;
                            for c in 0..channels {
                                *out_ptr.add(out_frame_base_idx + c) += mono_sample;
                            }
                        }
                    }
                }
            }

            sound.cursor += mix_frames;

            if sound.cursor >= sound.clip.frames_count {
                sounds.swap_remove(i);
            } else {
                i += 1;
            }
        }

        for sample in out_data.iter_mut() {
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}
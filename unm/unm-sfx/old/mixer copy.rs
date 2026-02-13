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
        // 获取输出缓冲区的原始指针，避免循环中的 move 问题
        let out_ptr = out_data.as_mut_ptr();
        let mut i = 0;

        while i < sounds.len() {
            // 安全说明：sounds 是私有的，swap_remove 是原地操作，i 始终在有效范围内
            let sound = unsafe { sounds.get_unchecked_mut(i) };
            let mix_frames = out_frames.min(sound.clip.frames_count - sound.cursor);

            unsafe {
                let src_ptr = sound.clip.data_ptr.add(sound.cursor * 2);

                match channels {
                    2 => {
                        // 2声道对2声道：提升缓存命中率的紧凑循环
                        // LLVM 会自动将此编译为 SIMD 指令 (如 ADDPS)
                        for j in 0..(mix_frames * 2) {
                            let out_val = out_ptr.add(j);
                            *out_val += *src_ptr.add(j);
                        }
                    }
                    1 => {
                        // 混音至单声道
                        for j in 0..mix_frames {
                            let s = src_ptr.add(j * 2);
                            let l = *s;
                            let r = *s.add(1);
                            *out_ptr.add(j) += (l + r) * 0.5;
                        }
                    }
                    _ => {
                        // 多声道：仅填充前两个物理通道
                        for j in 0..mix_frames {
                            let dst_base = out_ptr.add(j * channels);
                            let src_base = src_ptr.add(j * 2);
                            *dst_base += *src_base;
                            *dst_base.add(1) += *src_base.add(1);
                        }
                    }
                }
            }

            sound.cursor += mix_frames;

            if sound.cursor >= sound.clip.frames_count {
                // O(1) 删除，不移动后续内存，保持 CPU 缓存友好
                sounds.swap_remove(i);
            } else {
                i += 1;
            }
        }

        for sample in out_data.iter_mut() {
            // clamp 在 Rust 1.50+ 中是内置的
            // 它能确保 sample 维持在 -1.0 到 1.0 之间，防止爆音
            *sample = sample.clamp(-1.0, 1.0);
        }
    }
}

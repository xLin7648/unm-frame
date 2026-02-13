// 标准库导入
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// 第三方 crate 导入
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Producer, Split};
use unm_tools::id_map::IdMap;
use oboe::{AudioOutputCallback, AudioStream, AudioStreamBuilder, DataCallbackResult, PerformanceMode, SharingMode, Usage, AudioStreamSafe, Stereo, AudioStreamBase, AudioStreamAsync, Output, AudioOutputStreamSafe, Error};

// 当前 crate 内部模块导入
use crate::atlas::{RawSource, SoundAtlas};
use crate::backend::AudioBackend;
use crate::clip::SfxHandle;
use crate::decoder;
use crate::mixer::Mixer;
use crate::player::{GLOBAL_ATLAS, GLOBAL_MIXER};

/// Oboe 音频回调结构体
struct OboeCallback(ringbuf::HeapCons<SfxHandle>, Arc<AtomicBool>);

impl AudioOutputCallback for OboeCallback {
    type FrameType = (f32, Stereo);

    fn on_audio_ready(
        &mut self,
        stream: &mut dyn AudioOutputStreamSafe,
        data: &mut [(f32, f32)],
    ) -> DataCallbackResult {
        unsafe {
            let data = unsafe {
                std::slice::from_raw_parts_mut(
                    data.as_mut_ptr() as *mut f32,
                    data.len() * 2
                )
            };

            // 1. 预填零（对应原代码 data.fill(0.0)）
            data.fill(0.0);

            // 2. 检查全局状态（保持原代码逻辑）
            if GLOBAL_MIXER.is_none() || GLOBAL_ATLAS.is_none() {
                return DataCallbackResult::Continue;
            }

            // 使用 unwrap_unchecked 以获得极致性能
            let mixer = GLOBAL_MIXER.as_mut().unwrap_unchecked();
            let atlas = GLOBAL_ATLAS.as_ref().unwrap_unchecked();

            // 3. 无锁消费指令
            while let Some(handle) = self.0.try_pop() {
                if let Some(map) = atlas.1.get(&handle) {
                    mixer.add_sound(*map);
                }
            }

            // 4. 混音处理
            mixer.mix(2, data);
        }

        DataCallbackResult::Continue
    }

    fn on_error_before_close(
        &mut self,
        _audio_stream: &mut dyn AudioOutputStreamSafe,
        _error: Error,
    ) {
        self.1.store(true, Ordering::Release);
    }
}

pub struct Player {
    producer: ringbuf::HeapProd<SfxHandle>,
    consumer: Option<ringbuf::HeapCons<SfxHandle>>,

    stream: Option<AudioStreamAsync<Output, OboeCallback>>,

    device_sample_rate: u32,
    cached_sources: Option<IdMap<RawSource, SfxHandle>>,
    device_lost: Arc<AtomicBool>,
}

impl Player {
    pub(crate) fn new() -> Self {
        let rb = HeapRb::<SfxHandle>::new(128);
        let (prod, cons) = rb.split();

        Self {
            device_sample_rate: 48000, // Android 默认通常为 48k
            cached_sources: None,
            stream: None,

            producer: prod,
            consumer: Some(cons),

            device_lost: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AudioBackend for Player {
    fn maintain_stream(&mut self) {
        if self.device_lost.load(Ordering::Acquire) {
            if let Some(mut s) = self.stream.take() {
                let _ = s.stop(); // 确保回调停止执行
            }

            unsafe {
                GLOBAL_MIXER = None;
                GLOBAL_ATLAS = None;
            }
            self.stream = None;

            let rb = HeapRb::<SfxHandle>::new(128);
            let (prod, cons) = rb.split();
            self.producer = prod;
            self.consumer = Some(cons);

            self.device_lost.store(false, Ordering::Release);
        }

        if self.cached_sources.is_some() && self.stream.is_none() {
            let _ = self.build_stream();
        }
    }

    fn build_stream(&mut self) -> anyhow::Result<()> {
        if self.cached_sources.is_none() {
            return Ok(());
        }

        let mut consumer = self.consumer.take().ok_or_else(|| {
            anyhow::anyhow!("Consumer handle lost - cannot rebuild stream without consumer")
        })?;

        let device_lost_trigger = self.device_lost.clone();
        device_lost_trigger.store(false, Ordering::Release);

        // 使用 Oboe 构建低延迟流
        let mut builder = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_sharing_mode(SharingMode::Exclusive) // 独占模式降低延迟
            .set_usage(Usage::Game)
            .set_channel_count::<Stereo>()
            .set_format::<f32>();

        let mut temp_stream = builder.open_stream().unwrap();
        self.device_sample_rate = temp_stream.get_sample_rate() as u32;

        drop(temp_stream);

        let sources = self.cached_sources.as_ref().unwrap();

        unsafe {
            GLOBAL_MIXER = Some(Mixer::new());
            GLOBAL_ATLAS = Some(SoundAtlas::build_from_sources(
                sources,
                self.device_sample_rate,
            ));
        }

        let mut stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_sharing_mode(SharingMode::Exclusive) // 独占模式降低延迟
            .set_usage(Usage::Game)
            .set_channel_count::<Stereo>()
            .set_format::<f32>()
            .set_callback(OboeCallback(consumer, device_lost_trigger))
            .open_stream()?;

        stream.start()?;
        self.stream = Some(stream);
        Ok(())
    }

    fn init_load_sound(&mut self, datas: Vec<Vec<u8>>) -> Option<Vec<SfxHandle>> {
        let mut sounds: IdMap<RawSource, SfxHandle> = IdMap::<RawSource, SfxHandle>::new();
        for data in datas {
            let data = if let Ok(dasta) = decoder::decode(data) {
                dasta
            } else {
                return None;
            };
            sounds.insert(data);
        }

        let result = sounds.keys().collect();
        self.cached_sources = Some(sounds);
        match self.build_stream() {
            Ok(_) => Some(result),
            Err(_) => None,
        }
    }

    fn play(&mut self, handle: SfxHandle) {
        let _ = self.producer.try_push(handle);
    }
}
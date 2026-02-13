// 标准库导入
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

// 第三方 crate 导入
use ringbuf::{
    HeapRb,
    traits::{Consumer, Producer, Split}
};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use unm_tools::id_map::IdMap;

// 当前 crate 内部模块导入
use crate::atlas::{RawSource, SoundAtlas};
use crate::backend::AudioBackend;
use crate::clip::SfxHandle;
use crate::decoder;
use crate::mixer::Mixer;
use crate::player::{GLOBAL_ATLAS, GLOBAL_MIXER};


pub struct Player {
    producer: ringbuf::HeapProd<SfxHandle>,
    consumer: Option<ringbuf::HeapCons<SfxHandle>>,

    stream: Option<cpal::Stream>,

    device_sample_rate: u32,
    cached_sources: Option<IdMap<RawSource, SfxHandle>>,
    device_lost: Arc<AtomicBool>,
}

impl Player {
     pub(crate) fn new() -> Self {
        let rb = HeapRb::<SfxHandle>::new(128);
        let (prod, cons) = rb.split();

        Self {
            device_sample_rate: 48000,
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

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("No Device"))?;
        let config: cpal::StreamConfig = device.default_output_config()?.into();

        let channels = config.channels as usize;
        self.device_sample_rate = config.sample_rate;

        let mut consumer = self.consumer.take().ok_or_else(|| {
            anyhow::anyhow!("Consumer handle lost - cannot rebuild stream without consumer")
        })?;
        let sources = self.cached_sources.as_ref().unwrap();

        unsafe {
            GLOBAL_MIXER = Some(Mixer::new());
            GLOBAL_ATLAS = Some(SoundAtlas::build_from_sources(
                sources,
                self.device_sample_rate,
            ));
        }

        let device_lost_trigger = self.device_lost.clone();
        device_lost_trigger.store(false, Ordering::Release);

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _| {
                data.fill(0.0);

                unsafe {
                    if GLOBAL_MIXER.is_none() || GLOBAL_ATLAS.is_none() {
                        return;
                    }

                    let mixer = GLOBAL_MIXER.as_mut().unwrap_unchecked();
                    let atlas = GLOBAL_ATLAS.as_ref().unwrap_unchecked();

                    // 1. 无锁消费指令
                    while let Some(handle) = consumer.try_pop() {
                        if let Some(map) = atlas.1.get(&handle) {
                            mixer.add_sound(*map);
                        }
                    }

                    // 2. 混音
                    mixer.mix(channels, data);
                }
            },
            move |_| {
                device_lost_trigger.store(true, Ordering::Release);
            },
            None,
        )?;

        stream.play()?;
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
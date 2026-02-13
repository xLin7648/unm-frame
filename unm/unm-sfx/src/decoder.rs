use symphonia::core::{
    audio::{AudioBufferRef, Signal}, codecs::{CODEC_TYPE_NULL, DecoderOptions}, conv::FromSample, errors::Error, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint
};

use std::io::Cursor;

use crate::atlas::RawSource;

/// 宏：将不同格式的采样混音为单声道并存入 Vec
/// 注意：这里假设需要将所有声道混音到左声道。
macro_rules! fill_interleaved { // 重命名为 fill_mono 或 mix_to_mono 更合适
    ($audio_buf:expr, $out_data:expr) => {{
        let frames = $audio_buf.frames();
        let chan_count = $audio_buf.spec().channels.count();

        // 遍历所有帧
        for i in 0..frames {
            let mut mixed_sample: f32 = 0.0;
            // 遍历所有声道，并求和进行平均混音
            for c in 0..chan_count {
                mixed_sample += f32::from_sample($audio_buf.chan(c)[i]);
            }
            // 将所有声道求和后平均，得到单声道采样
            $out_data.push(mixed_sample / chan_count as f32);
        }
    }};
}

pub(crate) fn decode(data: Vec<u8>) -> anyhow::Result<RawSource> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());

    let probed = symphonia::default::get_probe()
        .format(&Hint::new(), mss, &FormatOptions::default(), &MetadataOptions::default())
        .expect("不支持的音频格式");

    let mut format = probed.format;

    let track = format.tracks().iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL && t.codec_params.sample_rate.is_some())
        .expect("未找到音频轨道");

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .expect("无法创建解码器");

    let sample_rate = track.codec_params.sample_rate.unwrap_or(48000);
    let track_id = track.id;

    // 存储混音后的单声道数据
    let mut mono_data = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(ref err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                // 正常读取完毕，跳出循环
                break;
            }
            Err(err) => {
                return Err(err.into());
            }
        };

        if packet.track_id() != track_id { continue; }

        if let Ok(decoded) = decoder.decode(&packet) {
            match decoded {
                AudioBufferRef::F32(buf) => {
                    let frames = buf.frames();
                    let chan_count = buf.spec().channels.count();
                    // 处理 F32 数据，混音为单声道
                    for i in 0..frames {
                        let mut mixed_sample: f32 = 0.0;
                        for c in 0..chan_count {
                            mixed_sample += buf.chan(c)[i];
                        }
                        mono_data.push(mixed_sample / chan_count as f32);
                    }
                }
                // 其他格式通过宏转换并混音
                AudioBufferRef::U8(buf) => fill_interleaved!(buf, mono_data),
                AudioBufferRef::U16(buf) => fill_interleaved!(buf, mono_data),
                AudioBufferRef::U24(buf) => fill_interleaved!(buf, mono_data),
                AudioBufferRef::U32(buf) => fill_interleaved!(buf, mono_data),
                AudioBufferRef::S8(buf) => fill_interleaved!(buf, mono_data),
                AudioBufferRef::S16(buf) => fill_interleaved!(buf, mono_data),
                AudioBufferRef::S24(buf) => fill_interleaved!(buf, mono_data),
                AudioBufferRef::S32(buf) => fill_interleaved!(buf, mono_data),
                AudioBufferRef::F64(buf) => fill_interleaved!(buf, mono_data),
            }
        }
    }

    // 现在 mono_data 存储的是单声道数据，frames_count 直接等于其长度
    let frames_count = mono_data.len();
    let data: Box<[f32]> = mono_data.into_boxed_slice();

    Ok(RawSource {
        data,
        sample_rate,
        frames_count
    })
}
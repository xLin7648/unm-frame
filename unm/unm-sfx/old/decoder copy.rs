use symphonia::core::{
    audio::{AudioBufferRef, Signal}, codecs::{CODEC_TYPE_NULL, DecoderOptions}, conv::FromSample, errors::Error, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint
};

use std::io::Cursor;

use crate::atlas::RawSource;

/// 宏：将不同格式的采样交错化并存入 Vec
macro_rules! fill_interleaved {
    ($audio_buf:expr, $out_data:expr) => {{
        let frames = $audio_buf.frames();
        let chan_count = $audio_buf.spec().channels.count();

        if chan_count >= 2 {
            let l_chan = $audio_buf.chan(0);
            let r_chan = $audio_buf.chan(1);
            for i in 0..frames {
                $out_data.push(f32::from_sample(l_chan[i]));
                $out_data.push(f32::from_sample(r_chan[i]));
            }
        } else if chan_count == 1 {
            let l_chan = $audio_buf.chan(0);
            for i in 0..frames {
                let s = f32::from_sample(l_chan[i]);
                $out_data.push(s); // 左
                $out_data.push(s); // 右 (单声道转立体声)
            }
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

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let track_id = track.id;

    // 存储交错后的数据
    let mut interleaved_data = Vec::new();

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
                    // 针对 F32 的快速处理
                    let frames = buf.frames();
                    let chan_count = buf.spec().channels.count();
                    let l = buf.chan(0);
                    if chan_count >= 2 {
                        let r = buf.chan(1);
                        for i in 0..frames {
                            interleaved_data.push(l[i]);
                            interleaved_data.push(r[i]);
                        }
                    } else {
                        for i in 0..frames {
                            interleaved_data.push(l[i]);
                            interleaved_data.push(l[i]);
                        }
                    }
                }
                // 其他格式通过宏转换并交错
                AudioBufferRef::U8(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::U16(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::U24(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::U32(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::S8(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::S16(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::S24(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::S32(buf) => fill_interleaved!(buf, interleaved_data),
                AudioBufferRef::F64(buf) => fill_interleaved!(buf, interleaved_data),
            }
        }
    }

    let frames_count = interleaved_data.len() / 2;
    let data: Box<[f32]> = interleaved_data.into_boxed_slice();

    Ok(RawSource {
        data,
        sample_rate,
        frames_count
    })
}
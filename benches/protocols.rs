#![expect(clippy::unwrap_used)]
use criterion::{Criterion, criterion_group, criterion_main};
use ratatui::{buffer::Buffer, layout::Rect, widgets::StatefulWidget as _};
use ratatui_image::{
    Resize, ResizeEncodeRender, StatefulImage,
    picker::{Picker, ProtocolType},
};
use std::hint::black_box;

fn bench_resize_encode_render(c: &mut Criterion) {
    let image = image::ImageReader::open("./assets/Ada.png")
        .unwrap()
        .decode()
        .unwrap();

    let protocol_types = [
        ProtocolType::Sixel,
        ProtocolType::Kitty,
        ProtocolType::Iterm2,
        ProtocolType::Halfblocks,
    ];

    for &protocol in &protocol_types {
        let protocol_str = match protocol {
            ProtocolType::Sixel => "sixel",
            ProtocolType::Kitty => "kitty",
            ProtocolType::Iterm2 => "iterm2",
            ProtocolType::Halfblocks => "halfblocks",
        };
        c.bench_function(&format!("resize_encode_render_{protocol_str}"), |b| {
            b.iter(|| {
                let mut picker = Picker::halfblocks();
                picker.set_protocol_type(protocol);
                let mut proto = picker.new_resize_protocol(black_box(image.clone()));

                let area = Rect::new(0, 0, 10, 10);
                let buf = Buffer::empty(area);

                let image = StatefulImage::default();
                image.render(black_box(area), &mut black_box(buf), &mut proto);
            })
        });

        c.bench_function(&format!("resize_encode_{protocol_str}"), |b| {
            b.iter(|| {
                let mut picker = Picker::halfblocks();
                picker.set_protocol_type(protocol);
                let mut proto = picker.new_resize_protocol(black_box(image.clone()));

                let area = Rect::new(0, 0, 10, 10);
                proto.resize_encode(&Resize::Fit(None), black_box(area));
            })
        });
    }
}

fn bench_config() -> Criterion {
    Criterion::default()
        .measurement_time(std::time::Duration::from_secs(30))
        .warm_up_time(std::time::Duration::from_secs(5))
        .sample_size(250)
        .nresamples(10000)
        .configure_from_args()
}

criterion_group! {
    name = benches;
    config = bench_config();
    targets = bench_resize_encode_render
}
criterion_main!(benches);

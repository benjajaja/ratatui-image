use criterion::{criterion_group, criterion_main, Criterion};
use image::{DynamicImage, ImageBuffer};
use ratatui::layout::Rect;
use ratatui_image::{picker::*, Resize};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("encode kitty", |b| {
        let (picker, dyn_img, area) = make_picker(ProtocolType::Kitty);
        b.iter(|| picker.new_protocol(dyn_img.clone(), area, Resize::default()))
    });
    c.bench_function("encode sixel", |b| {
        let (picker, dyn_img, area) = make_picker(ProtocolType::Sixel);
        b.iter(|| picker.new_protocol(dyn_img.clone(), area, Resize::default()))
    });
    c.bench_function("encode iterm2", |b| {
        let (picker, dyn_img, area) = make_picker(ProtocolType::Iterm2);
        b.iter(|| picker.new_protocol(dyn_img.clone(), area, Resize::default()))
    });
    c.bench_function("encode halfblocks", |b| {
        let (picker, dyn_img, area) = make_picker(ProtocolType::Halfblocks);
        b.iter(|| picker.new_protocol(dyn_img.clone(), area, Resize::default()))
    });
}

fn make_picker(proto: ProtocolType) -> (Picker, DynamicImage, Rect) {
    let mut picker = Picker::from_fontsize((8, 16));
    picker.set_protocol_type(proto);

    let area = Rect::new(0, 0, 10, 10);

    let img = ImageBuffer::new(100, 100);
    let dyn_img = DynamicImage::ImageRgba8(img);
    (picker, dyn_img, area)
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

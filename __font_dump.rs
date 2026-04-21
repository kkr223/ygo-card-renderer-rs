use std::io::Cursor;
use ygo_card_renderer_rs::asset_bundle::get_bundle;

fn main() {
    let bundle = get_bundle();
    for name in ["ygo-atk-def", "ygo-link", "ygo-password"] {
        for (key, meta) in &bundle.index.fonts {
            if key.contains(name) || meta.buffer.contains(name) {
                println!("key={key} buffer={} family={:?}", meta.buffer, meta.family);
            }
        }
    }
}

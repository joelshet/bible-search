//! The browser's view of the engine. Everything crosses as JSON strings or typed arrays.

use crate::index::Bible;
use crate::search::Results;
use crate::{json, layout};
use qrcodegen::{QrCode, QrCodeEcc};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Engine {
    bible: Bible,
    last: Option<Results>,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new(core: &[u8]) -> Result<Engine, String> {
        Ok(Engine { bible: Bible::decode(core)?, last: None })
    }

    pub fn load_lexicon(&mut self, data: &[u8]) -> Result<(), String> {
        self.bible.load_lexicon(data)
    }

    /// Run a query and keep its results for `page`, `hits`, and `chapter`.
    pub fn search(&mut self, q: &str, live: bool, canonical: bool) -> String {
        let r = self.bible.search(q, live, canonical);
        let out = json::summary(&self.bible, &r, 0.0);
        self.last = Some(r);
        out
    }

    pub fn clear(&mut self) {
        self.last = None;
    }

    pub fn page(&self, offset: usize, limit: usize) -> String {
        self.last.as_ref().map_or("[]".into(), |r| json::page(&self.bible, r, offset, limit))
    }

    /// One byte per verse: 0 for no match, otherwise match strength 1-255.
    pub fn hits(&self) -> Vec<u8> {
        self.last.as_ref().map_or_else(|| vec![0; self.bible.len()], |r| r.hits.clone())
    }

    pub fn chapter(&self, verse: usize) -> String {
        if verse >= self.bible.len() {
            return "null".into();
        }
        json::chapter(&self.bible, verse, self.last.as_ref())
    }

    pub fn strongs(&self, code: &str) -> String {
        json::strongs(&self.bible, code)
    }

    /// The map for a space `aspect` times wider than it is tall.
    pub fn layout(&self, aspect: f64) -> Vec<f32> {
        layout::layout(&self.bible, aspect.clamp(0.5, 3.0))
    }

    pub fn meta(&self) -> String {
        json::meta(&self.bible)
    }

    /// SVG for a QR code of `text`, one unit per module plus a 4-module quiet zone.
    pub fn qr(text: &str) -> String {
        let Ok(qr) = QrCode::encode_text(text, QrCodeEcc::Medium) else { return String::new() };
        let n = qr.size();
        let mut path = String::new();
        for y in 0..n {
            for x in 0..n {
                if qr.get_module(x, y) {
                    path.push_str(&format!("M{},{}h1v1h-1z", x + 4, y + 4));
                }
            }
        }
        let s = n + 8;
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {s} {s}\" shape-rendering=\"crispEdges\"><rect width=\"{s}\" height=\"{s}\" fill=\"#fff\"/><path d=\"{path}\" fill=\"#000\"/></svg>"
        )
    }
}

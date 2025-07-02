use percent_encoding::AsciiSet;
use percent_encoding::CONTROLS;

pub mod github;
pub mod gitlab;
pub mod orama;

/// https://url.spec.whatwg.org/#fragment-percent-encode-set
const FRAGMENT: &AsciiSet =
  &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

/// https://url.spec.whatwg.org/#path-percent-encode-set
const PATH: &AsciiSet = &FRAGMENT.add(b'#').add(b'?').add(b'{').add(b'}');

fn sanitize_url_part(part: &str) -> String {
  percent_encoding::percent_encode(part.as_bytes(), PATH).to_string()
}

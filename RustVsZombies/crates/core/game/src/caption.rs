//! One-shot native hints. Text is copied by the game; no script allocation or
//! callback is retained by the message widget after this call.
use crate::runtime::{RuntimeError, RuntimeResult};
use rsvz_backend_api::HintCaptionBackend;
use rsvz_current::CurrentBackend;

/// Display literal text with A-TAS's bottom-fast native style for `duration`
/// native updates. Headless hosts validate the same parameters, then do nothing.
pub fn hint(text: &str, duration: u32) -> RuntimeResult<()>
where
    CurrentBackend: HintCaptionBackend,
{
    let caption = rsvz_model::HintCaption::new(text, duration).map_err(RuntimeError::new)?;
    if CurrentBackend::HINT_CAPTION_IS_NOOP {
        return Ok(());
    }
    crate::access::with_exclusive_backend(|backend| backend.show_hint_caption(caption))
        .map_err(crate::access::operation_error)
}

pub type CommitCallback = Box<dyn Fn(String)>;

pub struct PreEdit {
    /// The text currently being composed.
    pub text: String,
    /// The position of the cursor within the preedit text.
    pub cursor: usize,
}

pub enum PreEditEvent {
    /// Preedit text has started.
    Begin,
    /// Preedit text has been updated.
    Update(PreEdit),
    /// Preedit text has ended.
    End,
}
pub type PreEditCallback = Box<dyn Fn(PreEditEvent)>;

pub struct Candidate {
    /// The text of the candidate.
    pub candidates: Vec<String>,
    /// The index of the currently selected candidate.
    pub selected: usize,
}

pub enum CandidateEvent {
    /// Candidate list has started.
    Begin,
    /// Candidate list has been updated.
    Update(Candidate),
    /// Candidate list has ended.
    End,
}
pub type CandidateCallback = Box<dyn Fn(CandidateEvent)>;

/// Configuration for candidate list display
#[derive(Clone)]
pub struct CandidateConfig {
    /// Maximum number of candidates to display per page
    pub max_candidates: usize,
}

impl Default for CandidateConfig {
    fn default() -> Self {
        Self {
            max_candidates: 9,
        }
    }
}

pub struct InputSource {
    /// The unique identifier for the input source.
    pub name: String,
    /// The localized name of the input source.
    pub localized_name: String,
    /// The locale associated with the input source.
    pub locale: String,
    /// The localized locale name associated with the input source.
    pub localized_locale: String,
}

pub enum InputSourceInfo {
    /// The input source info is not supported.
    Unsupported,
    Supported(InputSource),
}
pub type InputSourceCallback = Box<dyn Fn(InputSourceInfo)>;

pub enum InputMode {
    /// Input mode info is not supported.
    Unsupported,
    /// Input mode for typing in English.
    Alpha,
    /// Input mode for typing in another language.
    Native,
}
pub type InputModeCallback = Box<dyn Fn(InputMode)>;

pub trait InputContext {
    /// Retrieves active input method
    fn get_input_source(&self) -> InputSourceInfo;

    /// Retrieves the current input mode.
    fn get_input_mode(&self) -> InputMode;

    /// Get if the input method is activated.
    fn get_activated(&self) -> bool;
    /// Set if the input method is activated.
    fn set_activated(&mut self, activated: bool);

    /// Sets the rectangle area for positioning candidate window.
    fn set_preedit_rect(&mut self, x: i32, y: i32, width: i32, height: i32);

    fn set_commit_callback(&mut self, callback: CommitCallback);
    fn set_preedit_callback(&mut self, callback: PreEditCallback);
    fn set_candidate_callback(&mut self, callback: CandidateCallback);
    fn set_input_source_callback(&mut self, callback: InputSourceCallback);
    fn set_input_mode_callback(&mut self, callback: InputModeCallback);

    /// Get candidate list configuration
    fn get_candidate_config(&self) -> CandidateConfig;
    /// Set candidate list configuration
    fn set_candidate_config(&mut self, config: CandidateConfig);
}

/// Version string for the library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
